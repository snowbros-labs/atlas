//! Core types shared by every SNOWBROS Inspector subsystem.
//!
//! This crate defines the common vocabulary of the engine: [`Diagnostic`],
//! [`Severity`], [`Confidence`], [`Span`], [`SourceLocation`], [`Config`],
//! and the [`Project`] model. Analysis crates depend on these types; they
//! never define their own competing versions.
//!
//! Design rules for this crate:
//! - No analysis logic. Types and (de)serialization only.
//! - Everything is `serde`-serializable — all consumers (CLI, LSP,
//!   dashboard) read the same structured JSON.
//! - Deterministic: no timestamps, randomness, or environment-dependent
//!   values inside analysis result types.

pub mod config;
pub mod diagnostic;
pub mod error;
pub mod project;
pub mod severity;
pub mod span;

pub use config::Config;
pub use diagnostic::{Diagnostic, Evidence, SuggestedFix};
pub use error::CoreError;
pub use project::Project;
pub use severity::{Confidence, Severity};
pub use span::{Position, SourceLocation, Span};
