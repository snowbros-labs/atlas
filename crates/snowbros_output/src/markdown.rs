//! Markdown report rendering — for PR comments and docs.

use std::fmt::Write as _;

use crate::report::Report;

/// Renders a report as a Markdown document.
///
/// Output is deterministic: sections and rows follow the report's sorted
/// diagnostic order.
pub fn render(report: &Report) -> String {
    let mut md = String::new();
    let _ = writeln!(md, "# SNOWBROS Inspector Report");
    let _ = writeln!(md);
    let _ = writeln!(
        md,
        "**{} finding{}** · engine v{}",
        report.summary.total,
        if report.summary.total == 1 { "" } else { "s" },
        report.engine_version
    );
    let _ = writeln!(md);

    let _ = writeln!(md, "## Health");
    let _ = writeln!(md);
    let _ = writeln!(md, "**Overall: {}/100**", report.scorecard.overall);
    let _ = writeln!(md);
    let _ = writeln!(md, "| Category | Score |");
    let _ = writeln!(md, "|---|---|");
    for (name, cat) in &report.scorecard.categories {
        let _ = writeln!(md, "| {name} | {} |", cat.score);
    }
    let _ = writeln!(md);

    if report.summary.total == 0 {
        let _ = writeln!(md, "No issues found.");
        return md;
    }

    let _ = writeln!(md, "## Summary");
    let _ = writeln!(md);
    let _ = writeln!(md, "| Severity | Count |");
    let _ = writeln!(md, "|---|---|");
    // Highest severity first.
    for (severity, count) in report.summary.by_severity.iter().rev() {
        let _ = writeln!(md, "| {severity} | {count} |");
    }
    let _ = writeln!(md);

    let _ = writeln!(md, "## Findings");
    let _ = writeln!(md);
    for d in &report.diagnostics {
        let _ = writeln!(md, "### [{}] {} — `{}`", d.severity, d.title, d.rule_id);
        let _ = writeln!(md);
        let _ = writeln!(
            md,
            "`{}:{}:{}` · confidence: {}",
            d.location.file, d.location.span.start.line, d.location.span.start.column, d.confidence
        );
        let _ = writeln!(md);
        let _ = writeln!(md, "{}", d.message);
        if !d.evidence.is_empty() {
            let _ = writeln!(md);
            let _ = writeln!(md, "**Evidence:**");
            for e in &d.evidence {
                match &e.location {
                    Some(loc) => {
                        let _ = writeln!(
                            md,
                            "- {} (`{}:{}`)",
                            e.description, loc.file, loc.span.start.line
                        );
                    }
                    None => {
                        let _ = writeln!(md, "- {}", e.description);
                    }
                }
            }
        }
        if let Some(fix) = &d.suggested_fix {
            let _ = writeln!(md);
            let _ = writeln!(md, "**Suggested fix:** {}", fix.description);
        }
        if let Some(url) = &d.docs_url {
            let _ = writeln!(md);
            let _ = writeln!(md, "[Documentation]({url})");
        }
        let _ = writeln!(md);
    }
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_core::{
        Confidence, Diagnostic, Evidence, Position, Severity, SourceLocation, Span, SuggestedFix,
    };

    fn sample_report() -> Report {
        Report::new(vec![Diagnostic::new(
            "security/no-eval",
            "Use of eval()",
            "eval() executes arbitrary strings as code.",
            "security",
            Severity::Critical,
            Confidence::Certain,
            SourceLocation::new(
                "src/x.ts",
                Span::new(Position::new(3, 5), Position::new(3, 15), 40, 50),
            ),
        )
        .with_evidence(Evidence::note("Direct eval call with request body"))
        .with_fix(SuggestedFix {
            description: "Parse with JSON.parse instead".into(),
            replacement: Some("JSON.parse(input)".into()),
        })])
    }

    #[test]
    fn renders_findings_and_summary() {
        let md = render(&sample_report());
        assert!(md.contains("# SNOWBROS Inspector Report"));
        assert!(md.contains("**1 finding**"));
        assert!(md.contains("| critical | 1 |"));
        assert!(md.contains("`src/x.ts:3:5`"));
        assert!(md.contains("**Suggested fix:** Parse with JSON.parse instead"));
    }

    #[test]
    fn empty_report_says_no_issues() {
        let md = render(&Report::new(vec![]));
        assert!(md.contains("No issues found."));
    }

    #[test]
    fn deterministic() {
        assert_eq!(render(&sample_report()), render(&sample_report()));
    }
}
