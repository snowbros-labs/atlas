//! `complexity/large-function` — a top-level function whose body is very long.
//!
//! Language-neutral by construction: it reads only the lowered Atlas IR
//! ([`FunctionData::body_span`]), which every frontend populates, so the same
//! rule flags an over-long function in TypeScript, JavaScript, or Python
//! without a single language-specific branch. This is the first rule to prove
//! the shared IR carries a real cross-language diagnostic.
//!
//! Scope and conservatism:
//! - Only *top-level* functions are measured. Methods are recorded as class
//!   members in the IR, not as standalone function symbols, so they are not
//!   double-counted or split apart here.
//! - Length is the function body's physical line span — a coarse but
//!   language-agnostic proxy. No cyclomatic or cognitive complexity is
//!   attempted; those need control-flow the IR does not yet carry.
//! - Severity [`Severity::Low`] / confidence [`Confidence::Possible`]: a long
//!   function is a smell, not a defect, so the finding invites review rather
//!   than asserting a bug. The rule ships at `nursery` maturity.

use snowbros_core::{Confidence, Diagnostic, Evidence, Severity, SourceLocation};
use snowbros_ir::SymbolKind;

use crate::context::AnalysisContext;
use crate::registry::Rule;
use crate::requirements::{AnalysisStage, LanguageSupport, RuleRequirements};

/// A function body longer than this many physical lines is reported. Chosen
/// as a widely-used readability ceiling; deliberately generous so the default
/// only fires on genuinely large functions.
const MAX_BODY_LINES: u32 = 50;

/// See module docs.
pub struct LargeFunction;

impl Rule for LargeFunction {
    fn id(&self) -> &'static str {
        "complexity/large-function"
    }

    /// Language-agnostic: function length is a property of the lowered IR that
    /// every wired language populates, so the rule runs on any language whose
    /// frontend supplies the semantic stage.
    fn requirements(&self) -> RuleRequirements {
        RuleRequirements {
            languages: LanguageSupport::Any,
            minimum_stage: AnalysisStage::Semantic,
        }
    }

    fn run(&self, ctx: &AnalysisContext<'_>) -> Vec<Diagnostic> {
        let Some(model) = ctx.semantic else {
            return Vec::new();
        };

        let mut diagnostics = Vec::new();
        for sym in model.symbols() {
            let SymbolKind::Function(data) = &sym.symbol.kind else {
                continue;
            };
            let Some(body) = &data.body_span else {
                continue;
            };
            // Physical line span of the body; base (0- vs 1-indexed) cancels in
            // the difference, so the count is correct either way.
            let lines = body.end.line.saturating_sub(body.start.line) + 1;
            if lines <= MAX_BODY_LINES {
                continue;
            }
            let name = &sym.symbol.name;
            diagnostics.push(
                Diagnostic::new(
                    self.id(),
                    "Large function",
                    format!(
                        "`{name}` is {lines} lines long (over {MAX_BODY_LINES}). \
                         Large functions are harder to read, test, and reuse; \
                         consider extracting cohesive pieces into smaller \
                         functions."
                    ),
                    "complexity",
                    Severity::Low,
                    Confidence::Possible,
                    SourceLocation::new(sym.module.to_path_buf(), sym.symbol.span),
                )
                .with_evidence(Evidence::note(format!(
                    "function body spans {lines} lines (threshold {MAX_BODY_LINES})"
                ))),
            );
        }
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ContextInputs;
    use snowbros_core::{Position, Span};
    use snowbros_graph::SemanticGraph;
    use snowbros_ir::{FunctionData, Module, Symbol, SymbolKind};
    use snowbros_semantic::SemanticModel;

    /// A function symbol whose body spans `[start_line, end_line]`.
    fn func(name: &str, start_line: u32, end_line: u32) -> Symbol {
        Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function(FunctionData {
                body_span: Some(Span::new(
                    Position::new(start_line, 0),
                    Position::new(end_line, 0),
                    0,
                    0,
                )),
                ..Default::default()
            }),
            span: Span::new(
                Position::new(start_line, 0),
                Position::new(start_line, 4),
                0,
                4,
            ),
            exported: false,
        }
    }

    fn model_of(path: &str, symbols: Vec<Symbol>) -> SemanticModel {
        let module = Module {
            path: path.into(),
            symbols,
            ..Default::default()
        };
        SemanticModel::from_modules([module])
    }

    fn diags_for(model: &SemanticModel) -> Vec<Diagnostic> {
        let g = SemanticGraph::new();
        let ctx = AnalysisContext::new(
            &g,
            Default::default(),
            ContextInputs {
                semantic: Some(model),
                ..Default::default()
            },
        );
        LargeFunction.run(&ctx)
    }

    #[test]
    fn flags_function_longer_than_threshold() {
        // Body lines 10..=65 → 56 lines, over the 50-line ceiling.
        let model = model_of("src/big.ts", vec![func("handler", 10, 65)]);
        let diags = diags_for(&model);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`handler`"));
        assert!(diags[0].message.contains("56 lines"));
        assert_eq!(diags[0].severity, Severity::Low);
        assert_eq!(diags[0].confidence, Confidence::Possible);
    }

    #[test]
    fn ignores_function_at_or_below_threshold() {
        // Exactly 50 lines (1..=50) must not fire — the ceiling is inclusive.
        let model = model_of("src/ok.py", vec![func("small", 1, 50)]);
        assert!(diags_for(&model).is_empty());
    }

    #[test]
    fn language_agnostic_fires_on_python_and_typescript_alike() {
        // Same rule, two languages, no language branch: a 60-line body in a
        // `.py` module and a `.ts` module both flag.
        let py = model_of("app/service.py", vec![func("process", 1, 60)]);
        let ts = model_of("src/service.ts", vec![func("process", 1, 60)]);
        assert_eq!(diags_for(&py).len(), 1);
        assert_eq!(diags_for(&ts).len(), 1);
    }

    #[test]
    fn non_function_symbols_are_ignored() {
        let sym = Symbol {
            name: "Config".to_string(),
            kind: SymbolKind::Const,
            span: Span::new(Position::new(1, 0), Position::new(1, 6), 0, 6),
            exported: true,
        };
        assert!(diags_for(&model_of("src/c.ts", vec![sym])).is_empty());
    }
}
