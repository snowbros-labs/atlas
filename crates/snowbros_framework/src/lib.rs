//! Automatic framework detection.
//!
//! Detection is fully automatic — no manual configuration — and layered
//! over three deterministic signals:
//! 1. Dependencies declared in `package.json` (or `composer.json`, etc.)
//! 2. Presence of well-known config files (`next.config.js`, …)
//! 3. Folder-structure markers (`app/` router, `manage.py`, …)
//!
//! Detection is a pure function over [`ProjectFacts`], so every detector
//! is testable without touching a real filesystem, and results are
//! reproducible by construction. Every detection carries evidence.

pub mod detect;
pub mod facts;
pub mod framework;

pub use detect::detect_frameworks;
pub use facts::{PackageJson, ProjectFacts};
pub use framework::{DetectedFramework, Framework};

// Re-exported so every subsystem shares the same core vocabulary.
pub use snowbros_core as core;
