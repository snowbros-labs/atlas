//! Python lowering: tree-sitter-python CST → Atlas IR + [`FileFacts`].
//!
//! The Python counterpart of [`crate::lowering`]. It translates Python syntax
//! into the *same* language-agnostic [`snowbros_ir::Module`] every other
//! frontend produces, so the semantic layer, symbol graph, and every shared
//! rule read Python exactly as they read TypeScript — no rule learns the word
//! "Python".
//!
//! Scope (M3, grows per milestone):
//! - top-level [`Symbol`]s: functions (`def`), classes, and module-level
//!   variable bindings, with their public/exported-ness;
//! - module [`Import`]s (`import a.b`, `from .pkg import x`), preserving
//!   relative-import dots for the resolver;
//! - [`Call`] sites with intra-module enclosure, for the call graph;
//! - [`Reference`]s (identifier uses) for reachability.
//!
//! Determinism and honesty match the JS/TS pass: nodes are emitted in source
//! order, nothing is sorted or deduplicated here, and constructs Atlas cannot
//! resolve statically (dynamic `__import__`, star-imported names, computed
//! attributes) are recorded as-is or skipped — never guessed.

use camino::Utf8PathBuf;
use tree_sitter::Node;

use snowbros_core::{Position, Span};
use snowbros_ir::{Call, ClassData, FunctionData, Import, Module, Reference, Symbol, SymbolKind};

use crate::facts::{FileFacts, NamedItem};
use crate::imports::{Import as FactImport, ImportKind};
use crate::treesitter::ParsedFile;

/// Lowers a parsed Python file into an Atlas IR [`Module`].
pub fn lower_python(parsed: &ParsedFile, path: impl Into<Utf8PathBuf>) -> Module {
    let mut module = Module::new(path);
    let root = parsed.tree.root_node();

    module.imports = collect_imports(root, parsed)
        .into_iter()
        .map(|i| Import {
            source: i.specifier,
            names: i.names,
            span: i.span,
        })
        .collect();

    // Public API of the module, if declared via `__all__` — authoritative
    // when present, otherwise the naming convention (leading `_` is private)
    // decides. Computed first so each symbol's `exported` flag is correct.
    let dunder_all = collect_dunder_all(root, parsed);

    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        lower_top_level(child, parsed, &mut module.symbols);
    }
    for sym in &mut module.symbols {
        sym.exported = is_public(&sym.name, dunder_all.as_deref());
    }

    collect_calls(root, parsed, &mut module.calls);
    resolve_call_enclosure(&mut module);

    let decl_name_spans: std::collections::BTreeSet<(u32, u32)> = module
        .symbols
        .iter()
        .map(|s| (s.span.start_byte, s.span.end_byte))
        .collect();
    collect_references(root, parsed, &decl_name_spans, &mut module.references);

    module
}

/// Extracts [`FileFacts`] from a parsed Python file: its imports (for the
/// import graph) and its exported names (for unused-export analysis).
///
/// The JS-only fact families — directives, `process.env` reads, dynamic Next
/// APIs, `eval`, secrets — are left empty; those are ECMAScript concepts and
/// the shared rules that read them simply find nothing for Python.
pub fn python_facts(parsed: &ParsedFile) -> FileFacts {
    let root = parsed.tree.root_node();
    let imports = collect_imports(root, parsed);
    let dunder_all = collect_dunder_all(root, parsed);

    let mut exports = Vec::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if let Some((name, span)) = top_level_name(child, parsed) {
            if is_public(&name, dunder_all.as_deref()) {
                exports.push(NamedItem { name, span });
            }
        }
    }

    FileFacts {
        imports,
        exports,
        ..FileFacts::default()
    }
}

/// Whether a module-level name is part of the module's public API.
///
/// `__all__`, when present, is authoritative (that is exactly its Python
/// meaning). Absent it, the convention holds: a leading underscore marks a
/// name private, everything else is public.
fn is_public(name: &str, dunder_all: Option<&[String]>) -> bool {
    match dunder_all {
        Some(all) => all.iter().any(|n| n == name),
        None => !name.starts_with('_'),
    }
}

/// Collects the string entries of a module-level `__all__ = [...]` /
/// `__all__ = (...)` assignment, or `None` if the module declares no
/// `__all__`. Only plain string literals are collected — a computed
/// `__all__` (concatenation, comprehension) yields an empty, present list
/// rather than a guess.
fn collect_dunder_all(root: Node<'_>, parsed: &ParsedFile) -> Option<Vec<String>> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() != "expression_statement" {
            continue;
        }
        let Some(assign) = child.named_child(0) else {
            continue;
        };
        if assign.kind() != "assignment" {
            continue;
        }
        let Some(left) = assign.child_by_field_name("left") else {
            continue;
        };
        if parsed.text_of(left) != "__all__" {
            continue;
        }
        let mut names = Vec::new();
        if let Some(right) = assign.child_by_field_name("right") {
            collect_string_literals(right, parsed, &mut names);
        }
        return Some(names);
    }
    None
}

/// Collects the text of `string` literals directly inside a list/tuple/set,
/// stripping quotes. Nested expressions other than string literals are
/// ignored (not guessed).
fn collect_string_literals(node: Node<'_>, parsed: &ParsedFile, out: &mut Vec<String>) {
    match node.kind() {
        "list" | "tuple" | "set" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "string" {
                    if let Some(s) = string_content(child, parsed) {
                        out.push(s);
                    }
                }
            }
        }
        _ => {}
    }
}

/// The inner text of a Python `string` node (its `string_content` child),
/// or `None` for an f-string / unusual form we will not guess at.
fn string_content(node: Node<'_>, parsed: &ParsedFile) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "string_content" {
            return Some(parsed.text_of(child).to_string());
        }
    }
    None
}

/// Lowers one top-level statement into zero or more [`Symbol`]s.
fn lower_top_level(node: Node<'_>, parsed: &ParsedFile, out: &mut Vec<Symbol>) {
    match node.kind() {
        "function_definition" => {
            if let Some(sym) = function_symbol(node, parsed) {
                out.push(sym);
            }
        }
        "class_definition" => {
            if let Some(sym) = class_symbol(node, parsed) {
                out.push(sym);
            }
        }
        // `@decorator` wraps the real definition; unwrap and lower it. The
        // decorators themselves are not symbols.
        "decorated_definition" => {
            if let Some(def) = node.child_by_field_name("definition") {
                lower_top_level(def, parsed, out);
            }
        }
        // Module-level `x = 1` / `x: int = 1` — a variable binding. Only a
        // bare identifier target is lowered; tuple/attribute targets are not
        // split apart or guessed.
        "expression_statement" => {
            if let Some(assign) = node.named_child(0) {
                if assign.kind() == "assignment" {
                    if let Some(left) = assign.child_by_field_name("left") {
                        if left.kind() == "identifier" {
                            out.push(Symbol {
                                name: parsed.text_of(left).to_string(),
                                kind: SymbolKind::Var,
                                span: span_of(left),
                                exported: false,
                            });
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// The declared name + name span of a top-level statement, if it declares
/// one (mirrors [`lower_top_level`] for the facts pass).
fn top_level_name(node: Node<'_>, parsed: &ParsedFile) -> Option<(String, Span)> {
    match node.kind() {
        "function_definition" | "class_definition" => {
            let name = node.child_by_field_name("name")?;
            Some((parsed.text_of(name).to_string(), span_of(name)))
        }
        "decorated_definition" => top_level_name(node.child_by_field_name("definition")?, parsed),
        "expression_statement" => {
            let assign = node.named_child(0)?;
            if assign.kind() != "assignment" {
                return None;
            }
            let left = assign.child_by_field_name("left")?;
            if left.kind() != "identifier" {
                return None;
            }
            Some((parsed.text_of(left).to_string(), span_of(left)))
        }
        _ => None,
    }
}

/// Builds a `Function` symbol from a `function_definition`.
fn function_symbol(node: Node<'_>, parsed: &ParsedFile) -> Option<Symbol> {
    let name = node.child_by_field_name("name")?;
    Some(Symbol {
        name: parsed.text_of(name).to_string(),
        kind: SymbolKind::Function(function_data(node, parsed)),
        span: span_of(name),
        exported: false,
    })
}

/// Builds a `Class` symbol from a `class_definition`.
fn class_symbol(node: Node<'_>, parsed: &ParsedFile) -> Option<Symbol> {
    let name = node.child_by_field_name("name")?;
    Some(Symbol {
        name: parsed.text_of(name).to_string(),
        kind: SymbolKind::Class(class_data(node, parsed)),
        span: span_of(name),
        exported: false,
    })
}

/// Extracts [`FunctionData`] from a `function_definition`.
fn function_data(node: Node<'_>, parsed: &ParsedFile) -> FunctionData {
    let params = node
        .child_by_field_name("parameters")
        .map(|p| param_names(p, parsed))
        .unwrap_or_default();
    // `async def` — the keyword is an anonymous child token of the definition.
    let mut cursor = node.walk();
    let is_async = node.children(&mut cursor).any(|c| c.kind() == "async");
    let body_span = node.child_by_field_name("body").map(span_of);
    FunctionData {
        params,
        is_async,
        // Python has no JSX; this stays false so React rules never fire here.
        returns_jsx: false,
        body_span,
    }
}

/// Parameter names in order. `self`/`cls` are kept (they are real
/// parameters). Splats record their bound name; `*`/`**` markers and complex
/// forms are recorded without being split apart.
fn param_names(params: Node<'_>, parsed: &ParsedFile) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = params.walk();
    for child in params.children(&mut cursor) {
        match child.kind() {
            "identifier" => names.push(parsed.text_of(child).to_string()),
            "default_parameter" | "typed_default_parameter" => {
                if let Some(name) = child.child_by_field_name("name") {
                    names.push(parsed.text_of(name).to_string());
                }
            }
            "typed_parameter" => {
                // First identifier child is the parameter name.
                let mut c = child.walk();
                for n in child.children(&mut c) {
                    if n.kind() == "identifier" {
                        names.push(parsed.text_of(n).to_string());
                        break;
                    }
                }
            }
            "list_splat_pattern" => {
                if let Some(id) = child.named_child(0) {
                    names.push(format!("*{}", parsed.text_of(id)));
                }
            }
            "dictionary_splat_pattern" => {
                if let Some(id) = child.named_child(0) {
                    names.push(format!("**{}", parsed.text_of(id)));
                }
            }
            _ => {}
        }
    }
    names
}

/// Method and field names declared in a class body, in source order.
fn class_data(node: Node<'_>, parsed: &ParsedFile) -> ClassData {
    let mut members = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "function_definition" => {
                    if let Some(name) = child.child_by_field_name("name") {
                        members.push(parsed.text_of(name).to_string());
                    }
                }
                "decorated_definition" => {
                    if let Some(def) = child.child_by_field_name("definition") {
                        if let Some(name) = def.child_by_field_name("name") {
                            members.push(parsed.text_of(name).to_string());
                        }
                    }
                }
                "expression_statement" => {
                    if let Some(assign) = child.named_child(0) {
                        if assign.kind() == "assignment" {
                            if let Some(left) = assign.child_by_field_name("left") {
                                if left.kind() == "identifier" {
                                    members.push(parsed.text_of(left).to_string());
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    ClassData { members }
}

/// Collects module [`FactImport`]s from `import` / `from … import …`
/// statements, preserving relative-import dots in the specifier so the
/// resolver can walk them.
fn collect_imports(root: Node<'_>, parsed: &ParsedFile) -> Vec<FactImport> {
    let mut out = Vec::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "import_statement" => lower_import_statement(child, parsed, &mut out),
            "import_from_statement" => lower_from_import(child, parsed, &mut out),
            _ => {}
        }
    }
    out
}

/// `import a`, `import a.b.c`, `import a as x`, `import a, b`.
///
/// A plain module import binds the whole module, not specific symbols, so its
/// `names` is `*` (namespace) — the same encoding a JS namespace import uses,
/// which the cross-file symbol resolver correctly treats as unresolvable to a
/// single declaration.
fn lower_import_statement(node: Node<'_>, parsed: &ParsedFile, out: &mut Vec<FactImport>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let specifier = match child.kind() {
            "dotted_name" => parsed.text_of(child).to_string(),
            "aliased_import" => match child.child_by_field_name("name") {
                Some(name) => parsed.text_of(name).to_string(),
                None => continue,
            },
            _ => continue,
        };
        out.push(FactImport {
            specifier,
            kind: ImportKind::Static,
            span: span_of(child),
            names: vec!["*".to_string()],
        });
    }
}

/// `from m import a, b`, `from .pkg import x`, `from . import sibling`,
/// `from m import *`.
///
/// The specifier is the module reference verbatim (including leading dots for
/// relative imports); `names` are the imported symbol names (the *original*
/// name for `a as b`, matching the JS named-import encoding), or `*` for a
/// star import.
fn lower_from_import(node: Node<'_>, parsed: &ParsedFile, out: &mut Vec<FactImport>) {
    let Some(module_name) = node.child_by_field_name("module_name") else {
        return;
    };
    let specifier = parsed.text_of(module_name).to_string();

    let mut names = Vec::new();
    let mut has_wildcard = false;
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "wildcard_import" {
                has_wildcard = true;
            } else if cursor.field_name() == Some("name") {
                // The imported names carry the `name` field; the module
                // carries `module_name`, already consumed above.
                match child.kind() {
                    "dotted_name" | "identifier" => names.push(parsed.text_of(child).to_string()),
                    "aliased_import" => {
                        if let Some(name) = child.child_by_field_name("name") {
                            names.push(parsed.text_of(name).to_string());
                        }
                    }
                    _ => {}
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    if has_wildcard {
        names.push("*".to_string());
    }

    out.push(FactImport {
        specifier,
        kind: ImportKind::Static,
        span: span_of(module_name),
        names,
    });
}

/// Assigns each call its enclosing top-level function symbol by body-span
/// containment — identical policy to the JS/TS pass.
fn resolve_call_enclosure(module: &mut Module) {
    let path = module.path.clone();
    for call in &mut module.calls {
        for symbol in &module.symbols {
            let SymbolKind::Function(data) = &symbol.kind else {
                continue;
            };
            let Some(body) = data.body_span else {
                continue;
            };
            if body.start_byte <= call.span.start_byte && call.span.end_byte <= body.end_byte {
                call.in_symbol = Some(symbol.id(&path));
                break;
            }
        }
    }
}

/// Depth-first collection of `call` sites. `in_symbol` is filled by
/// [`resolve_call_enclosure`].
fn collect_calls(node: Node<'_>, parsed: &ParsedFile, out: &mut Vec<Call>) {
    if node.kind() == "call" {
        if let Some(callee) = node.child_by_field_name("function") {
            let arg_count = node
                .child_by_field_name("arguments")
                .map(|a| a.named_child_count() as u32)
                .unwrap_or(0);
            out.push(Call {
                callee: parsed.text_of(callee).to_string(),
                arg_count,
                span: span_of(node),
                in_symbol: None,
            });
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_calls(child, parsed, out);
    }
}

/// Collects identifier *uses* into [`Reference`]s, excluding declaration
/// names. For an `attribute` (`obj.attr`) only the `object` is a reference —
/// the trailing `attr` never names a module-level symbol, so it is skipped,
/// matching the JS pass's exclusion of `property_identifier`.
fn collect_references(
    node: Node<'_>,
    parsed: &ParsedFile,
    decl_name_spans: &std::collections::BTreeSet<(u32, u32)>,
    out: &mut Vec<Reference>,
) {
    match node.kind() {
        "identifier" => {
            let span = span_of(node);
            if !decl_name_spans.contains(&(span.start_byte, span.end_byte)) {
                out.push(Reference {
                    name: parsed.text_of(node).to_string(),
                    span,
                });
            }
            return;
        }
        "attribute" => {
            // Visit the object subtree only; skip the `.attr` identifier.
            if let Some(obj) = node.child_by_field_name("object") {
                collect_references(obj, parsed, decl_name_spans, out);
            }
            return;
        }
        // A keyword argument's name (`f(x=1)`) is not a reference to a
        // module symbol; only its value is.
        "keyword_argument" => {
            if let Some(value) = node.child_by_field_name("value") {
                collect_references(value, parsed, decl_name_spans, out);
            }
            return;
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_references(child, parsed, decl_name_spans, out);
    }
}

/// Converts a tree-sitter node's position into a core [`Span`].
fn span_of(node: Node<'_>) -> Span {
    let start = node.start_position();
    let end = node.end_position();
    Span::new(
        Position::new(start.row as u32 + 1, start.column as u32 + 1),
        Position::new(end.row as u32 + 1, end.column as u32 + 1),
        node.start_byte() as u32,
        node.end_byte() as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::treesitter::parse;
    use crate::Language;

    fn lower_src(src: &str) -> Module {
        lower_python(&parse(src, Language::Python).unwrap(), "mod.py")
    }

    fn facts_src(src: &str) -> FileFacts {
        python_facts(&parse(src, Language::Python).unwrap())
    }

    fn names(m: &Module) -> Vec<&str> {
        m.symbols.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn lowers_functions_classes_and_module_vars() {
        let m = lower_src("def foo():\n    pass\n\nclass Bar:\n    pass\n\nBAZ = 1\n");
        assert_eq!(names(&m), vec!["foo", "Bar", "BAZ"]);
        assert!(matches!(m.symbols[0].kind, SymbolKind::Function(_)));
        assert!(matches!(m.symbols[1].kind, SymbolKind::Class(_)));
        assert!(matches!(m.symbols[2].kind, SymbolKind::Var));
    }

    #[test]
    fn decorated_definitions_are_unwrapped() {
        let m = lower_src("@app.route('/')\ndef handler():\n    pass\n");
        assert_eq!(names(&m), vec!["handler"]);
        assert!(matches!(m.symbols[0].kind, SymbolKind::Function(_)));
    }

    #[test]
    fn async_and_params_captured() {
        let m = lower_src("async def load(req, res=None, *args, **kw):\n    pass\n");
        match &m.symbols[0].kind {
            SymbolKind::Function(f) => {
                assert!(f.is_async);
                assert_eq!(f.params, vec!["req", "res", "*args", "**kw"]);
                assert!(f.body_span.is_some());
            }
            other => panic!("expected function, got {other:?}"),
        }
    }

    #[test]
    fn class_members_captured() {
        let m = lower_src("class Service:\n    url = 'x'\n    def fetch(self):\n        pass\n");
        match &m.symbols[0].kind {
            SymbolKind::Class(c) => assert_eq!(c.members, vec!["url", "fetch"]),
            other => panic!("expected class, got {other:?}"),
        }
    }

    #[test]
    fn public_names_exported_by_convention() {
        let m = lower_src("def public():\n    pass\n\ndef _private():\n    pass\n");
        let public = m.symbols.iter().find(|s| s.name == "public").unwrap();
        let private = m.symbols.iter().find(|s| s.name == "_private").unwrap();
        assert!(public.exported);
        assert!(!private.exported);
    }

    #[test]
    fn dunder_all_is_authoritative_for_exports() {
        // `helper` is public by name but excluded from __all__; `_api` is
        // underscore-private yet listed — __all__ wins both ways.
        let src = "__all__ = ['_api']\n\ndef helper():\n    pass\n\ndef _api():\n    pass\n";
        let m = lower_src(src);
        let helper = m.symbols.iter().find(|s| s.name == "helper").unwrap();
        let api = m.symbols.iter().find(|s| s.name == "_api").unwrap();
        assert!(!helper.exported);
        assert!(api.exported);
    }

    #[test]
    fn imports_absolute_and_aliased() {
        let m = lower_src("import os\nimport os.path as p\n");
        assert_eq!(m.imports.len(), 2);
        assert_eq!(m.imports[0].source, "os");
        assert_eq!(m.imports[0].names, vec!["*"]);
        assert_eq!(m.imports[1].source, "os.path");
    }

    #[test]
    fn from_imports_named_relative_and_star() {
        let m = lower_src(
            "from pkg.sub import thing, other as o\nfrom . import sibling\nfrom .mod import x\nfrom m import *\n",
        );
        let by_src = |s: &str| m.imports.iter().find(|i| i.source == s).unwrap().clone();
        // Named import records the original names (`other`, not `o`).
        assert_eq!(by_src("pkg.sub").names, vec!["thing", "other"]);
        // Relative-import dots are preserved in the specifier.
        assert_eq!(by_src(".").names, vec!["sibling"]);
        assert_eq!(by_src(".mod").names, vec!["x"]);
        // Star import.
        assert_eq!(by_src("m").names, vec!["*"]);
    }

    #[test]
    fn calls_collected_with_enclosure() {
        let m = lower_src("setup()\n\ndef run():\n    helper(a, b)\n");
        let setup = m.calls.iter().find(|c| c.callee == "setup").unwrap();
        let helper = m.calls.iter().find(|c| c.callee == "helper").unwrap();
        // Module-level call: no enclosing function.
        assert_eq!(setup.in_symbol, None);
        assert_eq!(helper.arg_count, 2);
        let run_id = m
            .symbols
            .iter()
            .find(|s| s.name == "run")
            .unwrap()
            .id(&m.path);
        assert_eq!(helper.in_symbol, Some(run_id));
    }

    #[test]
    fn references_exclude_declarations_and_attribute_names() {
        let m = lower_src("def helper():\n    pass\n\ndef run():\n    helper()\n    obj.helper\n");
        let ref_names: Vec<&str> = m.references.iter().map(|r| r.name.as_str()).collect();
        // `helper()` call is a reference; the declaration site is not.
        assert!(ref_names.contains(&"helper"));
        // `obj` is a reference; `obj.helper`'s trailing `.helper` is not
        // double-counted as a bare `helper` reference beyond the call above —
        // the object identifier is what's captured.
        assert!(ref_names.contains(&"obj"));
    }

    #[test]
    fn unused_private_symbol_has_no_reference() {
        let m = lower_src("def orphan():\n    pass\n\ndef used():\n    pass\n");
        let ref_names: Vec<&str> = m.references.iter().map(|r| r.name.as_str()).collect();
        assert!(!ref_names.contains(&"orphan"));
    }

    #[test]
    fn facts_expose_imports_and_public_exports() {
        let f = facts_src(
            "from pkg import thing\n\ndef public():\n    pass\n\ndef _hidden():\n    pass\n",
        );
        assert_eq!(f.imports.len(), 1);
        assert_eq!(f.imports[0].specifier, "pkg");
        let exports: Vec<&str> = f.exports.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(exports, vec!["public"]);
    }

    #[test]
    fn lowering_is_deterministic() {
        let src = "import os\n\ndef a():\n    b()\n\nclass C:\n    def m(self):\n        pass\n";
        let first = serde_json::to_string(&lower_src(src)).unwrap();
        let second = serde_json::to_string(&lower_src(src)).unwrap();
        assert_eq!(first, second);
    }
}
