//! `imports/unresolved-import` — relative imports pointing at nothing.
//!
//! Scope is deliberately narrow: only *relative* specifiers (dot-prefixed
//! — `./x`, `../y` in ECMAScript, `.mod`, `..pkg` in Python) are reported.
//! A relative import that matches no file after the language's own probing
//! is almost always a broken path or a deleted file. Alias and bare
//! specifiers are excluded — an unconfigured alias usually means config
//! knowledge we lack (tsconfig extends, monorepo roots), and guessing
//! there would produce noise. Absolute/package specifiers never reach this
//! rule: the resolver treats an unknown bare or dotted-absolute import as
//! External (stdlib / installed package), not unresolved.
//!
//! The rule stays language-agnostic: the per-language "how I probed"
//! narrative arrives as data in [`UnresolvedImport::probe_detail`], set by
//! the pipeline. No language-specific strings live here.
//!
//! Confidence is [`Confidence::Likely`], not certain: the target could
//! be generated at build time.

use snowbros_core::{Confidence, Diagnostic, Evidence, Severity, SourceLocation};

use crate::context::AnalysisContext;
use crate::registry::Rule;
use crate::requirements::{AnalysisStage, LanguageSupport, RuleRequirements};

/// See module docs.
pub struct UnresolvedImports;

impl Rule for UnresolvedImports {
    fn id(&self) -> &'static str {
        "imports/unresolved-import"
    }

    /// Language-agnostic: a broken relative import is a property every wired
    /// language's resolver reports the same way (dot-prefixed specifier, no
    /// target). The per-language probe narrative rides in `probe_detail`, so
    /// the rule runs on any language whose frontend supplies the semantic
    /// (import) stage.
    fn requirements(&self) -> RuleRequirements {
        RuleRequirements {
            languages: LanguageSupport::Any,
            minimum_stage: AnalysisStage::Semantic,
        }
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        ctx.unresolved_imports
            .iter()
            .filter(|u| u.specifier.starts_with('.'))
            .map(|u| {
                Diagnostic::new(
                    self.id(),
                    "Unresolved relative import",
                    format!(
                        "`{}` does not match any file in the project. The import \
                         will fail unless the target is generated at build time.",
                        u.specifier
                    ),
                    "imports",
                    Severity::Medium,
                    Confidence::Likely,
                    SourceLocation::new(u.file.clone(), u.span),
                )
                .with_evidence(Evidence::note(u.probe_detail.clone()))
            })
            .collect()
    }
}

/// `imports/broken-path-alias` — a specifier that matches a configured
/// tsconfig `paths` alias but resolves to no file.
///
/// Distinct from `imports/unresolved-import` (which covers only relative
/// specifiers): here the alias itself *is* configured — the expansion just
/// points nowhere, which means a typo in the specifier or a moved/deleted
/// target. The pipeline flags these via `UnresolvedImport::matched_alias`,
/// so the rule needs no tsconfig knowledge of its own.
pub struct BrokenPathAlias;

impl Rule for BrokenPathAlias {
    fn id(&self) -> &'static str {
        "imports/broken-path-alias"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        ctx.unresolved_imports
            .iter()
            .filter(|u| u.matched_alias)
            .map(|u| {
                Diagnostic::new(
                    self.id(),
                    "Broken path alias",
                    format!(
                        "`{}` matches a tsconfig `paths` alias but resolves to no \
                         file. The alias target is likely a typo or points at a \
                         moved or deleted module.",
                        u.specifier
                    ),
                    "imports",
                    Severity::Medium,
                    Confidence::Likely,
                    SourceLocation::new(u.file.clone(), u.span),
                )
                .with_evidence(Evidence::note(format!(
                    "alias `{}` expanded via tsconfig `paths` but no target file exists",
                    u.specifier
                )))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::UnresolvedImport;
    use snowbros_core::{Position, Span};
    use snowbros_graph::SemanticGraph;

    fn span() -> Span {
        Span::new(Position::new(3, 20), Position::new(3, 30), 50, 60)
    }

    #[test]
    fn relative_unresolved_reported_alias_skipped() {
        let g = SemanticGraph::new();
        let unresolved = vec![
            UnresolvedImport {
                file: "src/app.ts".into(),
                specifier: "./missing".into(),
                span: span(),
                matched_alias: false,
                probe_detail: "probed `./missing` — no match".into(),
            },
            UnresolvedImport {
                file: "src/app.ts".into(),
                specifier: "@/unknown-alias".into(),
                span: span(),
                matched_alias: false,
                probe_detail: String::new(),
            },
        ];
        let ctx = AnalysisContext::new(
            &g,
            Default::default(),
            crate::context::ContextInputs {
                unresolved_imports: &unresolved,
                ..Default::default()
            },
        );
        let diags = UnresolvedImports.run(&ctx);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`./missing`"));
        assert_eq!(diags[0].location.span.start.line, 3);
        assert_eq!(diags[0].confidence, Confidence::Likely);
        // Evidence is the pipeline-supplied, language-specific narrative.
        assert!(diags[0].evidence[0]
            .description
            .contains("probed `./missing`"));
    }

    #[test]
    fn python_dot_relative_reported_via_probe_detail() {
        // A single-dot Python relative import (`.missing_module`) must be
        // caught by the dot-prefix filter, and its evidence must be the
        // Python-flavored probe narrative — no language check in the rule.
        let g = SemanticGraph::new();
        let unresolved = vec![UnresolvedImport {
            file: "pkg/main.py".into(),
            specifier: ".missing_module".into(),
            span: span(),
            matched_alias: false,
            probe_detail: "probed `.missing_module` for a sibling `.py` module or \
                           package `__init__.py` — no match"
                .into(),
        }];
        let ctx = AnalysisContext::new(
            &g,
            Default::default(),
            crate::context::ContextInputs {
                unresolved_imports: &unresolved,
                ..Default::default()
            },
        );
        let diags = UnresolvedImports.run(&ctx);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`.missing_module`"));
        assert!(diags[0].evidence[0].description.contains("`__init__.py`"));
    }

    #[test]
    fn broken_alias_reported_only_for_matched_alias() {
        let g = SemanticGraph::new();
        let unresolved = vec![
            UnresolvedImport {
                file: "src/app.ts".into(),
                specifier: "@/moved/thing".into(),
                span: span(),
                matched_alias: true,
                probe_detail: String::new(),
            },
            UnresolvedImport {
                file: "src/app.ts".into(),
                specifier: "./missing".into(),
                span: span(),
                matched_alias: false,
                probe_detail: "probed `./missing` — no match".into(),
            },
        ];
        let ctx = AnalysisContext::new(
            &g,
            Default::default(),
            crate::context::ContextInputs {
                unresolved_imports: &unresolved,
                ..Default::default()
            },
        );
        // Broken-alias fires only on the alias; the relative one is the
        // unresolved-import rule's job.
        let alias = BrokenPathAlias.run(&ctx);
        assert_eq!(alias.len(), 1);
        assert!(alias[0].message.contains("`@/moved/thing`"));
        // And the generic rule fires only on the relative one.
        let rel = UnresolvedImports.run(&ctx);
        assert_eq!(rel.len(), 1);
        assert!(rel[0].message.contains("`./missing`"));
    }

    #[test]
    fn empty_when_all_resolved() {
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(
            &g,
            Default::default(),
            crate::context::ContextInputs::default(),
        );
        assert!(UnresolvedImports.run(&ctx).is_empty());
    }
}
