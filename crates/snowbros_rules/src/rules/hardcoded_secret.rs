//! `security/hardcoded-secret` — credentials committed in source.
//!
//! Two signals with different strengths:
//! - known credential prefix (`sk_live_`, `ghp_`, `AKIA`, …) →
//!   High / Likely
//! - credential-named binding assigned a long literal → Medium /
//!   Possible
//!
//! The report shows only a redacted preview (first 4 characters) —
//! the finding itself must never republish the secret. Test files are
//! excluded (fixture credentials are routine there).

use snowbros_core::{Confidence, Diagnostic, Evidence, Severity, SourceLocation};
use snowbros_parser::SecretSignal;

use crate::context::AnalysisContext;
use crate::registry::Rule;

/// See module docs.
pub struct HardcodedSecrets;

/// Test/fixture files where literal credentials are routine.
fn is_test_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.contains(".test.")
        || name.contains(".spec.")
        || path.contains("__tests__/")
        || path.contains("__mocks__/")
        || path.contains("fixtures/")
}

impl Rule for HardcodedSecrets {
    fn id(&self) -> &'static str {
        "security/hardcoded-secret"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for (path, facts) in &ctx.file_facts {
            if is_test_path(path.as_str()) {
                continue;
            }
            for candidate in &facts.secret_candidates {
                let (severity, confidence, why) = match candidate.signal {
                    SecretSignal::KnownPrefix => (
                        Severity::High,
                        Confidence::Likely,
                        "the literal starts with a well-known credential prefix",
                    ),
                    SecretSignal::SuspiciousName => (
                        Severity::Medium,
                        Confidence::Possible,
                        "a credential-named binding is assigned a long literal",
                    ),
                };
                let binding = candidate.binding.as_deref().unwrap_or("<expression>");
                diagnostics.push(
                    Diagnostic::new(
                        self.id(),
                        "Possible hardcoded secret",
                        format!(
                            "`{binding}` is assigned a literal that looks like a \
                             credential (`{}…`, {} chars). Secrets in source end \
                             up in git history and every bundle. Move it to an \
                             environment variable and rotate the exposed value.",
                            candidate.preview, candidate.length
                        ),
                        "security",
                        severity,
                        confidence,
                        SourceLocation::new(path.clone(), candidate.span),
                    )
                    .with_evidence(Evidence::note(format!(
                        "{why}; value redacted to `{}…`",
                        candidate.preview
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
    use snowbros_graph::SemanticGraph;
    use snowbros_parser::{extract_facts, parse, Language};
    use std::collections::BTreeMap;

    fn ctx_map(path: &str, src: &str) -> BTreeMap<camino::Utf8PathBuf, snowbros_parser::FileFacts> {
        let mut map = BTreeMap::new();
        map.insert(
            camino::Utf8PathBuf::from(path),
            extract_facts(&parse(src, Language::TypeScript).unwrap()),
        );
        map
    }

    #[test]
    fn prefix_secret_high_name_secret_medium_and_redacted() {
        let map = ctx_map(
            "src/config.ts",
            r#"
const stripe = "sk_live_abc123def456ghi789";
const apiToken = "zz91jf02mfkw88ax";
"#,
        );
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(&g, map, ContextInputs::default());
        let diags = HardcodedSecrets.run(&ctx);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].severity, Severity::High);
        assert_eq!(diags[1].severity, Severity::Medium);
        // Never leak the value.
        for d in &diags {
            assert!(!d.message.contains("abc123def456"));
            assert!(!d.message.contains("zz91jf02mfkw88ax"));
        }
    }

    #[test]
    fn test_files_excluded() {
        let map = ctx_map(
            "src/auth.test.ts",
            r#"const apiToken = "zz91jf02mfkw88ax";"#,
        );
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(&g, map, ContextInputs::default());
        assert!(HardcodedSecrets.run(&ctx).is_empty());
    }
}
