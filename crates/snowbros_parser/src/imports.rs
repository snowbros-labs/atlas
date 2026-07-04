//! Import extraction from parsed JS/TS files.
//!
//! Finds every module reference a file makes — the raw material for the
//! import graph:
//! - `import x from "mod"` / `import "mod"`
//! - `export ... from "mod"`
//! - `require("mod")`
//! - dynamic `import("mod")`
//!
//! Only string-literal specifiers are extracted. Computed specifiers
//! (`require(path)`) are deliberately skipped: reporting them would be a
//! guess, and the engine never guesses.

use serde::{Deserialize, Serialize};
use tree_sitter::Node;

use snowbros_core::{Position, Span};

use crate::treesitter::ParsedFile;

/// How a module reference is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportKind {
    /// Static `import … from "x"` or bare `import "x"`.
    Static,
    /// Re-export: `export … from "x"`.
    ReExport,
    /// CommonJS `require("x")`.
    Require,
    /// Dynamic `import("x")`.
    Dynamic,
}

/// One module reference found in a file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Import {
    /// The module specifier, verbatim without quotes, e.g. `./util` or
    /// `react`.
    pub specifier: String,
    /// How the module is referenced.
    pub kind: ImportKind,
    /// Location of the specifier string in the source.
    pub span: Span,
}

impl Import {
    /// Whether the specifier points inside the project (relative path)
    /// rather than at a package.
    pub fn is_relative(&self) -> bool {
        self.specifier.starts_with("./") || self.specifier.starts_with("../")
    }
}

/// Extracts all module references from a parsed file, in source order.
pub fn extract_imports(parsed: &ParsedFile) -> Vec<Import> {
    let mut imports = Vec::new();
    walk(parsed.tree.root_node(), parsed, &mut imports);
    imports
}

/// Depth-first walk collecting import-like constructs.
fn walk(node: Node<'_>, parsed: &ParsedFile, out: &mut Vec<Import>) {
    match node.kind() {
        "import_statement" => {
            if let Some(import) = string_field(node, "source", parsed, ImportKind::Static) {
                out.push(import);
            }
        }
        "export_statement" => {
            if let Some(import) = string_field(node, "source", parsed, ImportKind::ReExport) {
                out.push(import);
            }
        }
        "call_expression" => {
            if let Some(import) = call_import(node, parsed) {
                out.push(import);
            }
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, parsed, out);
    }
}

/// Handles `require("x")` and dynamic `import("x")` calls.
fn call_import(node: Node<'_>, parsed: &ParsedFile) -> Option<Import> {
    let callee = node.child_by_field_name("function")?;
    let kind = match parsed.text_of(callee) {
        "require" => ImportKind::Require,
        "import" => ImportKind::Dynamic,
        _ => return None,
    };
    let args = node.child_by_field_name("arguments")?;
    // First argument must be a plain string literal — anything else is a
    // computed specifier we refuse to guess about.
    let first = args.named_child(0)?;
    string_literal(first, parsed, kind)
}

/// Extracts a string field (e.g. the `source` of an import statement).
fn string_field(
    node: Node<'_>,
    field: &str,
    parsed: &ParsedFile,
    kind: ImportKind,
) -> Option<Import> {
    string_literal(node.child_by_field_name(field)?, parsed, kind)
}

/// Converts a `string` node into an [`Import`], stripping quotes.
fn string_literal(node: Node<'_>, parsed: &ParsedFile, kind: ImportKind) -> Option<Import> {
    if node.kind() != "string" {
        return None;
    }
    // The unquoted text is the `string_fragment` child; an empty string
    // (`""`) has none and is not a real module reference.
    let mut cursor = node.walk();
    let fragment = node
        .children(&mut cursor)
        .find(|c| c.kind() == "string_fragment")?;

    let start = node.start_position();
    let end = node.end_position();
    Some(Import {
        specifier: parsed.text_of(fragment).to_string(),
        kind,
        span: Span::new(
            Position::new(start.row as u32 + 1, start.column as u32 + 1),
            Position::new(end.row as u32 + 1, end.column as u32 + 1),
            node.start_byte() as u32,
            node.end_byte() as u32,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::Language;
    use crate::treesitter::parse;

    fn imports_of(src: &str, lang: Language) -> Vec<Import> {
        extract_imports(&parse(src, lang).unwrap())
    }

    #[test]
    fn static_imports() {
        let found = imports_of(
            r#"
import React from "react";
import { useState } from "react";
import "./globals.css";
"#,
            Language::TypeScript,
        );
        let specs: Vec<&str> = found.iter().map(|i| i.specifier.as_str()).collect();
        assert_eq!(specs, vec!["react", "react", "./globals.css"]);
        assert!(found.iter().all(|i| i.kind == ImportKind::Static));
    }

    #[test]
    fn re_exports() {
        let found = imports_of(r#"export { helper } from "./util";"#, Language::TypeScript);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, ImportKind::ReExport);
        assert_eq!(found[0].specifier, "./util");
    }

    #[test]
    fn require_and_dynamic_import() {
        let found = imports_of(
            r#"
const fs = require("fs");
const mod = await import("./lazy");
"#,
            Language::JavaScript,
        );
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].kind, ImportKind::Require);
        assert_eq!(found[0].specifier, "fs");
        assert_eq!(found[1].kind, ImportKind::Dynamic);
        assert_eq!(found[1].specifier, "./lazy");
    }

    #[test]
    fn computed_specifiers_are_skipped() {
        let found = imports_of(
            r#"
const name = "./a";
require(name);
import(prefix + "/mod");
"#,
            Language::JavaScript,
        );
        assert!(found.is_empty());
    }

    #[test]
    fn relative_detection() {
        let found = imports_of(
            r#"
import a from "./local";
import b from "../up";
import c from "pkg";
"#,
            Language::TypeScript,
        );
        assert!(found[0].is_relative());
        assert!(found[1].is_relative());
        assert!(!found[2].is_relative());
    }

    #[test]
    fn spans_are_one_based() {
        let found = imports_of(r#"import x from "y";"#, Language::TypeScript);
        assert_eq!(found[0].span.start.line, 1);
        assert!(found[0].span.start.column > 1);
    }

    #[test]
    fn tsx_imports_work() {
        let found = imports_of(
            r#"
import { Button } from "@/components/ui/button";
export default function Page() { return <Button />; }
"#,
            Language::Tsx,
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].specifier, "@/components/ui/button");
    }
}
