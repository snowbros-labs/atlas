//! Rule engine.
//!
//! Every rule implements [`Rule`]: read-only over an
//! [`AnalysisContext`], returns evidence-backed [`Diagnostic`]s. Rules
//! are order-independent and never mutate shared state, so the registry
//! can run them in any order (or in parallel later) with identical
//! results.
//!
//! Current rules are graph rules; AST/pattern rules join once the rule
//! metadata (YAML) layer lands.

pub mod context;
pub mod registry;
pub mod rules;

pub use context::AnalysisContext;
pub use registry::{builtin_rules, run_all, Rule};

// Re-exported so every subsystem shares the same core vocabulary.
pub use snowbros_core as core;
