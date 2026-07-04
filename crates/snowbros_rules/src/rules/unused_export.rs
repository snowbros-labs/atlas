//! `exports/unused-export` — exported names no importer ever binds.
//!
//! FP guards, per the accuracy-first policy:
//! - only files that *are* imported are considered (files nobody imports
//!   are `graph/dead-file` territory — no double reporting)
//! - a namespace (`* as ns`) or `export * from` importer marks every
//!   export of the target as used
//! - entry-point/test/config/declaration files are excluded (same
//!   pattern list as `graph/dead-file`)
//! - confidence is [`Confidence::Possible`]: the export may be a public
//!   library API or consumed by tooling

use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8PathBuf;
use snowbros_core::{Confidence, Diagnostic, Evidence, Severity, SourceLocation};

use crate::context::AnalysisContext;
use crate::registry::Rule;
use crate::rules::dead_files::is_excluded;

/// See module docs.
pub struct UnusedExports;

/// Per-target usage: names bound somewhere, and whether any importer
/// takes everything (`*`).
#[derive(Default)]
struct Usage {
    star: bool,
    names: BTreeSet<String>,
}

impl Rule for UnusedExports {
    fn id(&self) -> &'static str {
        "exports/unused-export"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let mut usage: BTreeMap<&Utf8PathBuf, Usage> = BTreeMap::new();
        for binding in ctx.import_bindings {
            let entry = usage.entry(&binding.to).or_default();
            if binding.names.is_empty() {
                // Bare `import "./x"`, require(), dynamic import():
                // side-effect or whole-module use — treat as star.
                entry.star = true;
            }
            for name in &binding.names {
                if name == "*" {
                    entry.star = true;
                } else {
                    entry.names.insert(name.clone());
                }
            }
        }

        let mut diagnostics = Vec::new();
        for (path, facts) in &ctx.file_facts {
            if facts.exports.is_empty() || is_excluded(path.as_str()) {
                continue;
            }
            // Unimported files are dead-file findings, not per-export noise.
            let Some(used) = usage.get(path) else {
                continue;
            };
            if used.star {
                continue;
            }
            for export in &facts.exports {
                if used.names.contains(&export.name) {
                    continue;
                }
                diagnostics.push(
                    Diagnostic::new(
                        self.id(),
                        "Unused export",
                        format!(
                            "`{}` is exported from `{path}` but no importer binds \
                             it. Unused exports block tree-shaking and hide dead \
                             code.",
                            export.name
                        ),
                        "architecture",
                        Severity::Low,
                        Confidence::Possible,
                        SourceLocation::new(path.clone(), export.span),
                    )
                    .with_evidence(Evidence::note(format!(
                        "{} file(s) import `{path}`, none bind `{}`",
                        ctx.import_bindings.iter().filter(|b| b.to == *path).count(),
                        export.name
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
    use crate::context::{ContextInputs, ImportBinding};
    use snowbros_graph::SemanticGraph;
    use snowbros_parser::{extract_facts, parse, Language};

    fn facts_map(entries: &[(&str, &str)]) -> BTreeMap<Utf8PathBuf, snowbros_parser::FileFacts> {
        entries
            .iter()
            .map(|(path, src)| {
                (
                    Utf8PathBuf::from(*path),
                    extract_facts(&parse(*src, Language::TypeScript).unwrap()),
                )
            })
            .collect()
    }

    #[test]
    fn unbound_export_reported() {
        let map = facts_map(&[(
            "src/util.ts",
            "export const used = 1; export const orphan = 2;",
        )]);
        let bindings = vec![ImportBinding {
            from: "src/app.ts".into(),
            to: "src/util.ts".into(),
            names: vec!["used".into()],
        }];
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(
            &g,
            map,
            ContextInputs {
                import_bindings: &bindings,
                ..ContextInputs::default()
            },
        );
        let diags = UnusedExports.run(&ctx);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`orphan`"));
    }

    #[test]
    fn star_import_marks_all_used() {
        let map = facts_map(&[("src/util.ts", "export const a = 1; export const b = 2;")]);
        let bindings = vec![ImportBinding {
            from: "src/app.ts".into(),
            to: "src/util.ts".into(),
            names: vec!["*".into()],
        }];
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(
            &g,
            map,
            ContextInputs {
                import_bindings: &bindings,
                ..ContextInputs::default()
            },
        );
        assert!(UnusedExports.run(&ctx).is_empty());
    }

    #[test]
    fn unimported_file_left_to_dead_file_rule() {
        let map = facts_map(&[("src/lonely.ts", "export const x = 1;")]);
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(&g, map, ContextInputs::default());
        assert!(UnusedExports.run(&ctx).is_empty());
    }

    #[test]
    fn entrypoints_excluded() {
        let map = facts_map(&[("app/page.tsx", "export default function P() {}")]);
        let bindings = vec![ImportBinding {
            from: "src/other.ts".into(),
            to: "app/page.tsx".into(),
            names: vec!["nothing".into()],
        }];
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(
            &g,
            map,
            ContextInputs {
                import_bindings: &bindings,
                ..ContextInputs::default()
            },
        );
        assert!(UnusedExports.run(&ctx).is_empty());
    }
}
