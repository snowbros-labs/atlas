//! Symbol-level TypeScript rules, resolved over the semantic layer.
//!
//! These are the M0 proof rules for the Atlas IR + [`SemanticModel`]
//! wiring: unlike the textual `exports/unused-export`, they read declared
//! [`ir::Symbol`]s and carry the stable [`SymbolId`] in their evidence,
//! demonstrating that rules can operate on Atlas concepts rather than
//! parser facts.
//!
//! [`SemanticModel`]: snowbros_semantic::SemanticModel
//! [`ir::Symbol`]: snowbros_ir::Symbol
//! [`SymbolId`]: snowbros_ir::SymbolId

use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8Path;
use snowbros_core::{Confidence, Diagnostic, Evidence, Severity, SourceLocation};

use crate::context::AnalysisContext;
use crate::registry::Rule;
use crate::rules::dead_files::is_excluded;

/// `typescript/unused-export` — an exported symbol that no importer binds,
/// resolved against the semantic symbol model.
///
/// FP guards mirror `exports/unused-export`: only imported files are
/// considered (unimported files are `graph/dead-file` territory), a `*`
/// importer marks everything used, and entry/config/declaration files are
/// excluded. Confidence is [`Confidence::Possible`] — the export may be a
/// public API.
pub struct UnusedExport;

/// Per-target usage: names bound somewhere, and whether any importer takes
/// everything (`*`).
#[derive(Default)]
struct Usage {
    star: bool,
    names: BTreeSet<String>,
}

impl Rule for UnusedExport {
    fn id(&self) -> &'static str {
        "typescript/unused-export"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let Some(model) = ctx.semantic else {
            return Vec::new();
        };

        let mut usage: BTreeMap<&Utf8Path, Usage> = BTreeMap::new();
        for binding in ctx.import_bindings {
            let entry = usage.entry(binding.to.as_path()).or_default();
            if binding.names.is_empty() {
                entry.star = true; // side-effect / whole-module import
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
        for sym in model.exported_symbols() {
            let path = sym.module;
            if is_excluded(path.as_str()) {
                continue;
            }
            // Unimported files are dead-file findings, not per-export noise.
            let Some(used) = usage.get(path) else {
                continue;
            };
            if used.star || used.names.contains(&sym.symbol.name) {
                continue;
            }
            diagnostics.push(
                Diagnostic::new(
                    self.id(),
                    "Unused export",
                    format!(
                        "`{}` is exported from `{path}` but no importer binds it. \
                         Unused exports block tree-shaking and hide dead code.",
                        sym.symbol.name
                    ),
                    "typescript",
                    Severity::Low,
                    Confidence::Possible,
                    SourceLocation::new(path.to_owned(), sym.symbol.span),
                )
                .with_evidence(Evidence::note(format!(
                    "symbol `{}` is imported by {} file(s), none bind it",
                    sym.id(),
                    ctx.import_bindings.iter().filter(|b| b.to == path).count()
                ))),
            );
        }
        diagnostics
    }
}

/// `typescript/duplicate-declaration` — a name declared more than once at
/// module top level.
///
/// Overload signatures and interface/namespace merging are *not* flagged:
/// only bodied declarations are lowered to symbols, so redeclarations here
/// are genuine clashes (`const x` twice, two `function x` bodies).
pub struct DuplicateDeclaration;

impl Rule for DuplicateDeclaration {
    fn id(&self) -> &'static str {
        "typescript/duplicate-declaration"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let Some(model) = ctx.semantic else {
            return Vec::new();
        };

        let mut diagnostics = Vec::new();
        for dup in model.duplicate_declarations() {
            // Anchor the finding at the redeclaration (last span).
            let last = *dup.spans.last().expect("duplicate has >= 2 spans");
            let lines: Vec<String> = dup
                .spans
                .iter()
                .map(|s| format!("line {}", s.start.line))
                .collect();
            diagnostics.push(
                Diagnostic::new(
                    self.id(),
                    "Duplicate declaration",
                    format!(
                        "`{}` is declared {} times at the top level of `{}`. \
                         Redeclaration shadows earlier definitions and is a \
                         TypeScript error for `const`/`let`.",
                        dup.name,
                        dup.spans.len(),
                        dup.module
                    ),
                    "typescript",
                    Severity::Medium,
                    Confidence::Likely,
                    SourceLocation::new(dup.module.clone(), last),
                )
                .with_evidence(Evidence::note(format!(
                    "`{}` declared at {}",
                    dup.name,
                    lines.join(", ")
                ))),
            );
        }
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ContextInputs, ImportBinding};
    use snowbros_graph::SemanticGraph;
    use snowbros_parser::{lower, parse, Language};
    use snowbros_semantic::SemanticModel;

    fn model(entries: &[(&str, &str)]) -> SemanticModel {
        SemanticModel::from_modules(
            entries
                .iter()
                .map(|(path, src)| lower(&parse(*src, Language::TypeScript).unwrap(), *path)),
        )
    }

    fn ctx<'a>(
        g: &'a SemanticGraph,
        model: &'a SemanticModel,
        bindings: &'a [ImportBinding],
    ) -> AnalysisContext<'a> {
        AnalysisContext::new(
            g,
            std::collections::BTreeMap::new(),
            ContextInputs {
                import_bindings: bindings,
                semantic: Some(model),
                ..ContextInputs::default()
            },
        )
    }

    #[test]
    fn unused_export_reported_via_semantic_model() {
        let m = model(&[(
            "src/util.ts",
            "export const used = 1; export const orphan = 2;",
        )]);
        let bindings = vec![ImportBinding {
            from: "src/app.ts".into(),
            to: "src/util.ts".into(),
            names: vec!["used".into()],
        }];
        let g = SemanticGraph::new();
        let diags = UnusedExport.run(&ctx(&g, &m, &bindings));
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`orphan`"));
        assert!(diags[0].evidence[0].description.contains("#const#orphan"));
    }

    #[test]
    fn star_import_marks_all_used() {
        let m = model(&[("src/util.ts", "export const a = 1; export const b = 2;")]);
        let bindings = vec![ImportBinding {
            from: "src/app.ts".into(),
            to: "src/util.ts".into(),
            names: vec!["*".into()],
        }];
        let g = SemanticGraph::new();
        assert!(UnusedExport.run(&ctx(&g, &m, &bindings)).is_empty());
    }

    #[test]
    fn unimported_file_left_to_dead_file_rule() {
        let m = model(&[("src/lonely.ts", "export const x = 1;")]);
        let g = SemanticGraph::new();
        assert!(UnusedExport.run(&ctx(&g, &m, &[])).is_empty());
    }

    #[test]
    fn duplicate_declaration_reported() {
        let m = model(&[("src/a.ts", "const dup = 1;\nconst dup = 2;\n")]);
        let g = SemanticGraph::new();
        let diags = DuplicateDeclaration.run(&ctx(&g, &m, &[]));
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`dup`"));
        assert!(diags[0].evidence[0].description.contains("line 1"));
        assert!(diags[0].evidence[0].description.contains("line 2"));
    }

    #[test]
    fn no_duplicate_for_unique_names() {
        let m = model(&[("src/a.ts", "const a = 1;\nconst b = 2;\n")]);
        let g = SemanticGraph::new();
        assert!(DuplicateDeclaration.run(&ctx(&g, &m, &[])).is_empty());
    }

    #[test]
    fn no_semantic_model_is_no_findings() {
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(
            &g,
            std::collections::BTreeMap::new(),
            ContextInputs::default(),
        );
        assert!(UnusedExport.run(&ctx).is_empty());
        assert!(DuplicateDeclaration.run(&ctx).is_empty());
    }
}
