//! Python import resolution.
//!
//! Maps a Python import to the project file(s) it refers to, using Python's
//! module-resolution rules rather than Node's. Kept separate from the JS/TS
//! [`resolve`](crate::resolve) so neither language's resolution can perturb
//! the other — the frontend-specific resolution RFC 0002 §2 calls for.
//!
//! Resolution is conservative in the zero-false-positive direction:
//! - a **relative** import (`from .mod import x`, `from . import y`) that
//!   points at nothing is [`PyResolution::Unresolved`] — a real broken import;
//! - an **absolute** dotted import (`import os`, `import myapp.util`) is
//!   resolved against project files when one matches, and otherwise treated as
//!   [`PyResolution::External`] (standard library or an installed package) —
//!   never flagged, because Atlas cannot see site-packages and must not guess.
//!
//! Like the JS resolver this is a pure function over a [`FileSet`] snapshot: no
//! I/O, fully deterministic.

use camino::{Utf8Path, Utf8PathBuf};

use crate::fileset::FileSet;

/// Outcome of resolving one Python import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PyResolution {
    /// Resolved to one or more project files (root-relative paths, sorted).
    /// A single `from . import a, b` can name several sibling modules, hence
    /// the vector.
    Project(Vec<Utf8PathBuf>),
    /// A standard-library or installed-package import — recognized, not a
    /// project file, never flagged.
    External,
    /// A relative import that resolves to no project file: a genuine broken
    /// import. Carries the specifier.
    Unresolved(String),
}

/// Resolves a Python import written in `from` (root-relative file path).
///
/// `specifier` is the module reference verbatim (leading dots preserved for
/// relative imports); `names` are the imported symbol names (`*` for a star
/// import), used only to resolve bare relative imports like `from . import x`.
///
/// `root_package` is the scan root's own package name when the root directory
/// is itself a package (contains a top-level `__init__.py`). Python makes such
/// a package importable by its own name from the parent on `sys.path`, so an
/// absolute import that leads with that name (`from fastapi.encoders import x`
/// when scanning the `fastapi/` package) refers to a project file even though
/// the leading segment is not a subdirectory of the root. Pass `None` when the
/// root is a plain source directory.
pub fn resolve_python_import(
    from: &Utf8Path,
    specifier: &str,
    names: &[String],
    files: &FileSet,
    root_package: Option<&str>,
) -> PyResolution {
    let dots = specifier.chars().take_while(|c| *c == '.').count();
    if dots > 0 {
        return resolve_relative(from, specifier, dots, names, files);
    }
    resolve_absolute(specifier, names, files, root_package)
}

/// Resolves a relative import by walking up `dots - 1` package levels from the
/// importing file's directory, then into the remaining dotted module path.
///
/// Paths are assembled as forward-slash strings (not via [`Utf8Path::join`],
/// which inserts a backslash on Windows) so a resolved target matches the
/// scanner's forward-slash file paths on every platform — the engine's
/// determinism contract.
fn resolve_relative(
    from: &Utf8Path,
    specifier: &str,
    dots: usize,
    names: &[String],
    files: &FileSet,
) -> PyResolution {
    // Start at the importing file's directory (its package), then ascend one
    // directory for each dot beyond the first.
    let mut base = from
        .parent()
        .map(|p| p.as_str().to_string())
        .unwrap_or_default();
    for _ in 1..dots {
        base = parent_str(&base);
    }

    let module_path = &specifier[dots..]; // e.g. "mod.sub" in ".mod.sub", "" in "."
    if !module_path.is_empty() {
        let target = join_str(&base, &module_path.replace('.', "/"));
        return match probe(&target, files) {
            Some(path) => PyResolution::Project(vec![path]),
            None => PyResolution::Unresolved(specifier.to_string()),
        };
    }

    // Bare `from . import a, b`: each name may be a sibling submodule. Resolve
    // the ones that are; names that are not modules (they are attributes of the
    // package's __init__) simply do not contribute an edge — never a false
    // "unresolved".
    let mut targets: Vec<Utf8PathBuf> = names
        .iter()
        .filter(|n| n.as_str() != "*")
        .filter_map(|n| probe(&join_str(&base, n), files))
        .collect();
    if targets.is_empty() {
        // Fall back to the package itself (`__init__.py`), if present.
        if let Some(init) = probe(&base, files) {
            targets.push(init);
        }
    }
    if targets.is_empty() {
        PyResolution::Unresolved(specifier.to_string())
    } else {
        targets.sort();
        targets.dedup();
        PyResolution::Project(targets)
    }
}

/// The parent of a forward-slash path string (everything before the last `/`),
/// or empty for a top-level segment.
fn parent_str(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((parent, _)) => parent.to_string(),
        None => String::new(),
    }
}

/// Joins a forward-slash base and a relative segment with `/`, avoiding a
/// leading slash when the base is empty (a top-level module).
fn join_str(base: &str, rest: &str) -> String {
    if base.is_empty() {
        rest.to_string()
    } else {
        format!("{base}/{rest}")
    }
}

/// Resolves an absolute dotted import. When the scan root is itself the package
/// named by `root_package`, an import leading with that name is resolved against
/// the root (Python imports the package by its own name from the parent). Then
/// probes for a matching project file (a first-party top-level package). Falls
/// back to `External` — the standard library or an installed third-party
/// package — which Atlas cannot see and must never flag.
fn resolve_absolute(
    specifier: &str,
    names: &[String],
    files: &FileSet,
    root_package: Option<&str>,
) -> PyResolution {
    // Root-is-a-package: an import leading with the root's own package name
    // refers to the project (Python imports the package by name from the
    // parent). A miss falls through to the ordinary probe below (kept
    // conservative: a leading-package import we cannot see becomes External,
    // never a false "unresolved").
    if let Some(pkg) = root_package {
        if let Some(rest) = strip_leading_segment(specifier, pkg) {
            if rest.is_empty() {
                // Bare `from <root_package> import a, b` — each name may be a
                // root-level submodule (`from fastapi import routing`), exactly
                // like the relative `from . import a, b` case. Resolve those;
                // names that are re-exported attributes of the package
                // `__init__` (not submodules) fall back to the `__init__`
                // itself. This keeps the edge on the submodule actually
                // imported rather than manufacturing a package-`__init__` cycle.
                let mut targets: Vec<Utf8PathBuf> = names
                    .iter()
                    .filter(|n| n.as_str() != "*")
                    .filter_map(|n| probe(n, files))
                    .collect();
                if targets.is_empty() {
                    let init = Utf8PathBuf::from("__init__.py");
                    if files.contains(&init) {
                        targets.push(init);
                    }
                }
                if !targets.is_empty() {
                    targets.sort();
                    targets.dedup();
                    return PyResolution::Project(targets);
                }
            } else if let Some(path) = probe(&rest.replace('.', "/"), files) {
                // `from <root_package>.a.b import x` → resolve `a/b` at the root.
                return PyResolution::Project(vec![path]);
            }
        }
    }

    let target = specifier.replace('.', "/");
    match probe(&target, files) {
        Some(path) => PyResolution::Project(vec![path]),
        None => PyResolution::External,
    }
}

/// If `specifier` leads with the dotted segment `seg` (exactly, at a `.`
/// boundary or the whole string), returns the remainder after it (`""` when
/// `specifier == seg`). Otherwise `None` — so `fastapimixed` is not treated as
/// leading with `fastapi`.
fn strip_leading_segment<'a>(specifier: &'a str, seg: &str) -> Option<&'a str> {
    if specifier == seg {
        return Some("");
    }
    specifier
        .strip_prefix(seg)
        .and_then(|rest| rest.strip_prefix('.'))
}

/// Probes a module path (a forward-slash string) against the file set:
/// `<target>.py` then `<target>/__init__.py` (a package). `.pyi` stubs are not
/// treated as modules for graph purposes.
fn probe(target: &str, files: &FileSet) -> Option<Utf8PathBuf> {
    let module = Utf8PathBuf::from(format!("{target}.py"));
    if files.contains(&module) {
        return Some(module);
    }
    let package = Utf8PathBuf::from(format!("{target}/__init__.py"));
    if files.contains(&package) {
        return Some(package);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(paths: &[&str]) -> FileSet {
        paths.iter().map(Utf8PathBuf::from).collect()
    }

    #[test]
    fn relative_module_resolves_to_sibling_file() {
        let fs = files(&["pkg/app.py", "pkg/models.py"]);
        let r = resolve_python_import(
            Utf8Path::new("pkg/app.py"),
            ".models",
            &["User".to_string()],
            &fs,
            None,
        );
        assert_eq!(r, PyResolution::Project(vec!["pkg/models.py".into()]));
    }

    #[test]
    fn relative_package_resolves_to_init() {
        let fs = files(&["pkg/app.py", "pkg/sub/__init__.py"]);
        let r = resolve_python_import(Utf8Path::new("pkg/app.py"), ".sub", &[], &fs, None);
        assert_eq!(r, PyResolution::Project(vec!["pkg/sub/__init__.py".into()]));
    }

    #[test]
    fn bare_relative_resolves_named_submodules() {
        let fs = files(&["pkg/app.py", "pkg/a.py", "pkg/b.py"]);
        let r = resolve_python_import(
            Utf8Path::new("pkg/app.py"),
            ".",
            &["a".to_string(), "b".to_string()],
            &fs,
            None,
        );
        assert_eq!(
            r,
            PyResolution::Project(vec!["pkg/a.py".into(), "pkg/b.py".into()])
        );
    }

    #[test]
    fn bare_relative_name_that_is_not_a_module_is_not_unresolved() {
        // `helper` is a function in __init__, not a submodule — no edge, but
        // not a false "unresolved" either. Resolves to the package init.
        let fs = files(&["pkg/app.py", "pkg/__init__.py"]);
        let r = resolve_python_import(
            Utf8Path::new("pkg/app.py"),
            ".",
            &["helper".to_string()],
            &fs,
            None,
        );
        assert_eq!(r, PyResolution::Project(vec!["pkg/__init__.py".into()]));
    }

    #[test]
    fn parent_relative_ascends_package_levels() {
        let fs = files(&["pkg/sub/app.py", "pkg/shared.py"]);
        let r = resolve_python_import(Utf8Path::new("pkg/sub/app.py"), "..shared", &[], &fs, None);
        assert_eq!(r, PyResolution::Project(vec!["pkg/shared.py".into()]));
    }

    #[test]
    fn broken_relative_import_is_unresolved() {
        let fs = files(&["pkg/app.py"]);
        let r = resolve_python_import(Utf8Path::new("pkg/app.py"), ".missing", &[], &fs, None);
        assert_eq!(r, PyResolution::Unresolved(".missing".to_string()));
    }

    #[test]
    fn absolute_stdlib_import_is_external_not_unresolved() {
        let fs = files(&["pkg/app.py"]);
        assert_eq!(
            resolve_python_import(Utf8Path::new("pkg/app.py"), "os", &[], &fs, None),
            PyResolution::External
        );
        assert_eq!(
            resolve_python_import(Utf8Path::new("pkg/app.py"), "os.path", &[], &fs, None),
            PyResolution::External
        );
    }

    #[test]
    fn absolute_first_party_package_resolves_to_project_file() {
        let fs = files(&["myapp/__init__.py", "myapp/util.py", "main.py"]);
        let r = resolve_python_import(Utf8Path::new("main.py"), "myapp.util", &[], &fs, None);
        assert_eq!(r, PyResolution::Project(vec!["myapp/util.py".into()]));
    }

    #[test]
    fn absolute_import_of_root_package_name_resolves_at_root() {
        // Scanning the `fastapi/` package itself: the fileset is root-relative
        // (`encoders.py`, not `fastapi/encoders.py`), yet code imports
        // `from fastapi.encoders import x`. With the root package name known,
        // the leading segment resolves to the root.
        let fs = files(&["__init__.py", "encoders.py", "middleware/cors.py"]);
        assert_eq!(
            resolve_python_import(
                Utf8Path::new("routing.py"),
                "fastapi.encoders",
                &[],
                &fs,
                Some("fastapi"),
            ),
            PyResolution::Project(vec!["encoders.py".into()])
        );
        // A nested module under the root package resolves too.
        assert_eq!(
            resolve_python_import(
                Utf8Path::new("applications.py"),
                "fastapi.middleware.cors",
                &[],
                &fs,
                Some("fastapi"),
            ),
            PyResolution::Project(vec!["middleware/cors.py".into()])
        );
        // The bare package name resolves to the root __init__.
        assert_eq!(
            resolve_python_import(
                Utf8Path::new("cli.py"),
                "fastapi",
                &[],
                &fs,
                Some("fastapi")
            ),
            PyResolution::Project(vec!["__init__.py".into()])
        );
        // `from fastapi import encoders` names a *submodule* — the edge lands on
        // the submodule file, not the package __init__ (which would fabricate a
        // cycle with __init__'s own re-export of it).
        assert_eq!(
            resolve_python_import(
                Utf8Path::new("param_functions.py"),
                "fastapi",
                &["encoders".to_string()],
                &fs,
                Some("fastapi"),
            ),
            PyResolution::Project(vec!["encoders.py".into()])
        );
    }

    #[test]
    fn root_package_miss_falls_through_to_external_not_unresolved() {
        // A leading-root-package import we cannot see (e.g. a C-extension or a
        // typo) must stay External — never a false "unresolved".
        let fs = files(&["__init__.py", "encoders.py"]);
        assert_eq!(
            resolve_python_import(
                Utf8Path::new("routing.py"),
                "fastapi.does_not_exist",
                &[],
                &fs,
                Some("fastapi"),
            ),
            PyResolution::External
        );
        // A third-party package that merely shares a prefix is not misrouted.
        assert_eq!(
            resolve_python_import(
                Utf8Path::new("routing.py"),
                "fastapi_extra.thing",
                &[],
                &fs,
                Some("fastapi"),
            ),
            PyResolution::External
        );
    }
}
