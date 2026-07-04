//! Whole-file fact extraction — everything analyzers need from one file
//! in a single, cacheable structure.
//!
//! [`FileFacts`] is the unit the incremental cache stores: imports,
//! exported names, `process.env` reads, and calls to Next.js APIs that
//! force dynamic rendering.

use serde::{Deserialize, Serialize};
use tree_sitter::Node;

use snowbros_core::{Position, Span};

use crate::imports::{extract_imports, Import};
use crate::treesitter::ParsedFile;

/// A named thing at a location (export, env read, call site).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedItem {
    /// The name (export name, env var name, called function name).
    pub name: String,
    /// Where it appears.
    pub span: Span,
}

/// Everything the engine extracts from one parsed file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFacts {
    /// Module references (see [`Import`]).
    pub imports: Vec<Import>,
    /// Names this file exports; `default` for a default export.
    pub exports: Vec<NamedItem>,
    /// `process.env.X` / `process.env["X"]` reads.
    pub env_reads: Vec<NamedItem>,
    /// Calls to dynamic-rendering APIs (`cookies`, `headers`,
    /// `draftMode` from `next/headers`; `noStore`/`unstable_noStore`
    /// from `next/cache`) — only counted when actually imported from
    /// those modules.
    pub dynamic_api_calls: Vec<NamedItem>,
}

/// Next.js modules whose named imports force dynamic rendering when
/// called.
const DYNAMIC_API_SOURCES: &[(&str, &[&str])] = &[
    ("next/headers", &["cookies", "headers", "draftMode"]),
    ("next/cache", &["noStore", "unstable_noStore"]),
];

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

/// Extracts all facts from a parsed file.
pub fn extract_facts(parsed: &ParsedFile) -> FileFacts {
    let imports = extract_imports(parsed);

    // Which dynamic-API names are actually imported from the right
    // modules in this file (unaliased imports; aliased dynamic-API
    // imports are rare and deliberately not guessed at).
    let mut dynamic_names: Vec<&str> = Vec::new();
    for import in &imports {
        for (source, names) in DYNAMIC_API_SOURCES {
            if import.specifier == *source {
                for n in *names {
                    if import.names.iter().any(|i| i == n) {
                        dynamic_names.push(n);
                    }
                }
            }
        }
    }

    let mut facts = FileFacts {
        imports,
        ..FileFacts::default()
    };
    collect(parsed.tree.root_node(), parsed, &dynamic_names, &mut facts);
    facts
}

/// Depth-first collection of exports, env reads, and dynamic API calls.
fn collect(node: Node<'_>, parsed: &ParsedFile, dynamic_names: &[&str], facts: &mut FileFacts) {
    match node.kind() {
        // Exports without a source (re-exports are handled as imports).
        "export_statement" if node.child_by_field_name("source").is_none() => {
            collect_export_names(node, parsed, facts);
        }
        "member_expression" => {
            // process.env.X
            if let (Some(object), Some(property)) = (
                node.child_by_field_name("object"),
                node.child_by_field_name("property"),
            ) {
                if parsed.text_of(object) == "process.env"
                    && property.kind() == "property_identifier"
                {
                    facts.env_reads.push(NamedItem {
                        name: parsed.text_of(property).to_string(),
                        span: span_of(node),
                    });
                }
            }
        }
        "subscript_expression" => {
            // process.env["X"]
            if let (Some(object), Some(index)) = (
                node.child_by_field_name("object"),
                node.child_by_field_name("index"),
            ) {
                if parsed.text_of(object) == "process.env" && index.kind() == "string" {
                    let mut cursor = index.walk();
                    let fragment = index
                        .children(&mut cursor)
                        .find(|c| c.kind() == "string_fragment");
                    if let Some(fragment) = fragment {
                        facts.env_reads.push(NamedItem {
                            name: parsed.text_of(fragment).to_string(),
                            span: span_of(node),
                        });
                    }
                }
            }
        }
        "call_expression" => {
            if let Some(callee) = node.child_by_field_name("function") {
                let name = parsed.text_of(callee);
                if dynamic_names.contains(&name) {
                    facts.dynamic_api_calls.push(NamedItem {
                        name: name.to_string(),
                        span: span_of(node),
                    });
                }
            }
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(child, parsed, dynamic_names, facts);
    }
}

/// Handles `export const x`, `export function f`, `export class C`,
/// `export { a, b as c }`, `export default …`.
fn collect_export_names(node: Node<'_>, parsed: &ParsedFile, facts: &mut FileFacts) {
    // export default …
    let mut cursor = node.walk();
    if node.children(&mut cursor).any(|c| c.kind() == "default") {
        facts.exports.push(NamedItem {
            name: "default".to_string(),
            span: span_of(node),
        });
        return;
    }

    if let Some(decl) = node.child_by_field_name("declaration") {
        match decl.kind() {
            "function_declaration"
            | "class_declaration"
            | "generator_function_declaration"
            | "abstract_class_declaration"
            | "type_alias_declaration"
            | "interface_declaration"
            | "enum_declaration" => {
                if let Some(name) = decl.child_by_field_name("name") {
                    facts.exports.push(NamedItem {
                        name: parsed.text_of(name).to_string(),
                        span: span_of(name),
                    });
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                let mut c2 = decl.walk();
                for declarator in decl.children(&mut c2) {
                    if declarator.kind() == "variable_declarator" {
                        if let Some(name) = declarator.child_by_field_name("name") {
                            // Plain identifiers only; destructured
                            // exports are skipped rather than guessed.
                            if name.kind() == "identifier" {
                                facts.exports.push(NamedItem {
                                    name: parsed.text_of(name).to_string(),
                                    span: span_of(name),
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        return;
    }

    // export { a, b as c } — the exported (outer) name is the alias when
    // present, otherwise the local name.
    let mut c2 = node.walk();
    for child in node.children(&mut c2) {
        if child.kind() == "export_clause" {
            let mut c3 = child.walk();
            for spec in child.children(&mut c3) {
                if spec.kind() == "export_specifier" {
                    let exported = spec
                        .child_by_field_name("alias")
                        .or_else(|| spec.child_by_field_name("name"));
                    if let Some(n) = exported {
                        facts.exports.push(NamedItem {
                            name: parsed.text_of(n).to_string(),
                            span: span_of(n),
                        });
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::Language;
    use crate::treesitter::parse;

    fn facts_of(src: &str, lang: Language) -> FileFacts {
        extract_facts(&parse(src, lang).unwrap())
    }

    #[test]
    fn named_imports_recorded() {
        let facts = facts_of(
            r#"
import React from "react";
import { useState, useEffect as ue } from "react";
import * as path from "node:path";
"#,
            Language::TypeScript,
        );
        assert_eq!(facts.imports[0].names, vec!["default"]);
        assert_eq!(facts.imports[1].names, vec!["useState", "useEffect"]);
        assert_eq!(facts.imports[2].names, vec!["*"]);
    }

    #[test]
    fn exports_recorded() {
        let facts = facts_of(
            r#"
export const alpha = 1;
export function beta() {}
export class Gamma {}
const hidden = 2;
export { hidden as delta };
export default function main() {}
export interface Shape { x: number }
export type Alias = string;
"#,
            Language::TypeScript,
        );
        let names: Vec<&str> = facts.exports.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["alpha", "beta", "Gamma", "delta", "default", "Shape", "Alias"]
        );
    }

    #[test]
    fn env_reads_recorded() {
        let facts = facts_of(
            r#"
const a = process.env.DATABASE_URL;
const b = process.env["API_KEY"];
const c = other.env.NOPE;
"#,
            Language::TypeScript,
        );
        let names: Vec<&str> = facts.env_reads.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["DATABASE_URL", "API_KEY"]);
    }

    #[test]
    fn dynamic_api_calls_require_the_import() {
        let with_import = facts_of(
            r#"
import { cookies } from "next/headers";
export async function Page() { const jar = cookies(); return jar; }
"#,
            Language::TypeScript,
        );
        assert_eq!(with_import.dynamic_api_calls.len(), 1);
        assert_eq!(with_import.dynamic_api_calls[0].name, "cookies");

        // Same call, no import from next/headers: a local helper named
        // cookies() must NOT count.
        let without = facts_of(
            r#"
function cookies() { return 1; }
export function Page() { return cookies(); }
"#,
            Language::TypeScript,
        );
        assert!(without.dynamic_api_calls.is_empty());
    }

    #[test]
    fn re_export_names_recorded_on_import() {
        let facts = facts_of(
            r#"export { helper, other } from "./util"; export * from "./all";"#,
            Language::TypeScript,
        );
        assert_eq!(facts.imports[0].names, vec!["helper", "other"]);
        assert_eq!(facts.imports[1].names, vec!["*"]);
    }
}
