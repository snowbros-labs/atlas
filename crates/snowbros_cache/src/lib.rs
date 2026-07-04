//! Persistent incremental cache.
//!
//! Caches the expensive step of the pipeline — parsing and import
//! extraction — keyed by file content. The semantic graph itself is
//! rebuilt from cached import lists every run: graph construction is
//! cheap in-memory work, parsing dominates, and rebuilding keeps rule
//! results provably identical to a cold run.
//!
//! Correctness policy (correctness beats speed, always):
//! - fast path: size + mtime match → entry trusted (standard Ruff-style
//!   fingerprinting)
//! - slow path: mtime differs → content is read and xxh3-hashed; only a
//!   hash match reuses the entry
//! - the whole cache is discarded when the cache format version, engine
//!   version, or configuration fingerprint (tsconfig/package.json/
//!   snowbros.toml) changes
//! - a corrupted or unreadable cache file is silently discarded — the
//!   engine falls back to a cold scan, never to stale data
//! - deleted/renamed files: the new cache is rebuilt from the current
//!   scan only, so entries for vanished paths are dropped automatically

pub mod fingerprint;
pub mod store;

pub use fingerprint::{config_fingerprint, hash_bytes, FileFingerprint};
pub use store::{CacheData, CacheStats, FileEntry, Lookup, CACHE_DIR, CACHE_FORMAT_VERSION};
