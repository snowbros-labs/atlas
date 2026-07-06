//! React rules (M1), built on the semantic React classification.
//!
//! The first increment ships a single high-confidence, purely structural
//! rule. It combines three deterministic signals — the semantic
//! component classification, the IR `is_async` flag, and the file's
//! `"use client"` directive — with no flow analysis, so it never guesses.

use snowbros_core::{Confidence, Diagnostic, Evidence, Severity, SourceLocation};
use snowbros_ir::SymbolKind;
use snowbros_semantic::{is_hook_name, role_of, ReactRole};

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

/// `react/component-naming` — a function that returns JSX but is neither
/// PascalCase (a component) nor a `useX` hook. React treats a lowercase
/// tag as a host element, so a JSX-returning helper used as `<x/>` renders
/// nothing. Confidence is [`Confidence::Possible`]: a helper called
/// directly (`row()`) is legitimate.
pub struct ComponentNaming;

impl Rule for ComponentNaming {
    fn id(&self) -> &'static str {
        "react/component-naming"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let Some(model) = ctx.semantic else {
            return Vec::new();
        };
        let mut diagnostics = Vec::new();
        for sym in model.symbols() {
            let SymbolKind::Function(data) = &sym.symbol.kind else {
                continue;
            };
            // JSX-returning but unclassified → camelCase/lowercase helper.
            if !data.returns_jsx || role_of(sym).is_some() {
                continue;
            }
            let name = &sym.symbol.name;
            diagnostics.push(
                Diagnostic::new(
                    self.id(),
                    "Component not PascalCase",
                    format!(
                        "`{name}` in `{}` returns JSX but is not PascalCase. React \
                         treats a lowercase name as a host element, so it will not \
                         render as a component when used as `<{name}/>`.",
                        sym.module
                    ),
                    "react",
                    Severity::Low,
                    Confidence::Possible,
                    SourceLocation::new(sym.module.to_owned(), sym.symbol.span),
                )
                .with_evidence(Evidence::note(format!(
                    "`{name}` returns JSX but its name does not start with a capital"
                ))),
            );
        }
        diagnostics
    }
}

/// `react/hook-returns-jsx` — a `useX` hook whose body returns JSX. Hooks
/// return state/handlers, not markup; a JSX-returning `useX` is almost
/// always a component mislabeled with the `use` prefix.
pub struct HookReturnsJsx;

impl Rule for HookReturnsJsx {
    fn id(&self) -> &'static str {
        "react/hook-returns-jsx"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let Some(model) = ctx.semantic else {
            return Vec::new();
        };
        let mut diagnostics = Vec::new();
        for hook in model.react_hooks() {
            let SymbolKind::Function(data) = &hook.symbol.kind else {
                continue;
            };
            if !data.returns_jsx {
                continue;
            }
            let name = &hook.symbol.name;
            diagnostics.push(
                Diagnostic::new(
                    self.id(),
                    "Hook returns JSX",
                    format!(
                        "`{name}` in `{}` is named as a hook (`use` prefix) but returns \
                         JSX. Hooks return values, not markup — rename it to a \
                         PascalCase component or stop returning JSX.",
                        hook.module
                    ),
                    "react",
                    Severity::Medium,
                    Confidence::Likely,
                    SourceLocation::new(hook.module.to_owned(), hook.symbol.span),
                )
                .with_evidence(Evidence::note(format!(
                    "`{name}` matches the hook naming contract yet returns JSX"
                ))),
            );
        }
        diagnostics
    }
}

/// `react/hook-in-non-component` — a hook call (`useX(...)`) that is not
/// enclosed by a component or another hook. The first Rule of Hooks: hooks
/// may only be called from React function components or custom hooks.
///
/// Enclosure is resolved to the nearest top-level declaration, so nested
/// closures inside a component correctly resolve to the component. A call
/// enclosed by nothing top-level is a module-level hook call, also invalid.
pub struct HookInNonComponent;

impl Rule for HookInNonComponent {
    fn id(&self) -> &'static str {
        "react/hook-in-non-component"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let Some(model) = ctx.semantic else {
            return Vec::new();
        };
        let mut diagnostics = Vec::new();
        for module in model.modules() {
            for call in &module.calls {
                if !is_hook_name(&call.callee) {
                    continue;
                }
                let enclosing =
                    model.enclosing_symbol(&module.path, call.span.start_byte, call.span.end_byte);
                let enclosing_role = enclosing.and_then(role_of);
                if matches!(
                    enclosing_role,
                    Some(ReactRole::Component) | Some(ReactRole::Hook)
                ) {
                    continue; // valid: called from a component or a hook
                }
                let context = match &enclosing {
                    Some(sym) => format!("`{}`, which is not a component or hook", sym.symbol.name),
                    None => "module top level".to_string(),
                };
                diagnostics.push(
                    Diagnostic::new(
                        self.id(),
                        "Hook called outside a component",
                        format!(
                            "`{}` is called from {context} in `{}`. Hooks may only be \
                             called from React components or custom hooks (the Rules of \
                             Hooks).",
                            call.callee, module.path
                        ),
                        "react",
                        Severity::High,
                        Confidence::Likely,
                        SourceLocation::new(module.path.clone(), call.span),
                    )
                    .with_evidence(Evidence::note(format!(
                        "hook `{}` invoked from {context}",
                        call.callee
                    ))),
                );
            }
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

    fn model_of(path: &str, src: &str) -> SemanticModel {
        SemanticModel::from_modules([lower(&parse(src, Language::Tsx).unwrap(), path)])
    }

    fn run_semantic<R: Rule>(rule: R, model: &SemanticModel) -> Vec<Diagnostic> {
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(
            &g,
            BTreeMap::new(),
            ContextInputs {
                semantic: Some(model),
                ..ContextInputs::default()
            },
        );
        rule.run(&ctx)
    }

    #[test]
    fn lowercase_jsx_helper_flagged_by_naming() {
        let m = model_of("src/ui.tsx", "export function row() { return <tr/>; }");
        let diags = run_semantic(ComponentNaming, &m);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("PascalCase"));
    }

    #[test]
    fn pascal_component_not_flagged_by_naming() {
        let m = model_of("src/ui.tsx", "export function Row() { return <tr/>; }");
        assert!(run_semantic(ComponentNaming, &m).is_empty());
    }

    #[test]
    fn hook_returning_jsx_flagged() {
        let m = model_of("src/h.tsx", "export function useThing() { return <div/>; }");
        let diags = run_semantic(HookReturnsJsx, &m);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("hook"));
    }

    #[test]
    fn hook_returning_value_not_flagged() {
        let m = model_of("src/h.tsx", "export function useThing() { return 0; }");
        assert!(run_semantic(HookReturnsJsx, &m).is_empty());
    }

    #[test]
    fn hook_in_plain_function_flagged() {
        let m = model_of(
            "src/bad.tsx",
            "export function setup() { const [n] = useState(0); return n; }",
        );
        let diags = run_semantic(HookInNonComponent, &m);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("useState"));
        assert!(diags[0].message.contains("not a component or hook"));
    }

    #[test]
    fn hook_in_component_not_flagged() {
        let m = model_of(
            "src/ok.tsx",
            "export function Counter() { const [n] = useState(0); return <div>{n}</div>; }",
        );
        assert!(run_semantic(HookInNonComponent, &m).is_empty());
    }

    #[test]
    fn hook_in_custom_hook_not_flagged() {
        let m = model_of(
            "src/ok.tsx",
            "export function useCount() { const [n] = useState(0); return n; }",
        );
        assert!(run_semantic(HookInNonComponent, &m).is_empty());
    }

    #[test]
    fn hook_at_module_top_level_flagged() {
        let m = model_of("src/top.tsx", "const [n] = useState(0);");
        let diags = run_semantic(HookInNonComponent, &m);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("module top level"));
    }

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
