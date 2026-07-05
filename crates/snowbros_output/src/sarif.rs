//! SARIF v2.1.0 output for code-scanning integrations (GitHub, GitLab,
//! Azure DevOps).
//!
//! Hand-rolled against the SARIF schema rather than pulling a heavy
//! dependency: the subset we emit (one run, driver rules, results with
//! physical locations) is small and stable. Output is deterministic —
//! rules sorted by id, results in report order.

use serde_json::{json, Value};

use snowbros_core::{Diagnostic, Severity};

use crate::report::Report;

/// SARIF severity levels.
fn level(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical | Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low | Severity::Info => "note",
    }
}

fn rule_descriptor(d: &Diagnostic) -> Value {
    json!({
        "id": d.rule_id,
        "name": d.rule_id,
        "shortDescription": { "text": d.title },
        "helpUri": d.docs_url.clone().unwrap_or_else(||
            format!("https://snowbros.dev/rules/{}", d.rule_id)),
    })
}

fn result(d: &Diagnostic) -> Value {
    json!({
        "ruleId": d.rule_id,
        "level": level(d.severity),
        "message": { "text": d.message },
        "locations": [{
            "physicalLocation": {
                "artifactLocation": { "uri": d.location.file.as_str() },
                "region": {
                    "startLine": d.location.span.start.line,
                    "startColumn": d.location.span.start.column,
                    "endLine": d.location.span.end.line,
                    "endColumn": d.location.span.end.column,
                }
            }
        }],
        "properties": {
            "confidence": d.confidence.to_string(),
            "category": d.category,
        }
    })
}

/// Renders a report as a SARIF v2.1.0 document.
pub fn render(report: &Report) -> String {
    // One descriptor per distinct rule, sorted by id.
    let mut seen: Vec<&str> = Vec::new();
    let mut rules: Vec<Value> = Vec::new();
    let mut diags_sorted: Vec<&Diagnostic> = report.diagnostics.iter().collect();
    diags_sorted.sort_by_key(|d| &d.rule_id);
    for d in diags_sorted {
        if !seen.contains(&d.rule_id.as_str()) {
            seen.push(&d.rule_id);
            rules.push(rule_descriptor(d));
        }
    }

    let results: Vec<Value> = report.diagnostics.iter().map(result).collect();

    let doc = json!({
        "$schema": "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "Snowbros Atlas",
                    "informationUri": "https://snowbros.dev",
                    "version": report.engine_version,
                    "rules": rules,
                }
            },
            "results": results,
        }]
    });
    serde_json::to_string_pretty(&doc).expect("SARIF document is always serializable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_core::{Confidence, Position, SourceLocation, Span};

    fn sample() -> Report {
        Report::new(vec![
            Diagnostic::new(
                "graph/no-circular-imports",
                "Circular import chain",
                "2 files import each other.",
                "architecture",
                Severity::High,
                Confidence::Certain,
                SourceLocation::new(
                    "src/a.ts",
                    Span::new(Position::new(1, 1), Position::new(1, 1), 0, 0),
                ),
            ),
            Diagnostic::new(
                "deps/unused-dependency",
                "Unused dependency",
                "`lodash` is never imported.",
                "dependencies",
                Severity::Low,
                Confidence::Likely,
                SourceLocation::new(
                    "package.json",
                    Span::new(Position::new(1, 1), Position::new(1, 1), 0, 0),
                ),
            ),
        ])
    }

    #[test]
    fn valid_sarif_shape() {
        let sarif: serde_json::Value = serde_json::from_str(&render(&sample())).unwrap();
        assert_eq!(sarif["version"], "2.1.0");
        let run = &sarif["runs"][0];
        assert_eq!(run["tool"]["driver"]["name"], "Snowbros Atlas");
        assert_eq!(run["results"].as_array().unwrap().len(), 2);
        assert_eq!(run["tool"]["driver"]["rules"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn severity_maps_to_sarif_levels() {
        let sarif: serde_json::Value = serde_json::from_str(&render(&sample())).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        // Report sorts by (file, byte, rule): package.json before src/a.ts.
        assert_eq!(results[0]["level"], "note");
        assert_eq!(results[1]["level"], "error");
    }

    #[test]
    fn deterministic() {
        assert_eq!(render(&sample()), render(&sample()));
    }
}
