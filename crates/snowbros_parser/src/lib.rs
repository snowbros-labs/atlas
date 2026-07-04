//! Multi-language parsing layer.
//!
//! Sprint 1 scope:
//! - Language detection ([`Language`]) from extension, file name, shebang
//! - Tree-sitter parsing for the JS/TS family ([`parse`])
//! - Import extraction ([`extract_imports`]) feeding the import graph
//!
//! oxc integration and the arena AST store land next.

pub mod facts;
pub mod imports;
pub mod language;
pub mod treesitter;

pub use facts::{extract_facts, FileFacts, NamedItem};
pub use imports::{extract_imports, Import, ImportKind};
pub use language::Language;
pub use treesitter::{parse, ParseError, ParsedFile};

// Re-exported so every subsystem shares the same core vocabulary.
pub use snowbros_core as core;
