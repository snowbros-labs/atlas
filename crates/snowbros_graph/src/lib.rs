//! Semantic graph engine — the heart of Snowbros Atlas.
//!
//! The [`SemanticGraph`] holds every entity the engine understands (files,
//! modules, symbols, packages) and the typed relationships between them.
//! Every analyzer reads this graph; none mutates it during analysis.
//!
//! Sprint 2 scope: graph model, circular-dependency detection (Tarjan
//! SCC), topological ordering, and impact analysis (reverse reachability).

pub mod graph;
pub mod model;

pub use graph::SemanticGraph;
pub use model::{EdgeKind, Node, NodeId, NodeKind};

// Re-exported so every subsystem shares the same core vocabulary.
pub use snowbros_core as core;
