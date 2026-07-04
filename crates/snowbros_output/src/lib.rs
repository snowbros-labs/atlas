//! Output formatters.
//!
//! Analysis logic never touches presentation: analyzers produce
//! [`Report`]s, and each formatter here renders a `Report` into one
//! output format. CLI, VS Code, dashboard, and CI all consume the same
//! data.
//!
//! Sprint 4 scope: report model, JSON, Markdown. SARIF, HTML, and
//! colored terminal output follow.

pub mod json;
pub mod markdown;
pub mod report;

pub use report::{Report, Summary};

// Re-exported so every subsystem shares the same core vocabulary.
pub use snowbros_core as core;
