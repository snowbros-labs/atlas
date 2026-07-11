//! Rule trait and the built-in registry.

use snowbros_core::Diagnostic;

use crate::context::AnalysisContext;
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
pub fn run_all(ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
    builtin_rules()
        .iter()
        .flat_map(|rule| rule.run(ctx))
        .collect()
}
