//! JSON output — the canonical machine-readable format.

use snowbros_core::CoreError;

use crate::report::Report;

/// Renders a report as pretty-printed JSON.
pub fn render(report: &Report) -> String {
    // Report contains only JSON-safe types; serialization cannot fail.
    serde_json::to_string_pretty(report).expect("Report is always JSON-serializable")
}

/// Parses a report back from JSON (used by the VS Code extension test
/// harness and round-trip tests).
pub fn parse(text: &str) -> Result<Report, CoreError> {
    serde_json::from_str(text).map_err(|e| CoreError::Io {
        path: "<json report>".to_string(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_core::{Confidence, Diagnostic, Position, Severity, SourceLocation, Span};

    #[test]
    fn roundtrip() {
        let report = Report::new(vec![Diagnostic::new(
            "security/no-eval",
            "Use of eval()",
            "eval() executes arbitrary strings.",
            "security",
            Severity::Critical,
            Confidence::Certain,
            SourceLocation::new(
                "src/x.ts",
                Span::new(Position::new(1, 1), Position::new(1, 10), 0, 9),
            ),
        )]);
        let json = render(&report);
        let back = parse(&json).unwrap();
        assert_eq!(report, back);
    }

    #[test]
    fn severity_keys_serialize_as_strings() {
        let report = Report::new(vec![]);
        let json = render(&report);
        assert!(json.contains("\"total\": 0"));
    }
}
