//! Applies `snowbros.toml` to raw diagnostics.
//!
//! Precedence, most specific wins:
//! 1. `rules.enable` — matching findings are always kept (overrides
//!    disable and thresholds)
//! 2. `rules.disable` — matching findings are dropped
//! 3. `analysis.min_severity` / `analysis.min_confidence` thresholds
//!
//! Patterns are exact rule ids (`security/no-eval`) or category globs
//! (`security/*`).

use snowbros_core::{Config, Diagnostic};

/// Whether `pattern` matches `rule_id` (exact, or `category/*`).
fn matches(pattern: &str, rule_id: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/*") {
        rule_id
            .split_once('/')
            .is_some_and(|(category, _)| category == prefix)
    } else {
        pattern == rule_id
    }
}

/// Filters diagnostics according to the configuration.
pub fn apply_config(diagnostics: Vec<Diagnostic>, config: &Config) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .filter(|d| {
            let enabled = config.rules.enable.iter().any(|p| matches(p, &d.rule_id));
            if enabled {
                return true;
            }
            let disabled = config.rules.disable.iter().any(|p| matches(p, &d.rule_id));
            if disabled {
                return false;
            }
            d.severity >= config.analysis.min_severity
                && d.confidence >= config.analysis.min_confidence
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_core::{Confidence, Position, Severity, SourceLocation, Span};

    fn diag(rule: &str, sev: Severity, conf: Confidence) -> Diagnostic {
        Diagnostic::new(
            rule,
            "t",
            "m",
            rule.split('/').next().unwrap(),
            sev,
            conf,
            SourceLocation::new(
                "x.ts",
                Span::new(Position::new(1, 1), Position::new(1, 1), 0, 0),
            ),
        )
    }

    fn config(toml: &str) -> Config {
        Config::from_toml_str(toml, "snowbros.toml").unwrap()
    }

    #[test]
    fn disable_exact_and_glob() {
        let cfg = config(
            r#"
[rules]
disable = ["graph/dead-file", "env/*"]
"#,
        );
        let out = apply_config(
            vec![
                diag("graph/dead-file", Severity::Low, Confidence::Possible),
                diag(
                    "graph/no-circular-imports",
                    Severity::High,
                    Confidence::Certain,
                ),
                diag("env/unused-env-var", Severity::Low, Confidence::Possible),
            ],
            &cfg,
        );
        let ids: Vec<&str> = out.iter().map(|d| d.rule_id.as_str()).collect();
        assert_eq!(ids, vec!["graph/no-circular-imports"]);
    }

    #[test]
    fn thresholds_filter() {
        let cfg = config(
            r#"
[analysis]
min_severity = "medium"
min_confidence = "likely"
"#,
        );
        let out = apply_config(
            vec![
                diag("a/low", Severity::Low, Confidence::Certain),
                diag("a/possible", Severity::High, Confidence::Possible),
                diag("a/keeps", Severity::Medium, Confidence::Likely),
            ],
            &cfg,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rule_id, "a/keeps");
    }

    #[test]
    fn enable_overrides_disable_and_thresholds() {
        let cfg = config(
            r#"
[analysis]
min_severity = "high"

[rules]
enable = ["graph/dead-file"]
disable = ["graph/*"]
"#,
        );
        let out = apply_config(
            vec![
                diag("graph/dead-file", Severity::Low, Confidence::Possible),
                diag(
                    "graph/no-circular-imports",
                    Severity::High,
                    Confidence::Certain,
                ),
            ],
            &cfg,
        );
        let ids: Vec<&str> = out.iter().map(|d| d.rule_id.as_str()).collect();
        // dead-file force-enabled; circular dropped by graph/* disable.
        assert_eq!(ids, vec!["graph/dead-file"]);
    }

    #[test]
    fn default_config_passes_default_thresholds() {
        let cfg = Config::default();
        let out = apply_config(
            vec![
                diag("a/x", Severity::Info, Confidence::Possible),
                // Below default min_confidence (possible).
                diag("a/y", Severity::High, Confidence::Unknown),
            ],
            &cfg,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rule_id, "a/x");
    }
}
