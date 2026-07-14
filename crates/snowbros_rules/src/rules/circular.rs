//! `graph/no-circular-imports` — import cycles, proven by Tarjan SCC.
//!
//! One finding per cycle (the root cause), anchored at the
//! lexicographically first member, with every member listed as evidence
//! — never N duplicate warnings.

use snowbros_core::{Confidence, Diagnostic, Evidence, Severity};

use crate::context::AnalysisContext;
use crate::registry::Rule;
use crate::requirements::{AnalysisStage, LanguageSupport, RuleRequirements};
use crate::rules::file_location;

/// See module docs.
pub struct CircularImports;

impl Rule for CircularImports {
    fn id(&self) -> &'static str {
        "graph/no-circular-imports"
    }

    /// Language-agnostic: an import cycle is a property of the import graph,
    /// which every wired language populates. Runs on any language whose
    /// frontend supplies the semantic (import) stage. A cycle never spans
    /// languages — each language's imports resolve only to its own files — so
    /// every finding's anchor is a single-language file the scheduler admits.
    fn requirements(&self) -> RuleRequirements {
        RuleRequirements {
            languages: LanguageSupport::Any,
            minimum_stage: AnalysisStage::Semantic,
        }
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for group in ctx.graph.circular_groups() {
            let mut labels: Vec<String> = group
                .iter()
                .filter_map(|&id| ctx.graph.node(id).map(|n| n.label()))
                .collect();
            labels.sort();
            let Some(anchor) = labels.first().cloned() else {
                continue;
            };

            let mut diag = Diagnostic::new(
                self.id(),
                "Circular import chain",
                format!(
                    "{} files import each other in a cycle. Cycles make modules \
                     impossible to test in isolation, can break tree-shaking, and \
                     cause undefined imports at runtime under some module orders.",
                    labels.len()
                ),
                "architecture",
                Severity::High,
                Confidence::Certain,
                file_location(anchor),
            );
            for label in &labels {
                diag = diag.with_evidence(Evidence::note(format!("cycle member: {label}")));
            }
            diagnostics.push(diag);
        }
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_graph::{EdgeKind, Node, SemanticGraph};

    #[test]
    fn one_finding_per_cycle_with_members_as_evidence() {
        let mut g = SemanticGraph::new();
        let a = g.add_node(Node::file("src/a.ts"));
        let b = g.add_node(Node::file("src/b.ts"));
        let c = g.add_node(Node::file("src/c.ts"));
        g.add_edge(a, b, EdgeKind::Imports);
        g.add_edge(b, a, EdgeKind::Imports);
        g.add_edge(a, c, EdgeKind::Imports);

        let ctx = AnalysisContext::new(
            &g,
            Default::default(),
            crate::context::ContextInputs::default(),
        );
        let diags = CircularImports.run(&ctx);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].location.file, "src/a.ts");
        assert_eq!(diags[0].evidence.len(), 2);
        assert_eq!(diags[0].confidence, snowbros_core::Confidence::Certain);
    }

    #[test]
    fn is_language_agnostic() {
        // Import cycles are graph-level; the rule must apply to every language,
        // not just ECMAScript, so Python (and future languages) get it too.
        assert_eq!(
            CircularImports.requirements().languages,
            crate::requirements::LanguageSupport::Any
        );
    }
}
