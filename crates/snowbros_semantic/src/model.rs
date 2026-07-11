//! The project symbol model.

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};

use snowbros_core::Span;
use snowbros_graph::{EdgeKind, Node, SemanticGraph};
use snowbros_ir::{Module, Symbol, SymbolId, SymbolKind};

/// A symbol together with the module that declares it.
///
/// Borrows into the [`SemanticModel`]; cheap to pass around and always
/// carries enough to build a [`SymbolId`] and point at source.
#[derive(Debug, Clone, Copy)]
pub struct SymbolRef<'a> {
    /// Path of the declaring module.
    pub module: &'a Utf8Path,
    /// The declared symbol.
    pub symbol: &'a Symbol,
}

impl SymbolRef<'_> {
    /// The symbol's globally-unique, stable id.
    pub fn id(&self) -> SymbolId {
        self.symbol.id(self.module)
    }
}

/// A name declared more than once at module top level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Duplicate {
    /// Module the redeclaration occurs in.
    pub module: Utf8PathBuf,
    /// The redeclared name.
    pub name: String,
    /// Every declaration span for the name, in source order.
    pub spans: Vec<Span>,
}

/// A project-wide index of declared symbols, built from lowered IR.
///
/// Modules are keyed by path in a [`BTreeMap`] so every traversal is
/// deterministic without extra sorting.
#[derive(Debug, Clone, Default)]
pub struct SemanticModel {
    modules: BTreeMap<Utf8PathBuf, Module>,
}

impl SemanticModel {
    /// Builds a model from lowered IR modules. Later modules with the same
    /// path replace earlier ones (last write wins), matching how a file is
    /// re-lowered on change.
    pub fn from_modules(modules: impl IntoIterator<Item = Module>) -> Self {
        let mut map = BTreeMap::new();
        for module in modules {
            map.insert(module.path.clone(), module);
        }
        Self { modules: map }
    }

    /// Number of indexed modules.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// The IR module for a path, if present.
    pub fn module(&self, path: impl AsRef<Utf8Path>) -> Option<&Module> {
        self.modules.get(path.as_ref())
    }

    /// Every module, in path order.
    pub fn modules(&self) -> impl Iterator<Item = &Module> {
        self.modules.values()
    }

    /// Every declared symbol across the project, in (module path, source)
    /// order.
    pub fn symbols(&self) -> Vec<SymbolRef<'_>> {
        let mut out = Vec::new();
        for (path, module) in &self.modules {
            for symbol in &module.symbols {
                out.push(SymbolRef {
                    module: path,
                    symbol,
                });
            }
        }
        out
    }

    /// The top-level function symbol whose body span encloses the byte
    /// range `[start_byte, end_byte)` in `module`, if any — a minimal
    /// call-enclosure resolution (the caller side of a future call-graph
    /// edge). Nested closures resolve to their nearest *top-level*
    /// declaration, which is sufficient for reachability-style rules and
    /// never yields a false enclosure.
    pub fn enclosing_symbol(
        &self,
        module: impl AsRef<Utf8Path>,
        start_byte: u32,
        end_byte: u32,
    ) -> Option<SymbolRef<'_>> {
        let (path, m) = self.modules.get_key_value(module.as_ref())?;
        for symbol in &m.symbols {
            if let SymbolKind::Function(data) = &symbol.kind {
                if let Some(body) = data.body_span {
                    if body.start_byte <= start_byte && end_byte <= body.end_byte {
                        return Some(SymbolRef {
                            module: path,
                            symbol,
                        });
                    }
                }
            }
        }
        None
    }

    /// Every exported symbol across the project, in (module path, source)
    /// order — the input to unused-export analysis.
    pub fn exported_symbols(&self) -> Vec<SymbolRef<'_>> {
        self.symbols()
            .into_iter()
            .filter(|s| s.symbol.exported)
            .collect()
    }

    /// Names declared more than once at module top level, per module.
    ///
    /// Detection is name-based within a single module: two top-level
    /// declarations sharing a name are a redeclaration regardless of kind
    /// (a `const` and a `function` of the same name genuinely clash in
    /// JS/TS). Results are sorted by (module, name); spans stay in source
    /// order so the evidence reads top-to-bottom.
    pub fn duplicate_declarations(&self) -> Vec<Duplicate> {
        let mut out = Vec::new();
        for (path, module) in &self.modules {
            // Preserve first-seen (source) order of names while grouping.
            let mut order: Vec<&str> = Vec::new();
            let mut spans: BTreeMap<&str, Vec<Span>> = BTreeMap::new();
            for symbol in &module.symbols {
                let entry = spans.entry(&symbol.name).or_default();
                if entry.is_empty() {
                    order.push(&symbol.name);
                }
                entry.push(symbol.span);
            }
            for name in order {
                let name_spans = &spans[name];
                if name_spans.len() > 1 {
                    out.push(Duplicate {
                        module: path.clone(),
                        name: name.to_string(),
                        spans: name_spans.clone(),
                    });
                }
            }
        }
        out.sort_by(|a, b| a.module.cmp(&b.module).then_with(|| a.name.cmp(&b.name)));
        out
    }

    /// Every resolved intra-module call edge, as `(caller, callee)` symbol
    /// ids, in (module, source) order.
    ///
    /// A call contributes an edge when both sides are known: its enclosing
    /// top-level function (`Call::in_symbol`, set during lowering) and a
    /// top-level symbol in the *same* module whose name matches the textual
    /// callee. Cross-file calls and member calls (`foo.bar`) are not
    /// resolved here — cross-file call resolution needs module resolution
    /// and lands in a later unit. Accuracy over quantity: an unresolved
    /// callee yields no edge rather than a guessed one.
    pub fn call_edges(&self) -> Vec<(SymbolId, SymbolId)> {
        let mut out = Vec::new();
        for (path, module) in &self.modules {
            // name → first top-level symbol declaring it (source order).
            let mut by_name: BTreeMap<&str, &Symbol> = BTreeMap::new();
            for symbol in &module.symbols {
                by_name.entry(&symbol.name).or_insert(symbol);
            }
            for call in &module.calls {
                let Some(caller) = &call.in_symbol else {
                    continue;
                };
                if let Some(callee) = by_name.get(call.callee.as_str()) {
                    out.push((caller.clone(), callee.id(path)));
                }
            }
        }
        out
    }

    /// Populates the semantic graph with symbol-level structure:
    /// - a [`Node`] per file (deduplicated by the graph);
    /// - a [`Node`] per declared symbol;
    /// - a `Contains` edge file → symbol for every declaration;
    /// - an `Exports` edge file → symbol for every exported declaration;
    /// - a `Calls` edge caller symbol → callee symbol for every resolved
    ///   intra-module call (see [`SemanticModel::call_edges`]).
    ///
    /// Additive: existing file/package nodes and edges are untouched.
    pub fn populate_graph(&self, graph: &mut SemanticGraph) {
        for (path, module) in &self.modules {
            let file = graph.add_node(Node::file(path.clone()));
            for symbol in &module.symbols {
                let sym = graph.add_node(Node::symbol(
                    path.clone(),
                    symbol.name.clone(),
                    symbol.kind.tag(),
                ));
                graph.add_edge(file, sym, EdgeKind::Contains);
                if symbol.exported {
                    graph.add_edge(file, sym, EdgeKind::Exports);
                }
            }
        }
        // Call edges, added after all symbol nodes exist so both endpoints
        // resolve. Node labels are stable (`module#kind#name`), so the
        // caller/callee ids map onto the nodes inserted above.
        for (caller, callee) in self.call_edges() {
            let (Some(from), Some(to)) = (
                graph.find(&symbol_node_label(&caller)),
                graph.find(&symbol_node_label(&callee)),
            ) else {
                continue;
            };
            graph.add_edge(from, to, EdgeKind::Calls);
        }
    }
}

/// The graph node label for a symbol id.
///
/// A [`SymbolId`] is `path#kind#name@start-end`; a symbol *node* label is
/// `path#kind#name` (spans are not part of node identity). Strip the span
/// suffix to join the two.
fn symbol_node_label(id: &SymbolId) -> String {
    let s = id.as_str();
    match s.rfind('@') {
        Some(at) => s[..at].to_string(),
        None => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_core::Position;
    use snowbros_ir::{FunctionData, SymbolKind};

    fn span(a: u32, b: u32) -> Span {
        Span::new(Position::new(1, 1), Position::new(1, 1), a, b)
    }

    fn sym(name: &str, kind: SymbolKind, exported: bool, at: u32) -> Symbol {
        Symbol {
            name: name.to_string(),
            kind,
            span: span(at, at + 1),
            exported,
        }
    }

    fn module(path: &str, symbols: Vec<Symbol>) -> Module {
        let mut m = Module::new(path);
        m.symbols = symbols;
        m
    }

    #[test]
    fn indexes_symbols_in_deterministic_order() {
        let model = SemanticModel::from_modules([
            module("b.ts", vec![sym("z", SymbolKind::Const, false, 0)]),
            module(
                "a.ts",
                vec![
                    sym(
                        "foo",
                        SymbolKind::Function(FunctionData::default()),
                        true,
                        0,
                    ),
                    sym("bar", SymbolKind::Const, false, 10),
                ],
            ),
        ]);
        let names: Vec<&str> = model
            .symbols()
            .iter()
            .map(|s| s.symbol.name.as_str())
            .collect();
        // a.ts before b.ts (path order); within a.ts, source order.
        assert_eq!(names, vec!["foo", "bar", "z"]);
    }

    #[test]
    fn exported_symbols_filtered() {
        let model = SemanticModel::from_modules([module(
            "a.ts",
            vec![
                sym("pub", SymbolKind::Const, true, 0),
                sym("priv", SymbolKind::Const, false, 10),
            ],
        )]);
        let exported: Vec<&str> = model
            .exported_symbols()
            .iter()
            .map(|s| s.symbol.name.as_str())
            .collect();
        assert_eq!(exported, vec!["pub"]);
    }

    #[test]
    fn symbol_ref_builds_stable_id() {
        let model = SemanticModel::from_modules([module(
            "src/app/page.tsx",
            vec![sym(
                "Page",
                SymbolKind::Function(FunctionData::default()),
                true,
                16,
            )],
        )]);
        let s = model.symbols()[0];
        assert_eq!(s.id().as_str(), "src/app/page.tsx#function#Page@16-17");
    }

    #[test]
    fn detects_duplicate_declarations() {
        let model = SemanticModel::from_modules([module(
            "a.ts",
            vec![
                sym("dup", SymbolKind::Const, false, 0),
                sym("unique", SymbolKind::Const, false, 10),
                sym(
                    "dup",
                    SymbolKind::Function(FunctionData::default()),
                    false,
                    20,
                ),
            ],
        )]);
        let dups = model.duplicate_declarations();
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].name, "dup");
        assert_eq!(dups[0].module, "a.ts");
        // Both spans, in source order.
        assert_eq!(dups[0].spans, vec![span(0, 1), span(20, 21)]);
    }

    #[test]
    fn no_false_duplicate_across_modules() {
        // Same name in two files is not a redeclaration.
        let model = SemanticModel::from_modules([
            module("a.ts", vec![sym("x", SymbolKind::Const, false, 0)]),
            module("b.ts", vec![sym("x", SymbolKind::Const, false, 0)]),
        ]);
        assert!(model.duplicate_declarations().is_empty());
    }

    #[test]
    fn populates_graph_with_contains_and_exports() {
        let model = SemanticModel::from_modules([module(
            "a.ts",
            vec![
                sym(
                    "pub",
                    SymbolKind::Function(FunctionData::default()),
                    true,
                    0,
                ),
                sym("priv", SymbolKind::Const, false, 10),
            ],
        )]);
        let mut g = SemanticGraph::new();
        model.populate_graph(&mut g);

        // 1 file + 2 symbols.
        assert_eq!(g.node_count(), 3);
        let file = g.find("a.ts").unwrap();
        let pub_sym = g.find("a.ts#function#pub").unwrap();
        let priv_sym = g.find("a.ts#const#priv").unwrap();

        assert!(g.has_outgoing(file, EdgeKind::Contains));
        assert!(g.has_incoming(pub_sym, EdgeKind::Contains));
        assert!(g.has_incoming(priv_sym, EdgeKind::Contains));
        // Only the exported symbol has an Exports edge.
        assert!(g.has_incoming(pub_sym, EdgeKind::Exports));
        assert!(!g.has_incoming(priv_sym, EdgeKind::Exports));
    }

    #[test]
    fn equal_names_across_files_are_distinct_nodes() {
        let model = SemanticModel::from_modules([
            module(
                "a.ts",
                vec![sym(
                    "f",
                    SymbolKind::Function(FunctionData::default()),
                    true,
                    0,
                )],
            ),
            module(
                "b.ts",
                vec![sym(
                    "f",
                    SymbolKind::Function(FunctionData::default()),
                    true,
                    0,
                )],
            ),
        ]);
        let mut g = SemanticGraph::new();
        model.populate_graph(&mut g);
        // 2 files + 2 distinct symbol nodes (no collision).
        assert_eq!(g.node_count(), 4);
        assert!(g.find("a.ts#function#f").is_some());
        assert!(g.find("b.ts#function#f").is_some());
    }

    fn func(name: &str, exported: bool, name_at: u32, body: (u32, u32)) -> Symbol {
        Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function(FunctionData {
                body_span: Some(span(body.0, body.1)),
                ..FunctionData::default()
            }),
            span: span(name_at, name_at + 1),
            exported,
        }
    }

    #[test]
    fn resolves_intra_module_call_edges() {
        use snowbros_ir::Call;
        let mut m = module(
            "a.ts",
            vec![
                func("caller", true, 0, (10, 90)),
                func("callee", false, 100, (110, 120)),
            ],
        );
        let caller_id = m.symbols[0].id("a.ts");
        // Call to `callee` from inside `caller`'s body.
        m.calls.push(Call {
            callee: "callee".to_string(),
            arg_count: 0,
            span: span(50, 58),
            in_symbol: Some(caller_id.clone()),
        });
        // Call to an unknown / external name — no edge.
        m.calls.push(Call {
            callee: "external".to_string(),
            arg_count: 0,
            span: span(60, 68),
            in_symbol: Some(caller_id.clone()),
        });

        let model = SemanticModel::from_modules([m]);
        let edges = model.call_edges();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].0, caller_id);
        assert_eq!(edges[0].1.as_str(), "a.ts#function#callee@100-101");
    }

    #[test]
    fn populate_graph_adds_calls_edge() {
        use snowbros_ir::Call;
        let mut m = module(
            "a.ts",
            vec![
                func("f", true, 0, (10, 90)),
                func("g", false, 100, (110, 120)),
            ],
        );
        let f_id = m.symbols[0].id("a.ts");
        m.calls.push(Call {
            callee: "g".to_string(),
            arg_count: 0,
            span: span(50, 51),
            in_symbol: Some(f_id),
        });
        let model = SemanticModel::from_modules([m]);
        let mut graph = SemanticGraph::new();
        model.populate_graph(&mut graph);

        let f = graph.find("a.ts#function#f").unwrap();
        let g = graph.find("a.ts#function#g").unwrap();
        assert!(graph.has_outgoing(f, EdgeKind::Calls));
        assert!(graph.has_incoming(g, EdgeKind::Calls));
    }

    #[test]
    fn no_call_edge_without_enclosure() {
        use snowbros_ir::Call;
        let mut m = module("a.ts", vec![func("g", false, 100, (110, 120))]);
        // Module-level call with no enclosing symbol → not an edge.
        m.calls.push(Call {
            callee: "g".to_string(),
            arg_count: 0,
            span: span(0, 1),
            in_symbol: None,
        });
        let model = SemanticModel::from_modules([m]);
        assert!(model.call_edges().is_empty());
    }

    #[test]
    fn last_write_wins_on_repeated_path() {
        let model = SemanticModel::from_modules([
            module("a.ts", vec![sym("old", SymbolKind::Const, false, 0)]),
            module("a.ts", vec![sym("new", SymbolKind::Const, false, 0)]),
        ]);
        assert_eq!(model.module_count(), 1);
        let names: Vec<&str> = model
            .symbols()
            .iter()
            .map(|s| s.symbol.name.as_str())
            .collect();
        assert_eq!(names, vec!["new"]);
    }
}
