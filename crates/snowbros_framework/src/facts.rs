//! Input facts for framework detection.
//!
//! [`ProjectFacts`] is a plain data snapshot of the signals detection
//! needs. Building it from disk is I/O; consuming it is pure. That split
//! keeps detectors deterministic and unit-testable.

use std::collections::BTreeMap;
use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

/// The subset of `package.json` that detection reads.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PackageJson {
    /// Runtime dependencies.
    pub dependencies: BTreeMap<String, String>,
    /// Development dependencies.
    #[serde(rename = "devDependencies")]
    pub dev_dependencies: BTreeMap<String, String>,
}

impl PackageJson {
    /// Looks up a dependency in runtime deps first, then dev deps.
    /// Returns the declared version string.
    pub fn dependency_version(&self, name: &str) -> Option<&str> {
        self.dependencies
            .get(name)
            .or_else(|| self.dev_dependencies.get(name))
            .map(String::as_str)
    }

    /// Whether the package depends on `name` at all.
    pub fn has_dependency(&self, name: &str) -> bool {
        self.dependency_version(name).is_some()
    }
}

/// Deterministic snapshot of the signals framework detection consumes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectFacts {
    /// Parsed `package.json` at the project root, if present.
    pub package_json: Option<PackageJson>,
    /// Root-relative paths of files and directories that exist at the
    /// project root (one level deep is enough for marker checks; deeper
    /// paths may be included and are matched exactly).
    pub root_entries: Vec<Utf8PathBuf>,
}

impl ProjectFacts {
    /// Whether a root-relative path exists in the snapshot.
    pub fn has_entry(&self, path: &str) -> bool {
        self.root_entries.iter().any(|p| p == path)
    }

    /// Returns the first of the given paths that exists in the snapshot.
    pub fn first_existing_entry<'a>(&self, paths: &[&'a str]) -> Option<&'a str> {
        paths.iter().copied().find(|p| self.has_entry(p))
    }

    /// Reads facts from a project root on disk.
    ///
    /// Collects root entries plus the first level of well-known
    /// subdirectories (`src/`, `app/`, `pages/`) so marker checks like
    /// `src/app` work.
    pub fn from_dir(root: &Utf8Path) -> Self {
        let package_json = fs::read_to_string(root.join("package.json"))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok());

        let mut root_entries = Vec::new();
        collect_entries(root, "", &mut root_entries);
        for sub in ["src", "app", "pages", "supabase", "prisma"] {
            collect_entries(&root.join(sub), sub, &mut root_entries);
        }
        root_entries.sort();

        Self {
            package_json,
            root_entries,
        }
    }
}

/// Appends the children of `dir` (as `prefix/child`) to `out`. Missing or
/// unreadable directories contribute nothing.
fn collect_entries(dir: &Utf8Path, prefix: &str, out: &mut Vec<Utf8PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let rel = if prefix.is_empty() {
            Utf8PathBuf::from(name)
        } else {
            Utf8PathBuf::from(prefix).join(name)
        };
        out.push(rel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_json_parses_both_dep_sections() {
        let json = r#"{
            "name": "demo",
            "dependencies": { "next": "^15.0.0" },
            "devDependencies": { "typescript": "^5.6.0" }
        }"#;
        let pkg: PackageJson = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.dependency_version("next"), Some("^15.0.0"));
        assert_eq!(pkg.dependency_version("typescript"), Some("^5.6.0"));
        assert!(!pkg.has_dependency("vue"));
    }

    #[test]
    fn runtime_dep_wins_over_dev_dep() {
        let json = r#"{
            "dependencies": { "react": "19.0.0" },
            "devDependencies": { "react": "18.0.0" }
        }"#;
        let pkg: PackageJson = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.dependency_version("react"), Some("19.0.0"));
    }

    #[test]
    fn entry_lookup() {
        let facts = ProjectFacts {
            package_json: None,
            root_entries: vec!["next.config.ts".into(), "src/app".into()],
        };
        assert!(facts.has_entry("next.config.ts"));
        assert!(facts.has_entry("src/app"));
        assert!(!facts.has_entry("nuxt.config.ts"));
    }
}
