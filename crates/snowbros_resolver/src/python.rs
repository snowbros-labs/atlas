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
pub fn resolve_python_import(
    from: &Utf8Path,
    specifier: &str,
    names: &[String],
    files: &FileSet,
) -> PyResolution {
    let dots = specifier.chars().take_while(|c| *c == '.').count();
    if dots > 0 {
        return resolve_relative(from, specifier, dots, names, files);
    }
    resolve_absolute(specifier, files)
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

/// Resolves an absolute dotted import. Probes for a matching project file
/// first (a first-party top-level package), falling back to `External` for
/// the standard library or an installed third-party package.
fn resolve_absolute(specifier: &str, files: &FileSet) -> PyResolution {
    let target = specifier.replace('.', "/");
    match probe(&target, files) {
        Some(path) => PyResolution::Project(vec![path]),
        None => PyResolution::External,
    }
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
        );
        assert_eq!(r, PyResolution::Project(vec!["pkg/models.py".into()]));
    }

    #[test]
    fn relative_package_resolves_to_init() {
        let fs = files(&["pkg/app.py", "pkg/sub/__init__.py"]);
        let r = resolve_python_import(Utf8Path::new("pkg/app.py"), ".sub", &[], &fs);
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
        );
        assert_eq!(r, PyResolution::Project(vec!["pkg/__init__.py".into()]));
    }

    #[test]
    fn parent_relative_ascends_package_levels() {
        let fs = files(&["pkg/sub/app.py", "pkg/shared.py"]);
        let r = resolve_python_import(Utf8Path::new("pkg/sub/app.py"), "..shared", &[], &fs);
        assert_eq!(r, PyResolution::Project(vec!["pkg/shared.py".into()]));
    }

    #[test]
    fn broken_relative_import_is_unresolved() {
        let fs = files(&["pkg/app.py"]);
        let r = resolve_python_import(Utf8Path::new("pkg/app.py"), ".missing", &[], &fs);
        assert_eq!(r, PyResolution::Unresolved(".missing".to_string()));
    }

    #[test]
    fn absolute_stdlib_import_is_external_not_unresolved() {
        let fs = files(&["pkg/app.py"]);
        assert_eq!(
            resolve_python_import(Utf8Path::new("pkg/app.py"), "os", &[], &fs),
            PyResolution::External
        );
        assert_eq!(
            resolve_python_import(Utf8Path::new("pkg/app.py"), "os.path", &[], &fs),
            PyResolution::External
        );
    }

    #[test]
    fn absolute_first_party_package_resolves_to_project_file() {
        let fs = files(&["myapp/__init__.py", "myapp/util.py", "main.py"]);
        let r = resolve_python_import(Utf8Path::new("main.py"), "myapp.util", &[], &fs);
        assert_eq!(r, PyResolution::Project(vec!["myapp/util.py".into()]));
    }
}
