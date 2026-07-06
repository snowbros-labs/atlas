//! Atlas semantic layer — the wedge.
//!
//! Takes the per-file [`snowbros_ir::Module`]s produced by lowering and
//! builds a **project-wide symbol model**: an index of every declared
//! symbol, resolution helpers over it, and the code that populates the
//! symbol nodes and edges the [`snowbros_graph`] already knows how to hold.
//!
//! This is where per-file *syntax* becomes per-project *meaning*. Symbol
//! resolution here is language-agnostic — it reads [`snowbros_ir`], never a
//! tree-sitter node — so it works for any language that lowers to IR.
//! Framework-specific enrichment (React components, Next.js routes) layers
//! on top in later milestones; it is not part of this crate's M0 surface.
//!
//! Design rules:
//! - **Deterministic.** Every returned collection is sorted by a stable
//!   key (module path, then symbol id); nothing leaks map order.
//! - **Read-only over the IR.** The model borrows nothing mutable and
//!   never rewrites the IR it was built from.
//! - **Evidence-preserving.** Every symbol keeps its span, so rules built
//!   on this model can always point at source.
//!
//! M0 scope: symbol index, exported-symbol enumeration, duplicate-
//! declaration detection, and graph population with `Contains` / `Exports`
//! edges. Cross-file reference resolution and `Calls` edges land with the
//! call-graph milestone (M2).

pub mod model;
pub mod react;

pub use model::{Duplicate, SemanticModel, SymbolRef};
pub use react::{role_of, ReactRole};
