//! `security/no-eval` — arbitrary code execution sinks.
//!
//! `eval()` (direct, `window.`, `globalThis.`) and `new Function()`
//! execute strings as code: a primary vector for XSS and RCE, a blocker
//! for CSP, and unanalyzable by any static tool. The call site is
//! proven ([`Confidence::Certain`]); whether attacker-controlled data
//! reaches it is a separate (taint) question, reflected in severity
//! High rather than Critical.

use snowbros_core::{Confidence, Diagnostic, Evidence, Severity, SourceLocation};

use crate::context::AnalysisContext;
use crate::registry::Rule;

/// See module docs.
pub struct NoEval;

impl Rule for NoEval {
    fn id(&self) -> &'static str {
        "security/no-eval"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for (path, facts) in &ctx.file_facts {
            for call in &facts.eval_calls {
                diagnostics.push(
                    Diagnostic::new(
                        self.id(),
                        "Dynamic code execution",
                        format!(
                            "`{}` executes a string as code. It enables code \
                             injection if any input reaches it, blocks Content \
                             Security Policy, and defeats static analysis. Use \
                             JSON.parse for data or explicit dispatch for \
                             dynamic behavior.",
                            call.name
                        ),
                        "security",
                        Severity::High,
                        Confidence::Certain,
                        SourceLocation::new(path.clone(), call.span),
                    )
                    .with_evidence(Evidence::note(format!(
                        "`{}` called at {}:{}",
                        call.name, path, call.span.start.line
                    )))
                    .with_docs_url("https://owasp.org/Top10/A03_2021-Injection/"),
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
    use snowbros_graph::SemanticGraph;
    use snowbros_parser::{extract_facts, parse, Language};
    use std::collections::BTreeMap;

    #[test]
    fn reports_each_eval_site() {
        let facts = extract_facts(
            &parse(
                "eval(input); const f = new Function(body);",
                Language::JavaScript,
            )
            .unwrap(),
        );
        let mut map = BTreeMap::new();
        map.insert(camino::Utf8PathBuf::from("src/x.js"), facts);
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(&g, map, ContextInputs::default());
        let diags = NoEval.run(&ctx);
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().all(|d| d.severity == Severity::High));
        assert!(diags.iter().all(|d| d.confidence == Confidence::Certain));
    }
}
