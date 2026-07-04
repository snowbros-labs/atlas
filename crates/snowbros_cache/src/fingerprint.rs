//! File and configuration fingerprinting (xxh3).

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use xxhash_rust::xxh3::xxh3_64;

/// Cheap file metadata used for the fast-path check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFingerprint {
    /// File size in bytes.
    pub size: u64,
    /// Modification time as milliseconds since the Unix epoch. `0` when
    /// the platform cannot provide one (forces the content-hash path).
    pub mtime_ms: u128,
}

impl FileFingerprint {
    /// Reads size and mtime for a file. `None` when metadata is
    /// unavailable — callers must treat that as a cache miss.
    pub fn read(path: &Path) -> Option<Self> {
        let meta = fs::metadata(path).ok()?;
        let mtime_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis())
            .unwrap_or(0);
        Some(Self {
            size: meta.len(),
            mtime_ms,
        })
    }
}

/// Hashes bytes with xxh3-64, hex-encoded for JSON-safe storage.
pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:016x}", xxh3_64(bytes))
}

/// Fingerprint of everything that changes analysis behavior globally:
/// resolver config, dependency manifest, engine config. Any change here
/// invalidates the whole cache.
///
/// `engine_version` is mixed in so a new binary never reuses an old
/// cache silently.
pub fn config_fingerprint(root: &Path, engine_version: &str) -> String {
    let mut acc = Vec::new();
    acc.extend_from_slice(engine_version.as_bytes());
    // Sorted, fixed list — deterministic across platforms.
    for name in [
        "package.json",
        "snowbros.toml",
        "tsconfig.base.json",
        "tsconfig.json",
    ] {
        acc.extend_from_slice(name.as_bytes());
        match fs::read(root.join(name)) {
            Ok(bytes) => acc.extend_from_slice(&bytes),
            Err(_) => acc.push(0),
        }
    }
    hash_bytes(&acc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_and_content_sensitive() {
        assert_eq!(hash_bytes(b"abc"), hash_bytes(b"abc"));
        assert_ne!(hash_bytes(b"abc"), hash_bytes(b"abd"));
        assert_eq!(hash_bytes(b"abc").len(), 16);
    }

    #[test]
    fn config_fingerprint_changes_with_config() {
        let dir = tempfile::tempdir().unwrap();
        let before = config_fingerprint(dir.path(), "0.1.0");
        std::fs::write(dir.path().join("tsconfig.json"), "{}").unwrap();
        let after = config_fingerprint(dir.path(), "0.1.0");
        assert_ne!(before, after);
    }

    #[test]
    fn config_fingerprint_changes_with_engine_version() {
        let dir = tempfile::tempdir().unwrap();
        assert_ne!(
            config_fingerprint(dir.path(), "0.1.0"),
            config_fingerprint(dir.path(), "0.2.0")
        );
    }

    #[test]
    fn file_fingerprint_reads_size() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x.ts");
        std::fs::write(&f, "12345").unwrap();
        let fp = FileFingerprint::read(&f).unwrap();
        assert_eq!(fp.size, 5);
        assert!(fp.mtime_ms > 0);
    }
}
