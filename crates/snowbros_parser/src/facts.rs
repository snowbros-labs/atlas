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
    /// Top-of-file directives: `use client` and/or `use server`.
    #[serde(default)]
    pub directives: Vec<String>,
    /// Names this file exports; `default` for a default export.
    pub exports: Vec<NamedItem>,
    /// `process.env.X` / `process.env["X"]` reads.
    pub env_reads: Vec<NamedItem>,
    /// Calls to dynamic-rendering APIs (`cookies`, `headers`,
    /// `draftMode` from `next/headers`; `noStore`/`unstable_noStore`
    /// from `next/cache`) — only counted when actually imported from
    /// those modules.
    pub dynamic_api_calls: Vec<NamedItem>,
    /// `eval(...)`, `window.eval(...)`, `globalThis.eval(...)`, and
    /// `new Function(...)` sites. The name records which form was used.
    pub eval_calls: Vec<NamedItem>,
    /// Potential hardcoded secrets. `name` describes the signal (see
    /// [`SecretSignal`]); the literal value is never stored beyond a
    /// redacted preview.
    pub secret_candidates: Vec<SecretCandidate>,
}

/// Why a string literal looks like a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretSignal {
    /// The literal starts with a well-known credential prefix
    /// (`sk-`, `ghp_`, `AKIA`, `xoxb-`, …).
    KnownPrefix,
    /// A variable whose name suggests a credential is assigned a
    /// long literal.
    SuspiciousName,
}

/// A string literal that looks like a hardcoded credential.
///
/// Only a redacted preview (first 4 characters) and the length are
/// kept — reports must never leak the secret itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretCandidate {
    /// Variable/property name it was assigned to, when known.
    pub binding: Option<String>,
    /// First 4 characters of the literal.
    pub preview: String,
    /// Full length of the literal in characters.
    pub length: usize,
    /// Which heuristic fired.
    pub signal: SecretSignal,
    /// Location of the string literal.
    pub span: Span,
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
        directives: leading_directives(parsed),
        ..FileFacts::default()
    };
    collect(parsed.tree.root_node(), parsed, &dynamic_names, &mut facts);
    facts
}

/// Reads the directive prologue: leading expression statements that are
/// plain string literals (`"use client"`, `"use server"`). Stops at the
/// first real statement.
fn leading_directives(parsed: &ParsedFile) -> Vec<String> {
    let mut directives = Vec::new();
    let root = parsed.tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "comment" || child.kind() == "hash_bang_line" {
            continue;
        }
        if child.kind() != "expression_statement" {
            break;
        }
        let Some(expr) = child.named_child(0) else {
            break;
        };
        if expr.kind() != "string" {
            break;
        }
        let mut c2 = expr.walk();
        let fragment = expr
            .children(&mut c2)
            .find(|c| c.kind() == "string_fragment");
        if let Some(fragment) = fragment {
            let text = parsed.text_of(fragment);
            if text == "use client" || text == "use server" {
                directives.push(text.to_string());
            }
        }
    }
    directives
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
                if matches!(name, "eval" | "window.eval" | "globalThis.eval") {
                    facts.eval_calls.push(NamedItem {
                        name: name.to_string(),
                        span: span_of(node),
                    });
                }
            }
        }
        "new_expression" => {
            if let Some(ctor) = node.child_by_field_name("constructor") {
                if parsed.text_of(ctor) == "Function" {
                    facts.eval_calls.push(NamedItem {
                        name: "new Function".to_string(),
                        span: span_of(node),
                    });
                }
            }
        }
        "string" => {
            collect_secret_candidate(node, parsed, facts);
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(child, parsed, dynamic_names, facts);
    }
}

/// Well-known credential prefixes. A literal starting with one of these
/// (and long enough) is a secret candidate regardless of variable name.
const SECRET_PREFIXES: &[&str] = &[
    "sk-",
    "sk_live_",
    "sk_test_",
    "ghp_",
    "gho_",
    "github_pat_",
    "glpat-",
    "xoxb-",
    "xoxp-",
    "AKIA",
    "ASIA",
    "AIza",
    "ya29.",
    "npm_",
    "-----BEGIN",
];

/// Name fragments that mark a binding as credential-like. Deliberately
/// excludes bare `key` (too many `reactKey`-style false positives).
const SECRET_NAME_FRAGMENTS: &[&str] = &[
    "secret",
    "token",
    "password",
    "passwd",
    "apikey",
    "api_key",
    "private_key",
    "credential",
];

/// Obvious placeholders — never secrets.
fn is_placeholder(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    value.contains(' ')
        || value.starts_with("http")
        || value.starts_with('<')
        || value.starts_with("process.env")
        || lower.contains("example")
        || lower.contains("changeme")
        || lower.contains("placeholder")
        || lower.contains("your-")
        || lower.contains("your_")
        || lower.contains("xxx")
}

/// Name of the binding a string is assigned to, when directly inside a
/// `const x = "…"`, `{ key: "…" }`, or `x = "…"`.
fn binding_name<'t>(string_node: Node<'t>, parsed: &'t ParsedFile) -> Option<&'t str> {
    let parent = string_node.parent()?;
    match parent.kind() {
        "variable_declarator" => parent
            .child_by_field_name("name")
            .map(|n| parsed.text_of(n)),
        "pair" => parent.child_by_field_name("key").map(|n| parsed.text_of(n)),
        "assignment_expression" => parent
            .child_by_field_name("left")
            .map(|n| parsed.text_of(n)),
        _ => None,
    }
}

/// Checks one string literal against the secret heuristics.
fn collect_secret_candidate(node: Node<'_>, parsed: &ParsedFile, facts: &mut FileFacts) {
    let mut cursor = node.walk();
    let fragment = node
        .children(&mut cursor)
        .find(|c| c.kind() == "string_fragment");
    let Some(fragment) = fragment else {
        return;
    };
    let value = parsed.text_of(fragment);
    if value.len() < 8 || is_placeholder(value) {
        return;
    }

    let binding = binding_name(node, parsed);
    let signal = if value.len() >= 16 && SECRET_PREFIXES.iter().any(|p| value.starts_with(p)) {
        Some(SecretSignal::KnownPrefix)
    } else if binding.is_some_and(|name| {
        let lower = name.to_ascii_lowercase();
        SECRET_NAME_FRAGMENTS.iter().any(|f| lower.contains(f))
    }) {
        Some(SecretSignal::SuspiciousName)
    } else {
        None
    };

    if let Some(signal) = signal {
        facts.secret_candidates.push(SecretCandidate {
            binding: binding.map(str::to_string),
            preview: value.chars().take(4).collect(),
            length: value.chars().count(),
            signal,
            span: span_of(node),
        });
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
    fn use_client_directive_detected() {
        let facts = facts_of(
            "\"use client\";\nimport { useState } from \"react\";\nexport const C = () => null;",
            Language::Tsx,
        );
        assert_eq!(facts.directives, vec!["use client"]);

        // A string later in the file is not a directive.
        let none = facts_of(
            "const x = 1;\nconst s = \"use client\";",
            Language::TypeScript,
        );
        assert!(none.directives.is_empty());
    }

    #[test]
    fn eval_variants_recorded() {
        let facts = facts_of(
            r#"
eval("1+1");
window.eval(code);
globalThis.eval(code);
const f = new Function("a", "return a");
const evaluate = (x) => x * 2; evaluate(3);
"#,
            Language::JavaScript,
        );
        let names: Vec<&str> = facts.eval_calls.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["eval", "window.eval", "globalThis.eval", "new Function"]
        );
    }

    #[test]
    fn secret_prefix_detected_and_redacted() {
        let facts = facts_of(
            r#"const stripe = "sk_live_abc123def456ghi789";"#,
            Language::TypeScript,
        );
        assert_eq!(facts.secret_candidates.len(), 1);
        let c = &facts.secret_candidates[0];
        assert_eq!(c.signal, SecretSignal::KnownPrefix);
        assert_eq!(c.preview, "sk_l");
        // The full value must never be stored.
        assert!(c.preview.len() <= 4);
        assert_eq!(c.binding.as_deref(), Some("stripe"));
    }

    #[test]
    fn suspicious_name_detected() {
        let facts = facts_of(
            r#"const apiToken = "zz91jf02mfkw88ax";"#,
            Language::TypeScript,
        );
        assert_eq!(facts.secret_candidates.len(), 1);
        assert_eq!(
            facts.secret_candidates[0].signal,
            SecretSignal::SuspiciousName
        );
    }

    #[test]
    fn placeholders_and_normal_strings_ignored() {
        let facts = facts_of(
            r#"
const password = "your-password-here";
const apiToken = "example-token-123";
const label = "just a normal sentence";
const url = "https://api.example.com/v1";
const short = "abc";
const name = "zz91jf02mfkw88ax";
"#,
            Language::TypeScript,
        );
        assert!(facts.secret_candidates.is_empty());
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
