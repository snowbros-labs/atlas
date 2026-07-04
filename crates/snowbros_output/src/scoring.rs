//! Project scoring — explainable category scores and overall health.
//!
//! Scores must be explainable: every point deducted is attributed to a
//! rule with a count, so a user can always answer "why is my
//! architecture score 62?".
//!
//! Formula (deterministic, integer-only at the end):
//! - each finding deducts `severity_weight × confidence_factor` points
//!   from its category, starting at 100, floored at 0
//! - severity weights: critical 25, high 15, medium 8, low 3, info 1
//! - confidence factors: certain 1.0, likely 0.7, possible 0.4,
//!   unknown 0.2 — an uncertain finding hurts the score less
//! - overall = mean of all monitored categories (categories without
//!   findings count as 100)

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use snowbros_core::{Confidence, Diagnostic, Severity};

/// Categories always included in the overall score, even when clean.
/// Grows as rule categories ship.
pub const MONITORED_CATEGORIES: &[&str] = &[
    "architecture",
    "dependencies",
    "environment",
    "imports",
    "performance",
];

/// Points deducted per finding of a severity.
fn severity_weight(severity: Severity) -> f64 {
    match severity {
        Severity::Critical => 25.0,
        Severity::High => 15.0,
        Severity::Medium => 8.0,
        Severity::Low => 3.0,
        Severity::Info => 1.0,
    }
}

/// Down-weighting for uncertain findings.
fn confidence_factor(confidence: Confidence) -> f64 {
    match confidence {
        Confidence::Certain => 1.0,
        Confidence::Likely => 0.7,
        Confidence::Possible => 0.4,
        Confidence::Unknown => 0.2,
    }
}

/// One rule's contribution to a category's deductions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Deduction {
    /// Rule that caused the deduction.
    pub rule_id: String,
    /// Number of findings from this rule in this category.
    pub count: usize,
    /// Total points deducted (rounded to one decimal).
    pub points: f64,
}

/// Score for one category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryScore {
    /// 0–100.
    pub score: u8,
    /// Why: per-rule deductions, sorted by points descending then rule id.
    pub deductions: Vec<Deduction>,
}

/// The project scorecard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scorecard {
    /// 0–100 overall health (mean of monitored categories).
    pub overall: u8,
    /// Per-category scores (monitored categories plus any category that
    /// produced findings).
    pub categories: BTreeMap<String, CategoryScore>,
}

impl Scorecard {
    /// Computes the scorecard from diagnostics.
    pub fn compute(diagnostics: &[Diagnostic]) -> Self {
        // Accumulate raw deductions per (category, rule).
        let mut raw: BTreeMap<&str, BTreeMap<&str, (usize, f64)>> = BTreeMap::new();
        for d in diagnostics {
            let points = severity_weight(d.severity) * confidence_factor(d.confidence);
            let entry = raw
                .entry(d.category.as_str())
                .or_default()
                .entry(d.rule_id.as_str())
                .or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += points;
        }

        let mut categories: BTreeMap<String, CategoryScore> = BTreeMap::new();
        let mut all: Vec<&str> = MONITORED_CATEGORIES.to_vec();
        for cat in raw.keys() {
            if !all.contains(cat) {
                all.push(cat);
            }
        }

        for cat in &all {
            let mut deductions: Vec<Deduction> = raw
                .get(cat)
                .map(|rules| {
                    rules
                        .iter()
                        .map(|(rule_id, (count, points))| Deduction {
                            rule_id: (*rule_id).to_string(),
                            count: *count,
                            points: (points * 10.0).round() / 10.0,
                        })
                        .collect()
                })
                .unwrap_or_default();
            deductions.sort_by(|a, b| {
                b.points
                    .partial_cmp(&a.points)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.rule_id.cmp(&b.rule_id))
            });
            let total: f64 = deductions.iter().map(|d| d.points).sum();
            let score = (100.0 - total).clamp(0.0, 100.0).round() as u8;
            categories.insert((*cat).to_string(), CategoryScore { score, deductions });
        }

        let overall = if categories.is_empty() {
            100
        } else {
            let sum: u32 = categories.values().map(|c| c.score as u32).sum();
            (sum as f64 / categories.len() as f64).round() as u8
        };

        Self {
            overall,
            categories,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_core::{Position, SourceLocation, Span};

    fn diag(rule: &str, category: &str, sev: Severity, conf: Confidence) -> Diagnostic {
        Diagnostic::new(
            rule,
            "t",
            "m",
            category,
            sev,
            conf,
            SourceLocation::new(
                "x.ts",
                Span::new(Position::new(1, 1), Position::new(1, 1), 0, 0),
            ),
        )
    }

    #[test]
    fn clean_project_scores_100() {
        let card = Scorecard::compute(&[]);
        assert_eq!(card.overall, 100);
        for cat in MONITORED_CATEGORIES {
            assert_eq!(card.categories[*cat].score, 100);
        }
    }

    #[test]
    fn deductions_are_attributed() {
        let diags = vec![
            diag(
                "graph/no-circular-imports",
                "architecture",
                Severity::High,
                Confidence::Certain,
            ),
            diag(
                "graph/dead-file",
                "architecture",
                Severity::Low,
                Confidence::Possible,
            ),
        ];
        let card = Scorecard::compute(&diags);
        let arch = &card.categories["architecture"];
        // 100 - 15*1.0 - 3*0.4 = 83.8 → 84
        assert_eq!(arch.score, 84);
        assert_eq!(arch.deductions.len(), 2);
        assert_eq!(arch.deductions[0].rule_id, "graph/no-circular-imports");
        assert_eq!(arch.deductions[0].points, 15.0);
        // Clean categories stay at 100.
        assert_eq!(card.categories["dependencies"].score, 100);
    }

    #[test]
    fn uncertain_findings_hurt_less() {
        let certain =
            Scorecard::compute(&[diag("r", "imports", Severity::Medium, Confidence::Certain)]);
        let possible =
            Scorecard::compute(&[diag("r", "imports", Severity::Medium, Confidence::Possible)]);
        assert!(possible.categories["imports"].score > certain.categories["imports"].score);
    }

    #[test]
    fn score_floors_at_zero() {
        let diags: Vec<Diagnostic> = (0..10)
            .map(|_| diag("sec/x", "security", Severity::Critical, Confidence::Certain))
            .collect();
        let card = Scorecard::compute(&diags);
        assert_eq!(card.categories["security"].score, 0);
    }

    #[test]
    fn deterministic() {
        let diags = vec![
            diag("a", "architecture", Severity::High, Confidence::Likely),
            diag("b", "imports", Severity::Low, Confidence::Certain),
        ];
        assert_eq!(Scorecard::compute(&diags), Scorecard::compute(&diags));
    }
}
