//! The [`Report`] — the single structured result of an analysis run.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use snowbros_core::{Diagnostic, Severity};

/// Complete result of one analysis run.
///
/// Deterministic by construction: diagnostics are stored sorted by
/// (file, span start, rule id), and the summary uses ordered maps.
/// No timestamps — the same codebase must produce the identical report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// Engine version that produced the report (`CARGO_PKG_VERSION`).
    pub engine_version: String,
    /// All findings, sorted by (file, span start, rule id).
    pub diagnostics: Vec<Diagnostic>,
    /// Aggregated counts.
    pub summary: Summary,
}

/// Aggregated counts over a report's diagnostics.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    /// Total number of findings.
    pub total: usize,
    /// Findings per severity (only severities that occur).
    pub by_severity: BTreeMap<Severity, usize>,
    /// Findings per category (only categories that occur).
    pub by_category: BTreeMap<String, usize>,
}

impl Report {
    /// Builds a report from diagnostics: sorts them deterministically and
    /// computes the summary.
    pub fn new(mut diagnostics: Vec<Diagnostic>) -> Self {
        diagnostics.sort_by(|a, b| {
            (&a.location.file, a.location.span.start_byte, &a.rule_id).cmp(&(
                &b.location.file,
                b.location.span.start_byte,
                &b.rule_id,
            ))
        });

        let mut summary = Summary {
            total: diagnostics.len(),
            ..Summary::default()
        };
        for d in &diagnostics {
            *summary.by_severity.entry(d.severity).or_default() += 1;
            *summary.by_category.entry(d.category.clone()).or_default() += 1;
        }

        Self {
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            diagnostics,
            summary,
        }
    }

    /// Whether the report contains any finding at or above `severity`.
    pub fn has_findings_at_least(&self, severity: Severity) -> bool {
        self.diagnostics.iter().any(|d| d.severity >= severity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_core::{Confidence, Position, SourceLocation, Span};

    fn diag(file: &str, start_byte: u32, rule: &str, severity: Severity) -> Diagnostic {
        Diagnostic::new(
            rule,
            "title",
            "message",
            rule.split('/').next().unwrap_or("misc"),
            severity,
            Confidence::Likely,
            SourceLocation::new(
                file,
                Span::new(
                    Position::new(1, 1),
                    Position::new(1, 2),
                    start_byte,
                    start_byte + 1,
                ),
            ),
        )
    }

    #[test]
    fn sorts_diagnostics_deterministically() {
        let report = Report::new(vec![
            diag("b.ts", 5, "security/x", Severity::High),
            diag("a.ts", 9, "perf/y", Severity::Low),
            diag("a.ts", 2, "security/x", Severity::High),
        ]);
        let order: Vec<(&str, u32)> = report
            .diagnostics
            .iter()
            .map(|d| (d.location.file.as_str(), d.location.span.start_byte))
            .collect();
        assert_eq!(order, vec![("a.ts", 2), ("a.ts", 9), ("b.ts", 5)]);
    }

    #[test]
    fn summary_counts() {
        let report = Report::new(vec![
            diag("a.ts", 0, "security/x", Severity::High),
            diag("a.ts", 5, "security/y", Severity::High),
            diag("b.ts", 0, "perf/z", Severity::Low),
        ]);
        assert_eq!(report.summary.total, 3);
        assert_eq!(report.summary.by_severity[&Severity::High], 2);
        assert_eq!(report.summary.by_category["security"], 2);
        assert_eq!(report.summary.by_category["perf"], 1);
    }

    #[test]
    fn threshold_check() {
        let report = Report::new(vec![diag("a.ts", 0, "perf/z", Severity::Medium)]);
        assert!(report.has_findings_at_least(Severity::Medium));
        assert!(!report.has_findings_at_least(Severity::High));
    }
}
