//! Import resolution: maps a module specifier written in one file to the
//! project file it refers to.
//!
//! Covers:
//! - relative specifiers (`./x`, `../y`) with Node/TS extension and
//!   index probing
//! - tsconfig `paths` aliases (`@/…`) via [`TsPaths`]
//! - bare package specifiers → [`Resolution::External`]
//!
//! Anything not provably resolvable is [`Resolution::Unresolved`] — the
//! engine reports "don't know" rather than guessing.
//!
//! Resolution is a pure function over a [`FileSet`] snapshot and a
//! pre-loaded [`TsPaths`] table: no I/O, fully deterministic, trivially
//! testable.

pub mod fileset;
pub mod jsonc;
pub mod tsconfig;

pub use fileset::FileSet;
pub use tsconfig::TsPaths;

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
/// against the project's file set and tsconfig alias table.
pub fn resolve(from: &Utf8Path, specifier: &str, files: &FileSet, aliases: &TsPaths) -> Resolution {
    if specifier.starts_with("./") || specifier.starts_with("../") {
        let base = from.parent().unwrap_or(Utf8Path::new(""));
        let target = normalize(&base.join(specifier));
        return match probe(&target, files) {
            Some(path) => Resolution::Project(path),
            None => Resolution::Unresolved(specifier.to_string()),
        };
    }

    // tsconfig alias? Try every candidate the alias table produces.
    let candidates = aliases.expand(specifier);
    if !candidates.is_empty() {
        for candidate in &candidates {
            if let Some(path) = probe(candidate, files) {
                return Resolution::Project(path);
            }
        }
        return Resolution::Unresolved(specifier.to_string());
    }

    // Alias-looking specifiers with no configured mapping: don't know,
    // don't guess.
    if specifier.starts_with("@/") || specifier.starts_with("~/") {
        return Resolution::Unresolved(specifier.to_string());
    }
    Resolution::External(specifier.to_string())
}

/// Extension and index probing against the file set:
/// exact → `x.{ts,tsx,…}` → `x/index.{ts,tsx,…}`.
fn probe(target: &Utf8Path, files: &FileSet) -> Option<Utf8PathBuf> {
    if files.contains(target) {
        return Some(target.to_path_buf());
    }
    for ext in EXTENSIONS {
        let candidate = Utf8PathBuf::from(format!("{target}.{ext}"));
        if files.contains(&candidate) {
            return Some(candidate);
        }
    }
    // Built as strings to keep forward slashes on Windows.
    for ext in EXTENSIONS {
        let candidate = Utf8PathBuf::from(format!("{target}/index.{ext}"));
        if files.contains(&candidate) {
            return Some(candidate);
        }
    }
    None
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
            resolve(
                Utf8Path::new("src/app.ts"),
                "./util.ts",
                &fs,
                &TsPaths::default()
            ),
            Resolution::Project("src/util.ts".into())
        );
    }

    #[test]
    fn extension_probing_prefers_ts() {
        let fs = files(&["src/app.ts", "src/util.js", "src/util.ts"]);
        assert_eq!(
            resolve(
                Utf8Path::new("src/app.ts"),
                "./util",
                &fs,
                &TsPaths::default()
            ),
            Resolution::Project("src/util.ts".into())
        );
    }

    #[test]
    fn index_probing() {
        let fs = files(&["src/app.ts", "src/components/index.tsx"]);
        assert_eq!(
            resolve(
                Utf8Path::new("src/app.ts"),
                "./components",
                &fs,
                &TsPaths::default()
            ),
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
                &fs,
                &TsPaths::default()
            ),
            Resolution::Project("src/shared/api.ts".into())
        );
    }

    #[test]
    fn bare_specifier_is_external() {
        let fs = files(&["src/app.ts"]);
        assert_eq!(
            resolve(
                Utf8Path::new("src/app.ts"),
                "react",
                &fs,
                &TsPaths::default()
            ),
            Resolution::External("react".into())
        );
        assert_eq!(
            resolve(
                Utf8Path::new("src/app.ts"),
                "node:fs",
                &fs,
                &TsPaths::default()
            ),
            Resolution::External("node:fs".into())
        );
        assert_eq!(
            resolve(
                Utf8Path::new("src/app.ts"),
                "@scope/pkg",
                &fs,
                &TsPaths::default()
            ),
            Resolution::External("@scope/pkg".into())
        );
    }

    #[test]
    fn alias_is_unresolved_not_guessed() {
        let fs = files(&["src/app.ts", "src/components/ui/button.tsx"]);
        assert_eq!(
            resolve(
                Utf8Path::new("src/app.ts"),
                "@/components/ui/button",
                &fs,
                &TsPaths::default()
            ),
            Resolution::Unresolved("@/components/ui/button".into())
        );
    }

    #[test]
    fn configured_alias_resolves() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("tsconfig.json"),
            r#"{ "compilerOptions": { "paths": { "@/*": ["./src/*"] } } }"#,
        )
        .unwrap();
        let aliases = TsPaths::load(Utf8Path::new(dir.path().to_str().unwrap()));
        let fs = files(&["src/app.ts", "src/components/ui/button.tsx"]);
        assert_eq!(
            resolve(
                Utf8Path::new("src/app.ts"),
                "@/components/ui/button",
                &fs,
                &aliases
            ),
            Resolution::Project("src/components/ui/button.tsx".into())
        );
        // Configured alias pointing at nothing: Unresolved, not External.
        assert_eq!(
            resolve(Utf8Path::new("src/app.ts"), "@/missing", &fs, &aliases),
            Resolution::Unresolved("@/missing".into())
        );
    }

    #[test]
    fn missing_relative_target_is_unresolved() {
        let fs = files(&["src/app.ts"]);
        assert_eq!(
            resolve(
                Utf8Path::new("src/app.ts"),
                "./nope",
                &fs,
                &TsPaths::default()
            ),
            Resolution::Unresolved("./nope".into())
        );
    }
}
