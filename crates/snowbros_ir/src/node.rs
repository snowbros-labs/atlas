//! The Atlas IR node set.
//!
//! One [`Module`] per source file, holding the file's imports, declared
//! symbols, call sites, and bare references. The set is deliberately small
//! for v0.2.0; each later milestone adds only the nodes its rules need.

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use snowbros_core::Span;

use crate::id::{ModuleId, SymbolId};

/// The Atlas IR for a single source module (one file).
///
/// Produced by lowering a parsed file. Collections are kept in source
/// order by the lowering pass and must not be reordered by consumers that
/// need determinism — sort explicitly where output order matters.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    /// Project-relative path of the file this module was lowered from.
    pub path: Utf8PathBuf,
    /// Module references — `import` / `require` / re-export sources.
    pub imports: Vec<Import>,
    /// Top-level declared symbols (functions, classes, bindings).
    pub symbols: Vec<Symbol>,
    /// Call sites in the module, flattened for call-graph construction.
    pub calls: Vec<Call>,
    /// Bare identifier references (uses of a binding), for reachability
    /// and unused-binding analysis in later milestones.
    pub references: Vec<Reference>,
}

impl Module {
    /// Creates an empty module for the given path.
    pub fn new(path: impl Into<Utf8PathBuf>) -> Self {
        Self {
            path: path.into(),
            ..Self::default()
        }
    }

    /// This module's stable id (its path).
    pub fn id(&self) -> ModuleId {
        ModuleId::new(&self.path)
    }
}

/// A module reference: an `import`, `require`, or re-export source and the
/// names it binds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Import {
    /// The specifier exactly as written, e.g. `./util` or `react`.
    pub source: String,
    /// Names bound by the import: `default`, `*`, or named exports.
    pub names: Vec<String>,
    /// Location of the import statement.
    pub span: Span,
}

/// A declared symbol: a function, class, or binding.
///
/// [`Symbol`] is intentionally path-free so identical declarations dedupe
/// and cache cleanly; combine with the owning [`Module::path`] via
/// [`Symbol::id`] to obtain a globally-unique [`SymbolId`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symbol {
    /// Declared identifier. `default` for a default export with no name.
    pub name: String,
    /// The syntactic kind of the declaration (and any kind-specific data).
    pub kind: SymbolKind,
    /// Span of the declaration's name (or the declaration itself when the
    /// name is absent, e.g. an anonymous default export).
    pub span: Span,
    /// Whether the symbol is exported from its module.
    pub exported: bool,
}

impl Symbol {
    /// Builds this symbol's globally-unique id within the given module.
    pub fn id(&self, module_path: impl AsRef<Utf8Path>) -> SymbolId {
        SymbolId::new(module_path, self.kind.tag(), &self.name, self.span)
    }
}

/// The syntactic kind of a [`Symbol`].
///
/// Structural only — a `Function` here is *any* function form; whether it
/// is a React component or a hook is decided by the semantic layer, not
/// recorded in the IR. Data-carrying variants hold the extra structure a
/// declaration exposes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SymbolKind {
    /// A function in any form (declaration, arrow, method, generator).
    Function(FunctionData),
    /// A class declaration.
    Class(ClassData),
    /// A `const` binding.
    Const,
    /// A `let` binding.
    Let,
    /// A `var` binding.
    Var,
    /// A binding whose form the lowering pass did not classify further.
    Unknown,
}

impl SymbolKind {
    /// Short, stable tag used in [`SymbolId`] construction and reporting.
    ///
    /// Must stay stable across releases — it participates in symbol ids
    /// and therefore in cache keys.
    pub fn tag(&self) -> &'static str {
        match self {
            SymbolKind::Function(_) => "function",
            SymbolKind::Class(_) => "class",
            SymbolKind::Const => "const",
            SymbolKind::Let => "let",
            SymbolKind::Var => "var",
            SymbolKind::Unknown => "unknown",
        }
    }
}

/// Extra structure carried by a function-kind [`Symbol`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionData {
    /// Parameter names in order. Destructured parameters are recorded as
    /// `{}` / `[]` rather than guessed apart.
    pub params: Vec<String>,
    /// Whether the function is declared `async`.
    pub is_async: bool,
    /// Whether the function body syntactically returns JSX. Populated from
    /// the React milestone (M1); `false` until lowering learns JSX.
    pub returns_jsx: bool,
    /// Span of the function body, when it has one.
    pub body_span: Option<Span>,
}

/// Extra structure carried by a class-kind [`Symbol`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassData {
    /// Method and field names declared on the class, in source order.
    pub members: Vec<String>,
}

/// A call site.
///
/// `callee` is the textual callee (`useState`, `foo.bar`) — the semantic
/// layer resolves it to a [`SymbolId`] when building the call graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    /// Textual callee expression as written.
    pub callee: String,
    /// Number of arguments passed.
    pub arg_count: u32,
    /// Location of the call expression.
    pub span: Span,
    /// The declared symbol that encloses this call, when known — the
    /// caller side of a future call-graph edge.
    pub in_symbol: Option<SymbolId>,
}

/// A bare reference to an identifier (a use of a binding).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    /// The referenced name.
    pub name: String,
    /// Location of the reference.
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_core::Position;

    fn span(a: u32, b: u32) -> Span {
        Span::new(Position::new(1, 1), Position::new(1, 1), a, b)
    }

    #[test]
    fn symbol_id_uses_kind_tag() {
        let sym = Symbol {
            name: "Page".to_string(),
            kind: SymbolKind::Function(FunctionData::default()),
            span: span(10, 40),
            exported: true,
        };
        assert_eq!(
            sym.id("src/app/page.tsx").as_str(),
            "src/app/page.tsx#function#Page@10-40"
        );
    }

    #[test]
    fn module_id_is_its_path() {
        let m = Module::new("src/lib/util.ts");
        assert_eq!(m.id().as_str(), "src/lib/util.ts");
    }

    #[test]
    fn kind_tags_are_exhaustive_and_stable() {
        assert_eq!(SymbolKind::Const.tag(), "const");
        assert_eq!(SymbolKind::Let.tag(), "let");
        assert_eq!(SymbolKind::Var.tag(), "var");
        assert_eq!(SymbolKind::Unknown.tag(), "unknown");
        assert_eq!(SymbolKind::Class(ClassData::default()).tag(), "class");
        assert_eq!(
            SymbolKind::Function(FunctionData::default()).tag(),
            "function"
        );
    }

    #[test]
    fn module_serde_roundtrip_is_stable() {
        let mut m = Module::new("a.tsx");
        m.imports.push(Import {
            source: "react".to_string(),
            names: vec!["default".to_string(), "useState".to_string()],
            span: span(0, 30),
        });
        m.symbols.push(Symbol {
            name: "App".to_string(),
            kind: SymbolKind::Function(FunctionData {
                params: vec!["props".to_string()],
                is_async: false,
                returns_jsx: true,
                body_span: Some(span(40, 80)),
            }),
            span: span(35, 38),
            exported: true,
        });
        m.calls.push(Call {
            callee: "useState".to_string(),
            arg_count: 1,
            span: span(50, 62),
            in_symbol: Some(m.symbols[0].id(&m.path)),
        });
        m.references.push(Reference {
            name: "props".to_string(),
            span: span(70, 75),
        });

        let json = serde_json::to_string(&m).unwrap();
        let back: Module = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);

        // Determinism: re-serializing the round-tripped value is identical.
        assert_eq!(json, serde_json::to_string(&back).unwrap());
    }

    #[test]
    fn symbol_kind_serializes_with_tag() {
        let json = serde_json::to_string(&SymbolKind::Const).unwrap();
        assert_eq!(json, "{\"kind\":\"const\"}");
    }
}
