//! React rules (M1), built on the semantic React classification.
//!
//! The first increment ships a single high-confidence, purely structural
//! rule. It combines three deterministic signals — the semantic
//! component classification, the IR `is_async` flag, and the file's
//! `"use client"` directive — with no flow analysis, so it never guesses.

use snowbros_core::{Confidence, Diagnostic, Evidence, Severity, SourceLocation};
use snowbros_ir::SymbolKind;

use crate::context::AnalysisContext;
use crate::registry::Rule;

/// `react/async-client-component` — a Client Component (`"use client"`)
/// declared `async`.
///
/// Only Server Components may be `async`. An `async` Client Component is
/// invalid: React cannot render a promise on the client, and the app
/// errors at runtime. This is a structural fact (component + async +
/// client directive), hence [`Confidence::Certain`].
pub struct AsyncClientComponent;

impl Rule for AsyncClientComponent {
    fn id(&self) -> &'static str {
        "react/async-client-component"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let Some(model) = ctx.semantic else {
            return Vec::new();
        };
        let mut diagnostics = Vec::new();
        for component in model.react_components() {
            let SymbolKind::Function(data) = &component.symbol.kind else {
                continue;
            };
            if !data.is_async {
                continue;
            }
            // The declaring file must carry a top-of-file `"use client"`.
            let is_client = ctx
                .file_facts
                .get(component.module)
                .is_some_and(|f| f.directives.iter().any(|d| d == "use client"));
            if !is_client {
                continue;
            }
            let name = &component.symbol.name;
            diagnostics.push(
                Diagnostic::new(
                    self.id(),
                    "Async Client Component",
                    format!(
                        "Component `{name}` in `{}` is `async`, but the file is a \
                         Client Component (\"use client\"). Only Server Components \
                         may be async — this errors at runtime.",
                        component.module
                    ),
                    "react",
                    Severity::High,
                    Confidence::Certain,
                    SourceLocation::new(component.module.to_owned(), component.symbol.span),
                )
                .with_evidence(Evidence::note(format!(
                    "`{name}` is an async JSX-returning function in a \"use client\" file"
                ))),
            );
        }
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ContextInputs;
    use camino::Utf8PathBuf;
    use snowbros_graph::SemanticGraph;
    use snowbros_parser::{extract_facts, lower, parse, FileFacts, Language};
    use snowbros_semantic::SemanticModel;
    use std::collections::BTreeMap;

    fn setup(path: &str, src: &str) -> (SemanticModel, BTreeMap<Utf8PathBuf, FileFacts>) {
        let parsed = parse(src, Language::Tsx).unwrap();
        let model = SemanticModel::from_modules([lower(&parsed, path)]);
        let mut facts = BTreeMap::new();
        facts.insert(Utf8PathBuf::from(path), extract_facts(&parsed));
        (model, facts)
    }

    fn run(model: &SemanticModel, facts: BTreeMap<Utf8PathBuf, FileFacts>) -> Vec<Diagnostic> {
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(
            &g,
            facts,
            ContextInputs {
                semantic: Some(model),
                ..ContextInputs::default()
            },
        );
        AsyncClientComponent.run(&ctx)
    }

    #[test]
    fn async_client_component_flagged() {
        let (m, f) = setup(
            "app/widget.tsx",
            "\"use client\";\nexport default async function Widget() { return <div/>; }",
        );
        let diags = run(&m, f);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("async"));
        assert_eq!(diags[0].severity, Severity::High);
    }

    #[test]
    fn async_server_component_not_flagged() {
        // No "use client" → Server Component → async is valid.
        let (m, f) = setup(
            "app/page.tsx",
            "export default async function Page() { return <div/>; }",
        );
        assert!(run(&m, f).is_empty());
    }

    #[test]
    fn sync_client_component_not_flagged() {
        let (m, f) = setup(
            "app/widget.tsx",
            "\"use client\";\nexport function Widget() { return <div/>; }",
        );
        assert!(run(&m, f).is_empty());
    }

    #[test]
    fn async_client_non_component_not_flagged() {
        // Async, client, but not a component (no JSX) → out of scope.
        let (m, f) = setup(
            "app/data.ts",
            "\"use client\";\nexport async function load() { return 1; }",
        );
        assert!(run(&m, f).is_empty());
    }
}
