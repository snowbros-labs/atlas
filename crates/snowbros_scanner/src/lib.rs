//! Project file scanner.
//!
//! Walks a project root, respecting `.gitignore`, hidden-file
//! conventions, and hard exclusions (`node_modules`, build output), and
//! tags every file with its detected [`Language`].
//!
//! Output is deterministic: files are sorted by path regardless of
//! filesystem iteration order.

use camino::{Utf8Path, Utf8PathBuf};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};

use snowbros_parser::Language;

/// Directories never worth scanning, even without a `.gitignore`.
const HARD_EXCLUDES: &[&str] = &[
    ".snowbros",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".turbo",
    ".vercel",
    "coverage",
    "__pycache__",
    ".venv",
    "vendor",
];

/// One file found by the scanner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScannedFile {
    /// Path relative to the scanned root (always forward-slashed).
    pub path: Utf8PathBuf,
    /// Detected language, if recognized.
    pub language: Option<Language>,
    /// File size in bytes.
    pub size: u64,
}

/// Result of scanning a project root.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanResult {
    /// All scanned files, sorted by path.
    pub files: Vec<ScannedFile>,
    /// Paths the walker could not read (permission errors, broken
    /// symlinks). Reported, never silently dropped.
    pub skipped: Vec<String>,
}

impl ScanResult {
    /// Files of a specific language, in path order.
    pub fn files_of(&self, language: Language) -> impl Iterator<Item = &ScannedFile> {
        self.files
            .iter()
            .filter(move |f| f.language == Some(language))
    }

    /// Files in the ECMAScript family (JS/TS/JSX/TSX), in path order.
    pub fn ecmascript_files(&self) -> impl Iterator<Item = &ScannedFile> {
        self.files
            .iter()
            .filter(|f| f.language.is_some_and(Language::is_ecmascript))
    }
}

/// Scans `root`, returning every non-ignored file tagged with its
/// language.
pub fn scan(root: &Utf8Path) -> ScanResult {
    let mut result = ScanResult::default();

    let walker = WalkBuilder::new(root)
        .hidden(true) // skip dotfiles/dirs (.git, .cache, …)
        .git_ignore(true)
        .git_global(false) // machine-global ignores would break determinism
        .git_exclude(true)
        .require_git(false) // honor .gitignore even before `git init`
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !HARD_EXCLUDES.contains(&name.as_ref())
        })
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                result.skipped.push(e.to_string());
                continue;
            }
        };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Ok(abs) = Utf8PathBuf::from_path_buf(entry.path().to_path_buf()) else {
            result
                .skipped
                .push(format!("non-UTF-8 path: {}", entry.path().display()));
            continue;
        };
        let rel = abs.strip_prefix(root).unwrap_or(&abs);
        // Normalize to forward slashes for cross-platform determinism.
        let path = Utf8PathBuf::from(rel.as_str().replace('\\', "/"));

        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let language = Language::detect(&path, None);

        result.files.push(ScannedFile {
            path,
            language,
            size,
        });
    }

    result.files.sort_by(|a, b| a.path.cmp(&b.path));
    result.skipped.sort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(root: &std::path::Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn scan_dir(dir: &tempfile::TempDir) -> ScanResult {
        scan(Utf8Path::new(dir.path().to_str().unwrap()))
    }

    #[test]
    fn finds_and_tags_files_sorted() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/b.ts", "export {}");
        write(dir.path(), "src/a.tsx", "export {}");
        write(dir.path(), "README.md", "# hi");

        let result = scan_dir(&dir);
        let paths: Vec<&str> = result.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["README.md", "src/a.tsx", "src/b.ts"]);
        assert_eq!(result.files[1].language, Some(Language::Tsx));
        assert_eq!(result.files[0].language, Some(Language::Markdown));
    }

    #[test]
    fn skips_node_modules_and_hidden() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "src/app.ts", "export {}");
        write(dir.path(), "node_modules/pkg/index.js", "x");
        write(dir.path(), ".cache/tmp.js", "x");

        let result = scan_dir(&dir);
        let paths: Vec<&str> = result.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["src/app.ts"]);
    }

    #[test]
    fn respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), ".gitignore", "generated/\n");
        write(dir.path(), "src/app.ts", "export {}");
        write(dir.path(), "generated/out.ts", "export {}");

        let result = scan_dir(&dir);
        let paths: Vec<&str> = result.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["src/app.ts"]);
    }

    #[test]
    fn ecmascript_filter() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.ts", "");
        write(dir.path(), "b.py", "");
        write(dir.path(), "c.jsx", "");

        let result = scan_dir(&dir);
        let es: Vec<&str> = result.ecmascript_files().map(|f| f.path.as_str()).collect();
        assert_eq!(es, vec!["a.ts", "c.jsx"]);
    }

    #[test]
    fn deterministic_across_runs() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "z.ts", "");
        write(dir.path(), "a.ts", "");
        write(dir.path(), "m/x.ts", "");
        assert_eq!(scan_dir(&dir), scan_dir(&dir));
    }
}
