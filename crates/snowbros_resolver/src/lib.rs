//! Import resolution: maps a module specifier written in one file to the
//! project file it refers to.
//!
//! Sprint scope: relative specifiers (`./x`, `../y`) with Node/TS-style
//! extension and index probing. Package specifiers are classified as
//! [`Resolution::External`]; alias specifiers (`@/…`, tsconfig `paths`)
//! are [`Resolution::Unresolved`] until tsconfig support lands — the
//! engine reports "don't know" rather than guessing.
//!
//! Resolution is a pure function over a [`FileSet`] snapshot: no I/O,
//! fully deterministic, trivially testable.

pub mod fileset;

pub use fileset::FileSet;

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

/// Probe order for extensionless imports, mirroring the TS/Node
/// resolution most bundlers use. Order matters and is part of the
/// engine's determinism contract.
const EXTENSIONS: &[&str] = &["ts", "tsx", "d.ts", "js", "jsx", "mjs", "cjs", "json"];

/// Outcome of resolving one specifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum Resolution {
    /// Resolved to a file inside the project (root-relative path).
    Project(Utf8PathBuf),
    /// A package import (bare specifier like `react` or `node:fs`).
    External(String),
    /// Could not be resolved deterministically (unknown alias, or a
    /// relative path pointing at nothing). Carries the specifier.
    Unresolved(String),
}

/// Resolves `specifier` as written in `from` (root-relative file path)
/// against the project's file set.
pub fn resolve(from: &Utf8Path, specifier: &str, files: &FileSet) -> Resolution {
    if specifier.starts_with("./") || specifier.starts_with("../") {
        return resolve_relative(from, specifier, files);
    }
    // Bare specifiers: packages (`react`, `node:fs`, `@scope/pkg`).
    // `@/…` and other single-`@` aliases are tsconfig-defined, not
    // packages — we cannot resolve them yet.
    if specifier.starts_with("@/") || specifier.starts_with("~/") {
        return Resolution::Unresolved(specifier.to_string());
    }
    Resolution::External(specifier.to_string())
}

/// Relative resolution with extension and index probing.
fn resolve_relative(from: &Utf8Path, specifier: &str, files: &FileSet) -> Resolution {
    let base = from.parent().unwrap_or(Utf8Path::new(""));
    let target = normalize(&base.join(specifier));

    // 1. Exact file (specifier already has an extension).
    if files.contains(&target) {
        return Resolution::Project(target);
    }
    // 2. Extension probing: ./util → ./util.ts, ./util.tsx, …
    for ext in EXTENSIONS {
        let candidate = Utf8PathBuf::from(format!("{target}.{ext}"));
        if files.contains(&candidate) {
            return Resolution::Project(candidate);
        }
    }
    // 3. Directory index: ./util → ./util/index.ts, …
    // Built as a string to keep forward slashes on Windows.
    for ext in EXTENSIONS {
        let candidate = Utf8PathBuf::from(format!("{target}/index.{ext}"));
        if files.contains(&candidate) {
            return Resolution::Project(candidate);
        }
    }
    Resolution::Unresolved(specifier.to_string())
}

/// Lexically normalizes a path: resolves `.` and `..` segments without
/// touching the filesystem. Emits forward slashes regardless of host OS
/// (`Utf8Path::join` inserts `\` on Windows).
fn normalize(path: &Utf8Path) -> Utf8PathBuf {
    let unified = path.as_str().replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for component in unified.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    Utf8PathBuf::from(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(paths: &[&str]) -> FileSet {
        paths.iter().map(Utf8PathBuf::from).collect()
    }

    #[test]
    fn exact_relative_file() {
        let fs = files(&["src/app.ts", "src/util.ts"]);
        assert_eq!(
            resolve(Utf8Path::new("src/app.ts"), "./util.ts", &fs),
            Resolution::Project("src/util.ts".into())
        );
    }

    #[test]
    fn extension_probing_prefers_ts() {
        let fs = files(&["src/app.ts", "src/util.js", "src/util.ts"]);
        assert_eq!(
            resolve(Utf8Path::new("src/app.ts"), "./util", &fs),
            Resolution::Project("src/util.ts".into())
        );
    }

    #[test]
    fn index_probing() {
        let fs = files(&["src/app.ts", "src/components/index.tsx"]);
        assert_eq!(
            resolve(Utf8Path::new("src/app.ts"), "./components", &fs),
            Resolution::Project("src/components/index.tsx".into())
        );
    }

    #[test]
    fn parent_traversal() {
        let fs = files(&["src/features/auth/login.ts", "src/shared/api.ts"]);
        assert_eq!(
            resolve(
                Utf8Path::new("src/features/auth/login.ts"),
                "../../shared/api",
                &fs
            ),
            Resolution::Project("src/shared/api.ts".into())
        );
    }

    #[test]
    fn bare_specifier_is_external() {
        let fs = files(&["src/app.ts"]);
        assert_eq!(
            resolve(Utf8Path::new("src/app.ts"), "react", &fs),
            Resolution::External("react".into())
        );
        assert_eq!(
            resolve(Utf8Path::new("src/app.ts"), "node:fs", &fs),
            Resolution::External("node:fs".into())
        );
        assert_eq!(
            resolve(Utf8Path::new("src/app.ts"), "@scope/pkg", &fs),
            Resolution::External("@scope/pkg".into())
        );
    }

    #[test]
    fn alias_is_unresolved_not_guessed() {
        let fs = files(&["src/app.ts", "src/components/ui/button.tsx"]);
        assert_eq!(
            resolve(Utf8Path::new("src/app.ts"), "@/components/ui/button", &fs),
            Resolution::Unresolved("@/components/ui/button".into())
        );
    }

    #[test]
    fn missing_relative_target_is_unresolved() {
        let fs = files(&["src/app.ts"]);
        assert_eq!(
            resolve(Utf8Path::new("src/app.ts"), "./nope", &fs),
            Resolution::Unresolved("./nope".into())
        );
    }
}
