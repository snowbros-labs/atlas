//! Cache storage: on-disk format, lookup, persistence.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use tracing::debug;

use snowbros_parser::FileFacts;

use crate::fingerprint::{hash_bytes, FileFingerprint};

/// Bumped whenever the on-disk layout or cached data semantics change.
/// A mismatch discards the cache wholesale.
/// v2: entries store full [`FileFacts`] instead of just imports.
/// v3: facts gained eval calls and secret candidates.
/// v4: facts gained directives (`use client` / `use server`).
pub const CACHE_FORMAT_VERSION: u32 = 4;

/// Directory (under the project root) holding cache state.
pub const CACHE_DIR: &str = ".snowbros";

const CACHE_FILE: &str = "cache.json";

/// Cached parse result for one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Fast-path fingerprint (size + mtime).
    pub fingerprint: FileFingerprint,
    /// xxh3 hash of the file content (hex).
    pub content_hash: String,
    /// Extracted facts, or `None` when the file failed to parse (the
    /// failure reason is cached too, so broken files don't get re-parsed
    /// every run).
    pub facts: Option<FileFacts>,
    /// Parse/read failure message when `facts` is `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
}

/// Result of a cache lookup for one file.
#[derive(Debug)]
pub enum Lookup {
    /// Entry is valid — reuse the cached parse result. Boxed: an entry
    /// carries full file facts and dwarfs the `Stale` variant.
    Fresh(Box<FileEntry>),
    /// Entry is stale or absent. When the content was already read for
    /// hash comparison it is handed back so the caller never reads the
    /// file twice.
    Stale(Option<String>),
}

/// Counters for observability.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// Entries reused (fast path or content-hash path).
    pub hits: usize,
    /// Files that had to be parsed.
    pub misses: usize,
}

/// The whole cache: header + per-file entries.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CacheData {
    /// On-disk format version.
    pub format_version: u32,
    /// Fingerprint of global configuration (see
    /// [`crate::config_fingerprint`]).
    pub config_fingerprint: String,
    /// Per-file entries, keyed by root-relative path.
    pub files: BTreeMap<Utf8PathBuf, FileEntry>,
}

impl CacheData {
    /// Creates an empty cache with the given config fingerprint.
    pub fn empty(config_fingerprint: String) -> Self {
        Self {
            format_version: CACHE_FORMAT_VERSION,
            config_fingerprint,
            files: BTreeMap::new(),
        }
    }

    /// Loads the cache from `<root>/.snowbros/cache.json`.
    ///
    /// Returns an empty cache when the file is missing, unreadable,
    /// corrupted, from another format version, or built under a
    /// different configuration — stale data is never trusted.
    pub fn load(root: &Path, config_fingerprint: &str) -> Self {
        let path = root.join(CACHE_DIR).join(CACHE_FILE);
        let Ok(text) = fs::read_to_string(&path) else {
            debug!(target: "snowbros::cache", "no cache file — cold scan");
            return Self::empty(config_fingerprint.to_string());
        };
        let Ok(data) = serde_json::from_str::<Self>(&text) else {
            debug!(target: "snowbros::cache", "corrupted cache — discarded");
            return Self::empty(config_fingerprint.to_string());
        };
        if data.format_version != CACHE_FORMAT_VERSION {
            debug!(target: "snowbros::cache", "format version mismatch — discarded");
            return Self::empty(config_fingerprint.to_string());
        }
        if data.config_fingerprint != config_fingerprint {
            debug!(target: "snowbros::cache", "config changed — discarded");
            return Self::empty(config_fingerprint.to_string());
        }
        data
    }

    /// Persists the cache under `<root>/.snowbros/`, creating the
    /// directory (with a self-ignoring `.gitignore`) if needed.
    /// Best-effort: failure to persist never fails the analysis.
    pub fn save(&self, root: &Path) {
        let dir = root.join(CACHE_DIR);
        if fs::create_dir_all(&dir).is_err() {
            return;
        }
        // Keep the cache out of version control without user action.
        let gitignore = dir.join(".gitignore");
        if !gitignore.exists() {
            let _ = fs::write(&gitignore, "*\n");
        }
        if let Ok(json) = serde_json::to_string(self) {
            let _ = fs::write(dir.join(CACHE_FILE), json);
        }
    }

    /// Checks whether the cached entry for `rel` is still valid for the
    /// file at `abs`.
    pub fn lookup(&self, rel: &Utf8PathBuf, abs: &Path) -> Lookup {
        let Some(entry) = self.files.get(rel) else {
            return Lookup::Stale(None);
        };
        let Some(current) = FileFingerprint::read(abs) else {
            return Lookup::Stale(None);
        };
        // Fast path: identical size and a usable, identical mtime.
        if current == entry.fingerprint && current.mtime_ms != 0 {
            return Lookup::Fresh(Box::new(entry.clone()));
        }
        // Slow path: metadata moved (touch, checkout, copy) — compare
        // content. Same content ⇒ same parse result, by determinism.
        let Ok(content) = fs::read_to_string(abs) else {
            return Lookup::Stale(None);
        };
        if hash_bytes(content.as_bytes()) == entry.content_hash {
            let mut refreshed = entry.clone();
            refreshed.fingerprint = current;
            return Lookup::Fresh(Box::new(refreshed));
        }
        Lookup::Stale(Some(content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_parser::{extract_facts, parse, Language};

    fn entry_for(content: &str, abs: &Path) -> FileEntry {
        let parsed = parse(content.to_string(), Language::TypeScript).unwrap();
        FileEntry {
            fingerprint: FileFingerprint::read(abs).unwrap(),
            content_hash: hash_bytes(content.as_bytes()),
            facts: Some(extract_facts(&parsed)),
            failure: None,
        }
    }

    fn setup(
        content: &str,
    ) -> (
        tempfile::TempDir,
        Utf8PathBuf,
        std::path::PathBuf,
        CacheData,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let rel = Utf8PathBuf::from("src/a.ts");
        let abs = dir.path().join("src/a.ts");
        fs::create_dir_all(abs.parent().unwrap()).unwrap();
        fs::write(&abs, content).unwrap();
        let mut cache = CacheData::empty("cfg1".into());
        cache.files.insert(rel.clone(), entry_for(content, &abs));
        (dir, rel, abs, cache)
    }

    #[test]
    fn unchanged_file_is_fresh() {
        let (_dir, rel, abs, cache) = setup("import x from \"./b\";");
        assert!(matches!(cache.lookup(&rel, &abs), Lookup::Fresh(_)));
    }

    #[test]
    fn touched_but_identical_content_is_fresh_via_hash() {
        let (_dir, rel, abs, mut cache) = setup("import x from \"./b\";");
        // Simulate a touch: stored mtime differs from disk.
        cache.files.get_mut(&rel).unwrap().fingerprint.mtime_ms = 12345;
        match cache.lookup(&rel, &abs) {
            Lookup::Fresh(e) => assert_ne!(e.fingerprint.mtime_ms, 12345),
            other => panic!("expected Fresh, got {other:?}"),
        }
    }

    #[test]
    fn changed_content_is_stale_and_returns_content() {
        let (_dir, rel, abs, cache) = setup("import x from \"./b\";");
        fs::write(&abs, "import y from \"./c\"; // changed").unwrap();
        match cache.lookup(&rel, &abs) {
            Lookup::Stale(Some(content)) => assert!(content.contains("./c")),
            other => panic!("expected Stale(Some), got {other:?}"),
        }
    }

    #[test]
    fn unknown_file_is_stale() {
        let (_dir, _rel, abs, cache) = setup("export {};");
        assert!(matches!(
            cache.lookup(&Utf8PathBuf::from("src/other.ts"), &abs),
            Lookup::Stale(None)
        ));
    }

    #[test]
    fn persistence_roundtrip() {
        let (dir, _rel, _abs, cache) = setup("export {};");
        cache.save(dir.path());
        let loaded = CacheData::load(dir.path(), "cfg1");
        assert_eq!(loaded, cache);
        // Self-ignoring: .snowbros/.gitignore written.
        assert!(dir.path().join(CACHE_DIR).join(".gitignore").exists());
    }

    #[test]
    fn config_change_discards_cache() {
        let (dir, _rel, _abs, cache) = setup("export {};");
        cache.save(dir.path());
        let loaded = CacheData::load(dir.path(), "cfg2-DIFFERENT");
        assert!(loaded.files.is_empty());
        assert_eq!(loaded.config_fingerprint, "cfg2-DIFFERENT");
    }

    #[test]
    fn corrupted_cache_discarded() {
        let (dir, _rel, _abs, cache) = setup("export {};");
        cache.save(dir.path());
        fs::write(dir.path().join(CACHE_DIR).join("cache.json"), "{ nope").unwrap();
        let loaded = CacheData::load(dir.path(), "cfg1");
        assert!(loaded.files.is_empty());
    }

    #[test]
    fn version_mismatch_discarded() {
        let (dir, _rel, _abs, mut cache) = setup("export {};");
        cache.format_version = CACHE_FORMAT_VERSION + 1;
        cache.save(dir.path());
        let loaded = CacheData::load(dir.path(), "cfg1");
        assert!(loaded.files.is_empty());
    }

    #[test]
    fn missing_cache_is_cold() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = CacheData::load(dir.path(), "cfg1");
        assert!(loaded.files.is_empty());
        assert_eq!(loaded.format_version, CACHE_FORMAT_VERSION);
    }
}
