//! `imports/unresolved-import` — relative imports pointing at nothing.
//!
//! Scope is deliberately narrow: only *relative* specifiers (`./x`,
//! `../y`) are reported. A relative import that matches no file after
//! extension and index probing is almost always a broken path or a
//! deleted file. Alias and bare specifiers are excluded — an
//! unconfigured alias usually means tsconfig knowledge we lack (package
//! extends, monorepo roots), and guessing there would produce noise.
//!
//! Confidence is [`Confidence::Likely`], not certain: the target could
//! be generated at build time.

use snowbros_core::{Confidence, Diagnostic, Evidence, Severity, SourceLocation};

use crate::context::AnalysisContext;
use crate::registry::Rule;

/// See module docs.
pub struct UnresolvedImports;

impl Rule for UnresolvedImports {
    fn id(&self) -> &'static str {
        "imports/unresolved-import"
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        ctx.unresolved_imports
            .iter()
            .filter(|u| u.specifier.starts_with("./") || u.specifier.starts_with("../"))
            .map(|u| {
                Diagnostic::new(
                    self.id(),
                    "Unresolved relative import",
                    format!(
                        "`{}` does not match any file in the project, even after \
                         trying every known extension and index file. The import \
                         will fail unless the target is generated at build time.",
                        u.specifier
                    ),
                    "imports",
                    Severity::Medium,
                    Confidence::Likely,
                    SourceLocation::new(u.file.clone(), u.span),
                )
                .with_evidence(Evidence::note(format!(
                    "probed `{}` with extensions ts/tsx/d.ts/js/jsx/mjs/cjs/json \
                     and index files — no match",
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
            },
            UnresolvedImport {
                file: "src/app.ts".into(),
                specifier: "@/unknown-alias".into(),
                span: span(),
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
