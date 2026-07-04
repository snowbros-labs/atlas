//! Self-contained HTML report — one file, inline CSS, no external
//! requests, no JavaScript. Safe to attach to CI artifacts or email.

use std::fmt::Write as _;

use snowbros_core::Severity;

use crate::report::Report;

/// Escapes text for safe HTML interpolation.
fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn severity_class(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "critical",
        Severity::High => "high",
        Severity::Medium => "medium",
        Severity::Low => "low",
        Severity::Info => "info",
    }
}

fn score_class(score: u8) -> &'static str {
    match score {
        90..=100 => "good",
        70..=89 => "ok",
        _ => "bad",
    }
}

const CSS: &str = r#"
:root { color-scheme: light dark; }
body { font-family: system-ui, sans-serif; margin: 2rem auto; max-width: 60rem;
       padding: 0 1rem; line-height: 1.5; }
h1 { font-size: 1.4rem; } h2 { font-size: 1.1rem; margin-top: 2rem; }
.scores { display: flex; flex-wrap: wrap; gap: 0.8rem; }
.score { border: 1px solid #8884; border-radius: 8px; padding: 0.8rem 1.2rem;
         min-width: 8rem; text-align: center; }
.score .value { font-size: 1.8rem; font-weight: 700; display: block; }
.score.good .value { color: #1a9c4b; }
.score.ok .value { color: #c98a00; }
.score.bad .value { color: #d43a3a; }
.finding { border: 1px solid #8884; border-left-width: 5px; border-radius: 6px;
           padding: 0.7rem 1rem; margin: 0.8rem 0; }
.finding.critical, .finding.high { border-left-color: #d43a3a; }
.finding.medium { border-left-color: #c98a00; }
.finding.low, .finding.info { border-left-color: #8888; }
.finding .meta { font-size: 0.85rem; opacity: 0.75; }
.finding ul { margin: 0.4rem 0 0; padding-left: 1.2rem; font-size: 0.9rem; }
.badge { display: inline-block; padding: 0.05rem 0.5rem; border-radius: 99px;
         font-size: 0.75rem; font-weight: 600; text-transform: uppercase;
         border: 1px solid #8886; }
code { background: #8882; border-radius: 4px; padding: 0.1rem 0.3rem; }
footer { margin-top: 3rem; font-size: 0.8rem; opacity: 0.6; }
"#;

/// Renders a report as a complete standalone HTML document.
pub fn render(report: &Report) -> String {
    let mut h = String::new();
    let _ = write!(
        h,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>SNOWBROS Inspector Report</title><style>{CSS}</style></head><body>"
    );
    let _ = write!(
        h,
        "<h1>SNOWBROS Inspector Report</h1>\
         <p>{} finding{} · engine v{}</p>",
        report.summary.total,
        if report.summary.total == 1 { "" } else { "s" },
        esc(&report.engine_version)
    );

    // Scorecard.
    let _ = write!(h, "<h2>Health</h2><div class=\"scores\">");
    let _ = write!(
        h,
        "<div class=\"score {}\"><span class=\"value\">{}</span>overall</div>",
        score_class(report.scorecard.overall),
        report.scorecard.overall
    );
    for (name, cat) in &report.scorecard.categories {
        let _ = write!(
            h,
            "<div class=\"score {}\"><span class=\"value\">{}</span>{}</div>",
            score_class(cat.score),
            cat.score,
            esc(name)
        );
    }
    let _ = write!(h, "</div>");

    // Findings.
    let _ = write!(h, "<h2>Findings</h2>");
    if report.diagnostics.is_empty() {
        let _ = write!(h, "<p>No issues found.</p>");
    }
    for d in &report.diagnostics {
        let _ = write!(
            h,
            "<div class=\"finding {}\">\
             <span class=\"badge\">{}</span> <strong>{}</strong> \
             <code>{}</code>\
             <div class=\"meta\">{}:{}:{} · confidence: {}</div>\
             <p>{}</p>",
            severity_class(d.severity),
            d.severity,
            esc(&d.title),
            esc(&d.rule_id),
            esc(d.location.file.as_str()),
            d.location.span.start.line,
            d.location.span.start.column,
            d.confidence,
            esc(&d.message),
        );
        if !d.evidence.is_empty() {
            let _ = write!(h, "<ul>");
            for e in &d.evidence {
                let _ = write!(h, "<li>{}</li>", esc(&e.description));
            }
            let _ = write!(h, "</ul>");
        }
        if let Some(fix) = &d.suggested_fix {
            let _ = write!(h, "<p><strong>Fix:</strong> {}</p>", esc(&fix.description));
        }
        let _ = write!(h, "</div>");
    }

    let _ = write!(
        h,
        "<footer>Generated deterministically by SNOWBROS Inspector — same \
         codebase, same report.</footer></body></html>"
    );
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_core::{Confidence, Diagnostic, Position, SourceLocation, Span};

    fn sample() -> Report {
        Report::new(vec![Diagnostic::new(
            "graph/no-circular-imports",
            "Circular import <chain>",
            "2 files & friends import each other.",
            "architecture",
            Severity::High,
            Confidence::Certain,
            SourceLocation::new(
                "src/a.ts",
                Span::new(Position::new(1, 1), Position::new(1, 1), 0, 0),
            ),
        )])
    }

    #[test]
    fn complete_standalone_document() {
        let html = render(&sample());
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.ends_with("</html>"));
        assert!(html.contains("<style>"));
        // No external requests.
        assert!(!html.contains("http://"));
        assert!(!html.contains("src=\"http"));
    }

    #[test]
    fn escapes_html_in_user_content() {
        let html = render(&sample());
        assert!(html.contains("Circular import &lt;chain&gt;"));
        assert!(html.contains("&amp; friends"));
        assert!(!html.contains("<chain>"));
    }

    #[test]
    fn shows_scorecard() {
        let html = render(&sample());
        assert!(html.contains("overall"));
        assert!(html.contains("architecture"));
    }

    #[test]
    fn deterministic() {
        assert_eq!(render(&sample()), render(&sample()));
    }
}
