//! Output formatters.
//!
//! Analysis logic never touches presentation: analyzers produce
//! [`Report`]s, and each formatter here renders a `Report` into one
//! output format. CLI, VS Code, dashboard, and CI all consume the same
//! data.
//!
//! Formats: JSON (canonical), Markdown, SARIF v2.1.0, self-contained
//! HTML.

pub mod html;
pub mod json;
pub mod markdown;
pub mod report;
pub mod sarif;
pub mod scoring;

pub use report::{Report, Summary};
pub use scoring::{CategoryScore, Scorecard};

// Re-exported so every subsystem shares the same core vocabulary.
pub use snowbros_core as core;
