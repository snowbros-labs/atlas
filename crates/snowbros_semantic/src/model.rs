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

/// A cycle of interfaces connected by `extends` heritage, within one module.
///
/// Such a cycle is always a TypeScript error (TS2310, "recursively
/// references itself as a base type"), so flagging it yields no false
/// positives. Member-annotation type cycles are legal and are *not*
/// represented here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeCycle {
    /// Module the cycle occurs in.
    pub module: Utf8PathBuf,
    /// The interfaces in the cycle, each `(name, declaration span)`, sorted
    /// by name for deterministic reporting.
    pub members: Vec<(String, Span)>,
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

    /// Cycles of interfaces connected by `extends` heritage, per module.
    ///
    /// Detection is intra-module: heritage is matched only to interfaces
    /// declared in the *same* file (cross-file type resolution is a later
    /// milestone), so no cross-file guess is made. Same-name interface
    /// declarations merge, and their `extends` lists union. A self-extends
    /// (`interface A extends A`) is a one-member cycle. Results are sorted
    /// by (module, first member name); members are sorted by name.
    pub fn circular_type_references(&self) -> Vec<TypeCycle> {
        let mut out = Vec::new();
        for (path, module) in &self.modules {
            // Interface name → (first span, unioned extends targets). Only
            // interfaces are nodes; merging same-name decls is TS-faithful.
            let mut span_of: BTreeMap<&str, Span> = BTreeMap::new();
            let mut edges: BTreeMap<&str, std::collections::BTreeSet<&str>> = BTreeMap::new();
            for symbol in &module.symbols {
                if let SymbolKind::Interface(data) = &symbol.kind {
                    span_of.entry(&symbol.name).or_insert(symbol.span);
                    let set = edges.entry(&symbol.name).or_default();
                    for target in &data.extends {
                        set.insert(target.as_str());
                    }
                }
            }
            // Restrict edges to targets that are interfaces in this module.
            let nodes: Vec<&str> = span_of.keys().copied().collect();
            let adj: BTreeMap<&str, Vec<&str>> = nodes
                .iter()
                .map(|&n| {
                    let mut targets: Vec<&str> = edges
                        .get(n)
                        .into_iter()
                        .flatten()
                        .copied()
                        .filter(|t| span_of.contains_key(t))
                        .collect();
                    targets.sort_unstable();
                    (n, targets)
                })
                .collect();

            for scc in tarjan_scc(&nodes, &adj) {
                let is_cycle = scc.len() > 1 || (scc.len() == 1 && adj[scc[0]].contains(&scc[0]));
                if !is_cycle {
                    continue;
                }
                let mut members: Vec<(String, Span)> =
                    scc.iter().map(|&n| (n.to_string(), span_of[n])).collect();
                members.sort_by(|a, b| a.0.cmp(&b.0));
                out.push(TypeCycle {
                    module: path.clone(),
                    members,
                });
            }
        }
        out.sort_by(|a, b| {
            a.module
                .cmp(&b.module)
                .then_with(|| a.members[0].0.cmp(&b.members[0].0))
        });
        out
    }

    /// The first exported symbol named `name` in `module`, if any — the
    /// resolution target for a cross-file reference to `name`.
    pub fn exported_symbol(&self, module: impl AsRef<Utf8Path>, name: &str) -> Option<SymbolId> {
        let (path, m) = self.modules.get_key_value(module.as_ref())?;
        m.symbols
            .iter()
            .find(|s| s.exported && s.name == name)
            .map(|s| s.id(path))
    }

    /// Every resolved intra-module call edge, as `(caller, callee)` symbol
    /// ids — see [`SemanticModel::resolved_call_edges`] with no imports.
    pub fn call_edges(&self) -> Vec<(SymbolId, SymbolId)> {
        self.resolved_call_edges(&ImportedNames::new())
    }

    /// Every resolved call edge, as `(caller, callee)` symbol ids, in
    /// (module, source) order.
    ///
    /// A call contributes an edge when both sides are known: its enclosing
    /// top-level function (`Call::in_symbol`, set during lowering) and a
    /// resolvable callee. Resolution order for a plain-identifier callee:
    /// 1. a top-level symbol in the *same* module (intra-file), else
    /// 2. an unaliased named import of that name (`imports`), resolving to
    ///    the matching **exported** symbol in the target module (cross-file).
    ///
    /// Member calls (`foo.bar`), default/namespace imports, and aliased
    /// imports are deliberately unresolved: `imports` keys are the names
    /// actually bound as callables, and `default`/`*` are excluded by the
    /// caller. Accuracy over quantity — an unresolved callee yields no edge
    /// rather than a guessed one.
    pub fn resolved_call_edges(&self, imports: &ImportedNames) -> Vec<(SymbolId, SymbolId)> {
        let mut out = Vec::new();
        for (path, module) in &self.modules {
            // name → first top-level symbol declaring it (source order).
            let mut by_name: BTreeMap<&str, &Symbol> = BTreeMap::new();
            for symbol in &module.symbols {
                by_name.entry(&symbol.name).or_insert(symbol);
            }
            let module_imports = imports.get(path);
            for call in &module.calls {
                let Some(caller) = &call.in_symbol else {
                    continue;
                };
                let callee = call.callee.as_str();
                if let Some(sym) = by_name.get(callee) {
                    // Intra-file: a local declaration shadows any import.
                    out.push((caller.clone(), sym.id(path)));
                } else if let Some(target) = module_imports.and_then(|m| m.get(callee)) {
                    // Cross-file: resolve to the exported symbol in target.
                    if let Some(callee_id) = self.exported_symbol(target, callee) {
                        out.push((caller.clone(), callee_id));
                    }
                }
            }
        }
        out
    }

    /// Populates the semantic graph with symbol-level structure, intra-file
    /// calls only. See [`SemanticModel::populate_graph_with_imports`].
    pub fn populate_graph(&self, graph: &mut SemanticGraph) {
        self.populate_graph_with_imports(graph, &ImportedNames::new());
    }

    /// Populates the semantic graph with symbol-level structure:
    /// - a [`Node`] per file (deduplicated by the graph);
    /// - a [`Node`] per declared symbol;
    /// - a `Contains` edge file → symbol for every declaration;
    /// - an `Exports` edge file → symbol for every exported declaration;
    /// - a `Calls` edge caller → callee for every resolved call
    ///   (intra-file, plus cross-file via `imports`; see
    ///   [`SemanticModel::resolved_call_edges`]).
    ///
    /// Additive: existing file/package nodes and edges are untouched.
    pub fn populate_graph_with_imports(&self, graph: &mut SemanticGraph, imports: &ImportedNames) {
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
        for (caller, callee) in self.resolved_call_edges(imports) {
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

/// Resolved project-internal imports, per module: for each module path, a
/// map from the local callable name to the module it resolves to.
///
/// Only unaliased **named** imports belong here — a name whose local
/// binding equals its exported name, so a call to it resolves to that
/// export in the target. Default/namespace/aliased imports are excluded by
/// the builder because their local name cannot be matched to a callee
/// safely.
pub type ImportedNames = BTreeMap<Utf8PathBuf, BTreeMap<String, Utf8PathBuf>>;

/// Tarjan's strongly-connected-components over a string-keyed graph.
///
/// `nodes` is iterated in the given order (callers pass a sorted list) and
/// each adjacency list is likewise pre-sorted, so the returned components —
/// and the order of nodes within them — are deterministic.
fn tarjan_scc<'a>(nodes: &[&'a str], adj: &BTreeMap<&'a str, Vec<&'a str>>) -> Vec<Vec<&'a str>> {
    struct State<'a, 'b> {
        adj: &'b BTreeMap<&'a str, Vec<&'a str>>,
        index: BTreeMap<&'a str, usize>,
        low: BTreeMap<&'a str, usize>,
        on_stack: std::collections::BTreeSet<&'a str>,
        stack: Vec<&'a str>,
        next: usize,
        out: Vec<Vec<&'a str>>,
    }
    fn strong<'a>(v: &'a str, st: &mut State<'a, '_>) {
        st.index.insert(v, st.next);
        st.low.insert(v, st.next);
        st.next += 1;
        st.stack.push(v);
        st.on_stack.insert(v);
        if let Some(succ) = st.adj.get(v) {
            for &w in succ {
                if !st.index.contains_key(w) {
                    strong(w, st);
                    let lw = st.low[w];
                    let lv = st.low[v];
                    st.low.insert(v, lv.min(lw));
                } else if st.on_stack.contains(w) {
                    let iw = st.index[w];
                    let lv = st.low[v];
                    st.low.insert(v, lv.min(iw));
                }
            }
        }
        if st.low[v] == st.index[v] {
            let mut component = Vec::new();
            while let Some(w) = st.stack.pop() {
                st.on_stack.remove(w);
                component.push(w);
                if w == v {
                    break;
                }
            }
            st.out.push(component);
        }
    }
    let mut st = State {
        adj,
        index: BTreeMap::new(),
        low: BTreeMap::new(),
        on_stack: std::collections::BTreeSet::new(),
        stack: Vec::new(),
        next: 0,
        out: Vec::new(),
    };
    for &n in nodes {
        if !st.index.contains_key(n) {
            strong(n, &mut st);
        }
    }
    st.out
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
    fn resolves_cross_file_call_edge() {
        use snowbros_ir::Call;
        // consumer.ts imports `helper` from util.ts and calls it.
        let mut consumer = module("consumer.ts", vec![func("run", true, 0, (10, 90))]);
        let run_id = consumer.symbols[0].id("consumer.ts");
        consumer.calls.push(Call {
            callee: "helper".to_string(),
            arg_count: 0,
            span: span(50, 58),
            in_symbol: Some(run_id.clone()),
        });
        let util = module("util.ts", vec![func("helper", true, 0, (10, 20))]);
        let model = SemanticModel::from_modules([consumer, util]);

        // Without imports: unresolved (helper is not local to consumer.ts).
        assert!(model.call_edges().is_empty());

        // With the named-import mapping helper → util.ts: resolved.
        let mut imports = ImportedNames::new();
        imports
            .entry("consumer.ts".into())
            .or_default()
            .insert("helper".to_string(), "util.ts".into());
        let edges = model.resolved_call_edges(&imports);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].0, run_id);
        assert_eq!(edges[0].1.as_str(), "util.ts#function#helper@0-1");
    }

    #[test]
    fn local_declaration_shadows_import() {
        use snowbros_ir::Call;
        // consumer declares its own `helper` and also imports one — the
        // local wins.
        let mut consumer = module(
            "consumer.ts",
            vec![
                func("run", true, 0, (10, 90)),
                func("helper", false, 100, (110, 120)),
            ],
        );
        let run_id = consumer.symbols[0].id("consumer.ts");
        consumer.calls.push(Call {
            callee: "helper".to_string(),
            arg_count: 0,
            span: span(50, 58),
            in_symbol: Some(run_id),
        });
        let util = module("util.ts", vec![func("helper", true, 0, (10, 20))]);
        let model = SemanticModel::from_modules([consumer, util]);
        let mut imports = ImportedNames::new();
        imports
            .entry("consumer.ts".into())
            .or_default()
            .insert("helper".to_string(), "util.ts".into());
        let edges = model.resolved_call_edges(&imports);
        assert_eq!(edges.len(), 1);
        // Resolves to the LOCAL helper, not util's.
        assert_eq!(edges[0].1.as_str(), "consumer.ts#function#helper@100-101");
    }

    fn iface(name: &str, extends: &[&str], at: u32) -> Symbol {
        use snowbros_ir::InterfaceData;
        Symbol {
            name: name.to_string(),
            kind: SymbolKind::Interface(InterfaceData {
                members: Vec::new(),
                extends: extends.iter().map(|s| s.to_string()).collect(),
                type_refs: Vec::new(),
            }),
            span: span(at, at + 1),
            exported: false,
        }
    }

    #[test]
    fn detects_mutual_interface_extends_cycle() {
        let model = SemanticModel::from_modules([module(
            "a.ts",
            vec![
                iface("A", &["B"], 0),
                iface("B", &["A"], 10),
                iface("C", &["A"], 20), // C→A is not a cycle (A does not reach C)
            ],
        )]);
        let cycles = model.circular_type_references();
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].module, "a.ts");
        let names: Vec<&str> = cycles[0].members.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["A", "B"]);
    }

    #[test]
    fn detects_self_extends_cycle() {
        let model =
            SemanticModel::from_modules([module("a.ts", vec![iface("Loop", &["Loop"], 0)])]);
        let cycles = model.circular_type_references();
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].members.len(), 1);
        assert_eq!(cycles[0].members[0].0, "Loop");
    }

    #[test]
    fn acyclic_and_external_heritage_are_clean() {
        // A→B linear (no cycle); D extends an interface not in this module.
        let model = SemanticModel::from_modules([module(
            "a.ts",
            vec![
                iface("A", &["B"], 0),
                iface("B", &[], 10),
                iface("D", &["Elsewhere"], 20),
            ],
        )]);
        assert!(model.circular_type_references().is_empty());
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
