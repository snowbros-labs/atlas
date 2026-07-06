//! Node and edge model for the semantic graph.

use std::fmt;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

/// Opaque handle to a node in the [`crate::SemanticGraph`].
///
/// Wraps the underlying petgraph index so callers never depend on the
/// graph implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(pub(crate) u32);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "n{}", self.0)
    }
}

/// What kind of entity a node represents.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "kind")]
pub enum NodeKind {
    /// A source file.
    File {
        /// Project-relative path.
        path: Utf8PathBuf,
    },
    /// A named module (may span re-exports, barrels, packages-in-repo).
    Module {
        /// Module name, e.g. `@app/auth`.
        name: String,
    },
    /// A symbol: function, class, component, hook, variable, type.
    Symbol {
        /// Project-relative path of the module that declares the symbol.
        /// Part of the node's identity so equally-named symbols in
        /// different files are distinct nodes rather than merged.
        module: Utf8PathBuf,
        /// Symbol name as written in source.
        name: String,
        /// Free-form symbol kind, e.g. `function`, `class`, `component`.
        symbol_kind: String,
    },
    /// An external package dependency.
    Package {
        /// Package name, e.g. `react`.
        name: String,
        /// Declared version, verbatim from the manifest.
        version: Option<String>,
    },
}

/// A node: kind plus a stable display label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// The entity this node represents.
    pub kind: NodeKind,
}

impl Node {
    /// Convenience constructor for a file node.
    pub fn file(path: impl Into<Utf8PathBuf>) -> Self {
        Self {
            kind: NodeKind::File { path: path.into() },
        }
    }

    /// Convenience constructor for a symbol node. `module` is the path of
    /// the declaring file — it qualifies the symbol's identity so equal
    /// names across files do not collide.
    pub fn symbol(
        module: impl Into<Utf8PathBuf>,
        name: impl Into<String>,
        symbol_kind: impl Into<String>,
    ) -> Self {
        Self {
            kind: NodeKind::Symbol {
                module: module.into(),
                name: name.into(),
                symbol_kind: symbol_kind.into(),
            },
        }
    }

    /// Convenience constructor for a package node.
    pub fn package(name: impl Into<String>, version: Option<String>) -> Self {
        Self {
            kind: NodeKind::Package {
                name: name.into(),
                version,
            },
        }
    }

    /// Stable human-readable label (used in reports and DOT export).
    pub fn label(&self) -> String {
        match &self.kind {
            NodeKind::File { path } => path.to_string(),
            NodeKind::Module { name } => name.clone(),
            NodeKind::Symbol {
                module,
                name,
                symbol_kind,
            } => format!("{module}#{symbol_kind}#{name}"),
            NodeKind::Package { name, version } => match version {
                Some(v) => format!("{name}@{v}"),
                None => name.clone(),
            },
        }
    }
}

/// Typed relationship between two nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// `A imports B` (file → file).
    Imports,
    /// `A exports B` (file → symbol).
    Exports,
    /// `A contains B` (file → symbol).
    Contains,
    /// `A calls B` (symbol → symbol).
    Calls,
    /// `A references B`'s type (symbol → symbol).
    TypeRef,
    /// `A depends on B` (file/module → package).
    DependsOn,
}

impl fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Imports => "imports",
            Self::Exports => "exports",
            Self::Contains => "contains",
            Self::Calls => "calls",
            Self::TypeRef => "type_ref",
            Self::DependsOn => "depends_on",
        };
        f.write_str(s)
    }
}
