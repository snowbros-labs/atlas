//! Lowering: tree-sitter CST → Atlas IR.
//!
//! This is the one place JS/TS syntax is translated into the
//! language-agnostic [`snowbros_ir::Module`]. Everything downstream reads
//! IR, so a Python or Go lowering pass added later plugs in here without
//! touching the semantic layer or any rule.
//!
//! Scope for v0.2.0 (grows per milestone):
//! - top-level declared [`Symbol`]s (functions, classes, `const`/`let`/`var`
//!   bindings) with their exported-ness;
//! - module [`Import`]s (reusing [`crate::extract_imports`]);
//! - [`Call`] sites, for call-graph construction in the semantic layer.
//!
//! What this pass deliberately does **not** do: decide meaning. Whether a
//! function is a React component or a hook, whether a call resolves to a
//! particular declaration — that is the semantic layer's job. Lowering is
//! mechanical and structural.
//!
//! Determinism: nodes are emitted in source order (the order tree-sitter
//! yields children), never sorted or deduplicated here.

use camino::Utf8PathBuf;
use tree_sitter::Node;

use snowbros_core::{Position, Span};
use snowbros_ir::{Call, ClassData, FunctionData, Import, Module, Symbol, SymbolKind};

use crate::imports::extract_imports;
use crate::treesitter::ParsedFile;

/// Lowers a parsed JS/TS file into an Atlas IR [`Module`].
pub fn lower(parsed: &ParsedFile, path: impl Into<Utf8PathBuf>) -> Module {
    let mut module = Module::new(path);

    // Imports: reuse the import extractor, projecting to the IR shape.
    module.imports = extract_imports(parsed)
        .into_iter()
        .map(|i| Import {
            source: i.specifier,
            names: i.names,
            span: i.span,
        })
        .collect();

    let root = parsed.tree.root_node();

    // Symbols: only top-level declarations. `export` wrappers are unwrapped
    // so the declaration inside is recorded once, marked exported.
    let mut exported_names: Vec<String> = Vec::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "export_statement" => {
                lower_export_statement(child, parsed, &mut module, &mut exported_names)
            }
            _ => lower_declaration(child, parsed, false, &mut module.symbols),
        }
    }

    // A bare `export { a, b as c }` marks already-declared locals as
    // exported. The exported name is the local (inner) name.
    for sym in &mut module.symbols {
        if exported_names.iter().any(|n| n == &sym.name) {
            sym.exported = true;
        }
    }

    // Calls: flat, whole-tree. The enclosing symbol is left unresolved here
    // (`in_symbol: None`) — the semantic layer assigns it by span
    // containment when it builds the call graph.
    collect_calls(root, parsed, &mut module.calls);

    module
}

/// Handles an `export …` statement: `export <decl>`, `export default …`,
/// and `export { … }` clauses.
fn lower_export_statement(
    node: Node<'_>,
    parsed: &ParsedFile,
    module: &mut Module,
    exported_names: &mut Vec<String>,
) {
    // `export default …`
    let mut cursor = node.walk();
    let has_default = node.children(&mut cursor).any(|c| c.kind() == "default");

    if let Some(decl) = node.child_by_field_name("declaration") {
        if has_default {
            // `export default function/class …` — recorded as `default`.
            if let Some(sym) = default_symbol(decl, parsed, node) {
                module.symbols.push(sym);
            }
        } else {
            lower_declaration(decl, parsed, true, &mut module.symbols);
        }
        return;
    }

    if has_default {
        // `export default <expression>` (identifier, call, object, …).
        module.symbols.push(Symbol {
            name: "default".to_string(),
            kind: SymbolKind::Unknown,
            span: span_of(node),
            exported: true,
        });
        return;
    }

    // `export { a, b as c }` — collect the local (inner) names.
    let mut c2 = node.walk();
    for child in node.children(&mut c2) {
        if child.kind() == "export_clause" {
            let mut c3 = child.walk();
            for spec in child.children(&mut c3) {
                if spec.kind() == "export_specifier" {
                    if let Some(n) = spec.child_by_field_name("name") {
                        exported_names.push(parsed.text_of(n).to_string());
                    }
                }
            }
        }
    }
}

/// Builds the `default` symbol for `export default function/class …`.
fn default_symbol(decl: Node<'_>, parsed: &ParsedFile, export_node: Node<'_>) -> Option<Symbol> {
    let kind = match decl.kind() {
        "function_declaration" | "generator_function_declaration" | "function" => {
            SymbolKind::Function(function_data(decl, parsed))
        }
        "class_declaration" | "abstract_class_declaration" => {
            SymbolKind::Class(class_data(decl, parsed))
        }
        _ => return None,
    };
    Some(Symbol {
        name: "default".to_string(),
        kind,
        span: span_of(export_node),
        exported: true,
    })
}

/// Lowers a single declaration node into zero or more symbols.
fn lower_declaration(node: Node<'_>, parsed: &ParsedFile, exported: bool, out: &mut Vec<Symbol>) {
    match node.kind() {
        "function_declaration" | "generator_function_declaration" => {
            if let Some(name) = node.child_by_field_name("name") {
                out.push(Symbol {
                    name: parsed.text_of(name).to_string(),
                    kind: SymbolKind::Function(function_data(node, parsed)),
                    span: span_of(name),
                    exported,
                });
            }
        }
        "class_declaration" | "abstract_class_declaration" => {
            if let Some(name) = node.child_by_field_name("name") {
                out.push(Symbol {
                    name: parsed.text_of(name).to_string(),
                    kind: SymbolKind::Class(class_data(node, parsed)),
                    span: span_of(name),
                    exported,
                });
            }
        }
        "lexical_declaration" | "variable_declaration" => {
            let decl_kind = binding_kind(node, parsed);
            let mut cursor = node.walk();
            for declarator in node.children(&mut cursor) {
                if declarator.kind() == "variable_declarator" {
                    lower_declarator(declarator, parsed, decl_kind.clone(), exported, out);
                }
            }
        }
        _ => {}
    }
}

/// Lowers one `variable_declarator`. If its initializer is a function form
/// the symbol becomes a `Function`; otherwise it keeps the binding kind.
fn lower_declarator(
    node: Node<'_>,
    parsed: &ParsedFile,
    binding: SymbolKind,
    exported: bool,
    out: &mut Vec<Symbol>,
) {
    let Some(name) = node.child_by_field_name("name") else {
        return;
    };
    // Only plain identifiers are lowered as named symbols; destructured
    // bindings (`const { a } = x`) are not split apart or guessed.
    if name.kind() != "identifier" {
        return;
    }
    let kind = match node.child_by_field_name("value") {
        Some(value) if is_function_value(value) => {
            SymbolKind::Function(function_data(value, parsed))
        }
        _ => binding,
    };
    out.push(Symbol {
        name: parsed.text_of(name).to_string(),
        kind,
        span: span_of(name),
        exported,
    });
}

/// Whether a `variable_declarator` value node is a function form.
fn is_function_value(value: Node<'_>) -> bool {
    matches!(
        value.kind(),
        "arrow_function" | "function" | "function_expression" | "generator_function"
    )
}

/// `const`/`let`/`var` for a declaration node.
fn binding_kind(node: Node<'_>, parsed: &ParsedFile) -> SymbolKind {
    if node.kind() == "variable_declaration" {
        return SymbolKind::Var;
    }
    // lexical_declaration begins with a `const` or `let` token.
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match parsed.text_of(child) {
            "const" => return SymbolKind::Const,
            "let" => return SymbolKind::Let,
            _ => {}
        }
    }
    SymbolKind::Let
}

/// Extracts [`FunctionData`] from any function-form node.
fn function_data(node: Node<'_>, parsed: &ParsedFile) -> FunctionData {
    let params = node
        .child_by_field_name("parameters")
        .map(|p| param_names(p, parsed))
        .unwrap_or_default();
    let mut async_cursor = node.walk();
    let is_async = node
        .children(&mut async_cursor)
        .any(|c| c.kind() == "async");
    let body_span = node.child_by_field_name("body").map(span_of);
    FunctionData {
        params,
        is_async,
        returns_jsx: false, // populated in the React milestone (M1)
        body_span,
    }
}

/// Parameter names in order. Destructured parameters are recorded as `{}`
/// / `[]` rather than guessed apart.
fn param_names(params: Node<'_>, parsed: &ParsedFile) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = params.walk();
    for child in params.children(&mut cursor) {
        match child.kind() {
            "identifier" => names.push(parsed.text_of(child).to_string()),
            // TS wraps the pattern: `required_parameter`/`optional_parameter`
            // carry a `pattern` field.
            "required_parameter" | "optional_parameter" => {
                if let Some(pat) = child.child_by_field_name("pattern") {
                    names.push(pattern_name(pat, parsed));
                }
            }
            "object_pattern" => names.push("{}".to_string()),
            "array_pattern" => names.push("[]".to_string()),
            "rest_pattern" => names.push("...".to_string()),
            _ => {}
        }
    }
    names
}

/// Best-effort name for a parameter pattern.
fn pattern_name(pat: Node<'_>, parsed: &ParsedFile) -> String {
    match pat.kind() {
        "identifier" => parsed.text_of(pat).to_string(),
        "object_pattern" => "{}".to_string(),
        "array_pattern" => "[]".to_string(),
        "rest_pattern" => "...".to_string(),
        _ => parsed.text_of(pat).to_string(),
    }
}

/// Method and field names declared on a class body, in source order.
fn class_data(node: Node<'_>, parsed: &ParsedFile) -> ClassData {
    let mut members = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "method_definition" | "public_field_definition" | "field_definition" => {
                    if let Some(name) = child.child_by_field_name("name") {
                        members.push(parsed.text_of(name).to_string());
                    }
                }
                _ => {}
            }
        }
    }
    ClassData { members }
}

/// Depth-first collection of call sites. `in_symbol` is intentionally left
/// `None` — enclosure is resolved by the semantic layer.
fn collect_calls(node: Node<'_>, parsed: &ParsedFile, out: &mut Vec<Call>) {
    if node.kind() == "call_expression" {
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
    use crate::language::Language;
    use crate::treesitter::parse;

    fn lower_src(src: &str, lang: Language, path: &str) -> Module {
        lower(&parse(src, lang).unwrap(), path)
    }

    fn names(m: &Module) -> Vec<&str> {
        m.symbols.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn lowers_top_level_declarations() {
        let m = lower_src(
            r#"
function foo() {}
class Bar {}
const baz = 1;
let qux = 2;
var old = 3;
"#,
            Language::TypeScript,
            "a.ts",
        );
        assert_eq!(names(&m), vec!["foo", "Bar", "baz", "qux", "old"]);
        assert!(matches!(m.symbols[0].kind, SymbolKind::Function(_)));
        assert!(matches!(m.symbols[1].kind, SymbolKind::Class(_)));
        assert!(matches!(m.symbols[2].kind, SymbolKind::Const));
        assert!(matches!(m.symbols[3].kind, SymbolKind::Let));
        assert!(matches!(m.symbols[4].kind, SymbolKind::Var));
        assert!(m.symbols.iter().all(|s| !s.exported));
    }

    #[test]
    fn arrow_binding_is_a_function() {
        let m = lower_src("const add = (a, b) => a + b;", Language::TypeScript, "a.ts");
        assert_eq!(names(&m), vec!["add"]);
        match &m.symbols[0].kind {
            SymbolKind::Function(f) => assert_eq!(f.params, vec!["a", "b"]),
            other => panic!("expected function, got {other:?}"),
        }
    }

    #[test]
    fn exported_declarations_flagged() {
        let m = lower_src(
            r#"
export function a() {}
export const b = 1;
function c() {}
"#,
            Language::TypeScript,
            "a.ts",
        );
        let exported: Vec<&str> = m
            .symbols
            .iter()
            .filter(|s| s.exported)
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(exported, vec!["a", "b"]);
    }

    #[test]
    fn export_clause_marks_locals_exported() {
        let m = lower_src(
            r#"
const helper = 1;
const hidden = 2;
export { helper };
"#,
            Language::TypeScript,
            "a.ts",
        );
        let helper = m.symbols.iter().find(|s| s.name == "helper").unwrap();
        let hidden = m.symbols.iter().find(|s| s.name == "hidden").unwrap();
        assert!(helper.exported);
        assert!(!hidden.exported);
    }

    #[test]
    fn default_function_export_named_default() {
        let m = lower_src(
            "export default function Page() { return 1; }",
            Language::Tsx,
            "page.tsx",
        );
        assert_eq!(names(&m), vec!["default"]);
        assert!(m.symbols[0].exported);
        assert!(matches!(m.symbols[0].kind, SymbolKind::Function(_)));
    }

    #[test]
    fn default_expression_export_is_unknown_kind() {
        let m = lower_src(
            "const x = 1; export default x;",
            Language::TypeScript,
            "a.ts",
        );
        let def = m.symbols.iter().find(|s| s.name == "default").unwrap();
        assert!(def.exported);
        assert!(matches!(def.kind, SymbolKind::Unknown));
    }

    #[test]
    fn async_and_params_captured() {
        let m = lower_src(
            "export async function load(req, res) {}",
            Language::TypeScript,
            "a.ts",
        );
        match &m.symbols[0].kind {
            SymbolKind::Function(f) => {
                assert!(f.is_async);
                assert_eq!(f.params, vec!["req", "res"]);
                assert!(f.body_span.is_some());
            }
            other => panic!("expected function, got {other:?}"),
        }
    }

    #[test]
    fn destructured_params_are_not_split() {
        let m = lower_src(
            "function C({ title, onClose }, [first]) {}",
            Language::TypeScript,
            "a.ts",
        );
        match &m.symbols[0].kind {
            SymbolKind::Function(f) => assert_eq!(f.params, vec!["{}", "[]"]),
            other => panic!("expected function, got {other:?}"),
        }
    }

    #[test]
    fn class_members_captured() {
        let m = lower_src(
            r#"
class Service {
  url = "x";
  fetch() {}
  save(data) {}
}
"#,
            Language::TypeScript,
            "a.ts",
        );
        match &m.symbols[0].kind {
            SymbolKind::Class(c) => assert_eq!(c.members, vec!["url", "fetch", "save"]),
            other => panic!("expected class, got {other:?}"),
        }
    }

    #[test]
    fn calls_collected_with_arg_counts() {
        let m = lower_src(
            r#"
import { useState } from "react";
export function C() {
  const [n, setN] = useState(0);
  doThing(a, b, c);
}
"#,
            Language::Tsx,
            "c.tsx",
        );
        let callees: Vec<(&str, u32)> = m
            .calls
            .iter()
            .map(|c| (c.callee.as_str(), c.arg_count))
            .collect();
        assert!(callees.contains(&("useState", 1)));
        assert!(callees.contains(&("doThing", 3)));
        assert!(m.calls.iter().all(|c| c.in_symbol.is_none()));
    }

    #[test]
    fn imports_projected_to_ir() {
        let m = lower_src(
            r#"import React, { useState } from "react";"#,
            Language::Tsx,
            "c.tsx",
        );
        assert_eq!(m.imports.len(), 1);
        assert_eq!(m.imports[0].source, "react");
        assert_eq!(m.imports[0].names, vec!["default", "useState"]);
    }

    #[test]
    fn lowering_is_deterministic() {
        let src = r#"
export function a() { helper(); }
const b = () => 2;
export default class D { m() {} }
"#;
        let first = serde_json::to_string(&lower_src(src, Language::Tsx, "x.tsx")).unwrap();
        let second = serde_json::to_string(&lower_src(src, Language::Tsx, "x.tsx")).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn stable_ids_resolve() {
        let m = lower_src(
            "export function Page() {}",
            Language::Tsx,
            "src/app/page.tsx",
        );
        let id = m.symbols[0].id(&m.path);
        assert_eq!(id.as_str(), "src/app/page.tsx#function#Page@16-20");
    }
}
