//! Rule trait and the built-in registry.

use snowbros_core::Diagnostic;

use crate::context::AnalysisContext;
use crate::requirements::RuleRequirements;
use crate::rules;

/// A single analysis rule.
///
/// Contract:
/// - deterministic: same context → same diagnostics
/// - order-independent: must not depend on other rules having run
/// - evidence-backed: every diagnostic carries at least one evidence entry
pub trait Rule {
    /// Stable rule id, e.g. `graph/no-circular-imports`.
    fn id(&self) -> &'static str;
    /// Runs the rule against the context.
    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic>;
    /// The language family and minimum analysis stage this rule requires.
    ///
    /// Defaults to the ECMAScript family at the semantic stage — every rule
    /// Atlas shipped before multi-language. A rule targeting another language
    /// (or a genuinely language-agnostic one) overrides this; the scheduler
    /// then admits its findings only on files the requirements allow.
    fn requirements(&self) -> RuleRequirements {
        RuleRequirements::ecmascript()
    }
}

/// All built-in rules, in stable id order.
pub fn builtin_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(rules::boundary::PrivateEnvInClient),
        Box::new(rules::boundary::ServerOnlyInClient),
        Box::new(rules::circular::CircularImports),
        Box::new(rules::dead_files::DeadFiles),
        Box::new(rules::forced_dynamic::ForcedDynamic),
        Box::new(rules::hardcoded_secret::HardcodedSecrets),
        Box::new(rules::nextjs::ClientMetadataIgnored),
        Box::new(rules::nextjs::MixedRouter),
        Box::new(rules::no_eval::NoEval),
        Box::new(rules::react::AsyncClientComponent),
        Box::new(rules::react::ComponentNaming),
        Box::new(rules::react::HookInNonComponent),
        Box::new(rules::react::HookReturnsJsx),
        Box::new(rules::typescript::CircularTypeReference),
        Box::new(rules::typescript::DuplicateDeclaration),
        Box::new(rules::typescript::UnreachableSymbol),
        Box::new(rules::typescript::UnusedExport),
        Box::new(rules::unresolved::BrokenPathAlias),
        Box::new(rules::unresolved::UnresolvedImports),
        Box::new(rules::unused_deps::UnusedDependencies),
        Box::new(rules::unused_env::UnusedEnvVars),
        Box::new(rules::unused_export::UnusedExports),
    ]
}

/// Runs every built-in rule and collects diagnostics.
///
/// Each rule's findings are filtered through its [`RuleRequirements`]: a
/// finding survives only if the file it targets is a language the rule applies
/// to *and* that language's frontend is mature enough for the rule's stage
/// (language-neutral files like manifests always pass). This is the single
/// place the language policy is enforced — no rule body inspects the language.
pub fn run_all(ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
    builtin_rules()
        .iter()
        .flat_map(|rule| {
            let requirements = rule.requirements();
            rule.run(ctx)
                .into_iter()
                .filter(move |d| ctx.admits(&requirements, &d.location.file))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use camino::Utf8PathBuf;
    use snowbros_core::{Confidence, Evidence, Severity};
    use snowbros_graph::SemanticGraph;
    use snowbros_parser::Language;

    use crate::context::ContextInputs;
    use crate::requirements::{AnalysisStage, LanguageSupport};
    use crate::rules::file_location;

    /// A rule that emits one finding per file it is told about, with a
    /// configurable requirements contract — lets us test the scheduler filter
    /// in isolation from any real rule.
    struct FakeRule {
        files: Vec<&'static str>,
        requirements: RuleRequirements,
    }

    impl Rule for FakeRule {
        fn id(&self) -> &'static str {
            "test/fake"
        }
        fn run(&self, _ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
            self.files
                .iter()
                .map(|f| {
                    Diagnostic::new(
                        self.id(),
                        "x",
                        "x",
                        "test",
                        Severity::Low,
                        Confidence::Possible,
                        file_location(Utf8PathBuf::from(*f)),
                    )
                    .with_evidence(Evidence::note("n"))
                })
                .collect()
        }
        fn requirements(&self) -> RuleRequirements {
            self.requirements
        }
    }

    fn run_filtered(rule: &FakeRule, langs: &[(&str, Language)]) -> Vec<String> {
        let graph = SemanticGraph::new();
        let file_languages: BTreeMap<Utf8PathBuf, Language> = langs
            .iter()
            .map(|(p, l)| (Utf8PathBuf::from(*p), *l))
            .collect();
        let ctx = AnalysisContext::new(&graph, Default::default(), ContextInputs::default())
            .with_file_languages(file_languages);
        let requirements = rule.requirements();
        rule.run(&ctx)
            .into_iter()
            .filter(|d| ctx.admits(&requirements, &d.location.file))
            .map(|d| d.location.file.to_string())
            .collect()
    }

    #[test]
    fn ecmascript_rule_keeps_ts_drops_python() {
        let rule = FakeRule {
            files: vec!["a.ts", "b.py"],
            requirements: RuleRequirements::ecmascript(),
        };
        let kept = run_filtered(
            &rule,
            &[("a.ts", Language::TypeScript), ("b.py", Language::Python)],
        );
        assert_eq!(kept, vec!["a.ts".to_string()]);
    }

    #[test]
    fn language_neutral_files_always_pass() {
        // package.json has no source language; an ECMAScript rule's finding on
        // it (e.g. unused-dependency) must survive.
        let rule = FakeRule {
            files: vec!["package.json"],
            requirements: RuleRequirements::ecmascript(),
        };
        let kept = run_filtered(&rule, &[("a.ts", Language::TypeScript)]);
        assert_eq!(kept, vec!["package.json".to_string()]);
    }

    #[test]
    fn empty_language_map_admits_everything() {
        // Legacy path: no file languages known → all findings pass, preserving
        // pre-multi-language behavior.
        let rule = FakeRule {
            files: vec!["a.ts", "b.py"],
            requirements: RuleRequirements::ecmascript(),
        };
        let kept = run_filtered(&rule, &[]);
        assert_eq!(kept, vec!["a.ts".to_string(), "b.py".to_string()]);
    }

    #[test]
    fn callgraph_rule_skipped_on_preview_python_kept_on_ts() {
        // An Any-language rule needing the call graph runs on TS (Enterprise)
        // but not on Python (Preview tops out at Semantic).
        let rule = FakeRule {
            files: vec!["a.ts", "b.py"],
            requirements: RuleRequirements {
                languages: LanguageSupport::Any,
                minimum_stage: AnalysisStage::CallGraph,
            },
        };
        let kept = run_filtered(
            &rule,
            &[("a.ts", Language::TypeScript), ("b.py", Language::Python)],
        );
        assert_eq!(kept, vec!["a.ts".to_string()]);
    }
}
