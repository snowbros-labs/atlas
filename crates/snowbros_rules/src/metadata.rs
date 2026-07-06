//! Rule metadata: documentation, maturity, and examples for every rule.
//!
//! Metadata lives in TOML files under `rules/` in this crate and is
//! embedded at compile time — the binary stays self-contained and the
//! registry can never drift from the shipped rules (a test enforces
//! 1:1 coverage).
//!
//! TOML rather than YAML: the YAML ecosystem crate (`serde_yaml`) is
//! unmaintained (RUSTSEC-2024-0320) and TOML is already in-tree. YAML
//! remains reserved for the future Semgrep-style pattern rule format.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use snowbros_core::{Confidence, Severity};

/// Rule lifecycle stage (Biome-style graduation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Maturity {
    /// New; may produce noise; disabled-by-default candidates.
    Nursery,
    /// Behavior settled, still gathering field feedback.
    Preview,
    /// Trusted; breaking changes require a major version.
    Stable,
}

/// A code example attached to a rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Example {
    /// Code that should (or should not) trigger the rule.
    pub code: String,
    /// One-line explanation of why.
    pub note: String,
}

/// Everything documented about one rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleMetadata {
    /// Stable id, e.g. `security/no-eval`.
    pub id: String,
    /// Human title.
    pub title: String,
    /// Category (matches the diagnostic category).
    pub category: String,
    /// Default severity the rule reports at.
    pub severity: Severity,
    /// Typical confidence of its findings.
    pub confidence: Confidence,
    /// Lifecycle stage.
    pub maturity: Maturity,
    /// What the rule detects.
    pub description: String,
    /// Why it matters.
    pub rationale: String,
    /// What to do about findings.
    pub recommendation: String,
    /// Known false-positive scenarios and the guards in place.
    pub false_positives: String,
    /// Reference URLs.
    #[serde(default)]
    pub references: Vec<String>,
    /// Search tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Examples that trigger the rule.
    #[serde(default)]
    pub bad_examples: Vec<Example>,
    /// Examples that must NOT trigger the rule.
    #[serde(default)]
    pub good_examples: Vec<Example>,
}

/// The embedded metadata sources, one per shipped rule.
const METADATA_SOURCES: &[&str] = &[
    include_str!("../rules/architecture/dead-file.toml"),
    include_str!("../rules/architecture/no-circular-imports.toml"),
    include_str!("../rules/architecture/private-env-in-client.toml"),
    include_str!("../rules/architecture/server-only-in-client.toml"),
    include_str!("../rules/architecture/unused-export.toml"),
    include_str!("../rules/dependencies/unused-dependency.toml"),
    include_str!("../rules/environment/unused-env-var.toml"),
    include_str!("../rules/imports/unresolved-import.toml"),
    include_str!("../rules/nextjs/client-metadata-ignored.toml"),
    include_str!("../rules/nextjs/mixed-router.toml"),
    include_str!("../rules/performance/forced-dynamic.toml"),
    include_str!("../rules/react/async-client-component.toml"),
    include_str!("../rules/security/hardcoded-secret.toml"),
    include_str!("../rules/security/no-eval.toml"),
    include_str!("../rules/typescript/duplicate-declaration.toml"),
    include_str!("../rules/typescript/unused-export.toml"),
];

/// Registry of all rule metadata, keyed by rule id.
pub static METADATA: LazyLock<BTreeMap<String, RuleMetadata>> = LazyLock::new(|| {
    METADATA_SOURCES
        .iter()
        .map(|src| {
            let meta: RuleMetadata =
                toml::from_str(src).expect("embedded rule metadata is valid TOML");
            (meta.id.clone(), meta)
        })
        .collect()
});

impl RuleMetadata {
    /// Human label for the maturity stage.
    pub fn maturity_label(&self) -> &'static str {
        match self.maturity {
            Maturity::Nursery => "nursery",
            Maturity::Preview => "preview",
            Maturity::Stable => "stable",
        }
    }
}

/// Looks up metadata for a rule id.
pub fn rule_metadata(id: &str) -> Option<&'static RuleMetadata> {
    METADATA.get(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::builtin_rules;

    /// The harness: every shipped rule has complete metadata, and every
    /// metadata file corresponds to a shipped rule.
    #[test]
    fn metadata_covers_exactly_the_builtin_rules() {
        let rule_ids: Vec<String> = builtin_rules().iter().map(|r| r.id().to_string()).collect();
        for id in &rule_ids {
            assert!(
                rule_metadata(id).is_some(),
                "rule `{id}` is shipped but has no metadata file"
            );
        }
        for id in METADATA.keys() {
            assert!(
                rule_ids.contains(id),
                "metadata `{id}` exists but no such rule is shipped"
            );
        }
    }

    #[test]
    fn metadata_is_complete() {
        for meta in METADATA.values() {
            let id = &meta.id;
            assert!(!meta.description.is_empty(), "{id}: empty description");
            assert!(!meta.rationale.is_empty(), "{id}: empty rationale");
            assert!(
                !meta.recommendation.is_empty(),
                "{id}: empty recommendation"
            );
            assert!(
                !meta.false_positives.is_empty(),
                "{id}: empty false_positives"
            );
            assert!(
                !meta.bad_examples.is_empty(),
                "{id}: needs at least one bad example"
            );
        }
    }
}
