//! `next/forced-dynamic` — calls that opt a route out of static
//! rendering.
//!
//! `cookies()`, `headers()`, `draftMode()` (from `next/headers`) and
//! `noStore()`/`unstable_noStore()` (from `next/cache`) make every route
//! that renders the file dynamic. That is often intentional — severity
//! is Info — but it answers "why is this page not static?" with the
//! exact call site, proven ([`Confidence::Certain`]) because the call
//! only counts when the name was imported from the Next.js module.
//!
//! Only runs when Next.js is detected.

use snowbros_core::{Confidence, Diagnostic, Evidence, Severity, SourceLocation};

use crate::context::AnalysisContext;
use crate::registry::Rule;

/// See module docs.
pub struct ForcedDynamic;

impl Rule for ForcedDynamic {
    fn id(&self) -> &'static str {
        "next/forced-dynamic"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        // Next-specific rule: stay silent on non-Next projects.
        if !ctx.framework_owned_packages.contains("next") {
            return Vec::new();
        }
        let mut diagnostics = Vec::new();
        for (path, facts) in &ctx.file_facts {
            for call in &facts.dynamic_api_calls {
                diagnostics.push(
                    Diagnostic::new(
                        self.id(),
                        "Dynamic rendering forced",
                        format!(
                            "`{}()` opts every route that renders `{path}` out of \
                             static rendering and full-route caching. If this page \
                             should be static, read the value in a smaller dynamic \
                             boundary (e.g. a Suspense-wrapped child) instead.",
                            call.name
                        ),
                        "performance",
                        Severity::Info,
                        Confidence::Certain,
                        SourceLocation::new(path.clone(), call.span),
                    )
                    .with_evidence(Evidence::note(format!(
                        "`{}` is imported from next/headers or next/cache and \
                         called here",
                        call.name
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
    use snowbros_framework::{DetectedFramework, Framework};
    use snowbros_graph::SemanticGraph;
    use snowbros_parser::{extract_facts, parse, Language};
    use std::collections::BTreeMap;

    fn next_detected() -> Vec<DetectedFramework> {
        vec![DetectedFramework {
            framework: Framework::NextJs,
            confidence: snowbros_core::Confidence::Certain,
            version: None,
            evidence: vec![],
        }]
    }

    #[test]
    fn reports_cookies_call_in_next_project() {
        let src = r#"
import { cookies } from "next/headers";
export default async function Page() { const c = cookies(); return c; }
"#;
        let facts = extract_facts(&parse(src, Language::Tsx).unwrap());
        let mut map = BTreeMap::new();
        map.insert(camino::Utf8PathBuf::from("app/page.tsx"), facts);

        let g = SemanticGraph::new();
        let frameworks = next_detected();
        let ctx = AnalysisContext::new(
            &g,
            map,
            ContextInputs {
                frameworks: &frameworks,
                ..ContextInputs::default()
            },
        );
        let diags = ForcedDynamic.run(&ctx);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].confidence, Confidence::Certain);
        assert_eq!(diags[0].severity, Severity::Info);
        assert!(diags[0].location.span.start.line > 1);
    }

    #[test]
    fn silent_without_next() {
        let src = r#"
import { cookies } from "next/headers";
export function f() { return cookies(); }
"#;
        let facts = extract_facts(&parse(src, Language::TypeScript).unwrap());
        let mut map = BTreeMap::new();
        map.insert(camino::Utf8PathBuf::from("src/x.ts"), facts);
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(&g, map, ContextInputs::default());
        assert!(ForcedDynamic.run(&ctx).is_empty());
    }
}
