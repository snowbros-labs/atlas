//! Atlas IR — the language-agnostic intermediate representation.
//!
//! Every language parser *lowers* its own syntax tree (today tree-sitter
//! JS/TS/JSX/TSX; tomorrow Python, Go, …) into the single shape defined
//! here. Everything downstream — the semantic layer, the graph builder,
//! and the rule engine — reads Atlas IR, never a language-specific AST.
//! That is what lets a future `python/large-function` rule reuse the same
//! machinery as `react/large-component`: both read an [`ir::Symbol`] whose
//! kind is a [`SymbolKind::Function`].
//!
//! Design rules for this crate (mirroring [`snowbros_core`]):
//! - **No analysis logic.** Node types, stable ids, and (de)serialization
//!   only. Meaning (component detection, type resolution) belongs in the
//!   semantic layer.
//! - **Deterministic.** Ids are content-derived and sortable; no
//!   timestamps, randomness, or map-iteration order leaks into the IR.
//! - **`serde`-serializable.** IR is cacheable — warm re-analysis must
//!   re-derive byte-identical IR.
//! - **Structural, not semantic.** [`SymbolKind`] carries only syntactic
//!   kinds (function, class, const). React/TS meaning is layered on top by
//!   `snowbros_semantic`, not encoded here.
//!
//! The node set is intentionally minimal for v0.2.0 and grows per
//! milestone: JSX/hook affordances arrive with React (M1), type-level
//! nodes with TypeScript (M2), control-flow nodes with the
//! maintainability rules.
//!
//! [`ir::Symbol`]: Symbol

pub mod id;
pub mod node;

pub use id::{ModuleId, SymbolId};
pub use node::{
    Call, ClassData, EnumData, FunctionData, Import, InterfaceData, Module, Reference, Symbol,
    SymbolKind, TypeAliasData,
};
