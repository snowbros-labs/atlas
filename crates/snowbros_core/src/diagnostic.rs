//! The [`Diagnostic`] type — the single structured result every analyzer
//! produces and every consumer (CLI, LSP, dashboard, SARIF) reads.
//!
//! A diagnostic must always carry evidence. Findings without evidence are
//! not allowed to leave the engine.

use serde::{Deserialize, Serialize};

use crate::severity::{Confidence, Severity};
use crate::span::SourceLocation;

/// A single engineering finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable rule identifier, e.g. `security/no-eval`.
    pub rule_id: String,
    /// Short human-readable title of the finding.
    pub title: String,
    /// Full message explaining what was found and why it matters.
    pub message: String,
    /// Analysis category, e.g. `security`, `performance`, `architecture`.
    pub category: String,
    /// Impact level if the finding is real.
    pub severity: Severity,
    /// How certain the engine is that the finding is real.
    pub confidence: Confidence,
    /// Primary location the finding points at.
    pub location: SourceLocation,
    /// Supporting evidence. Must be non-empty for reported findings.
    pub evidence: Vec<Evidence>,
    /// Deterministic fix suggestion, if one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<SuggestedFix>,
    /// Link to rule documentation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,
}

impl Diagnostic {
    /// Creates a diagnostic with the required fields; evidence and fix are
    /// attached with [`Diagnostic::with_evidence`] / [`Diagnostic::with_fix`].
    pub fn new(
        rule_id: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
        category: impl Into<String>,
        severity: Severity,
        confidence: Confidence,
        location: SourceLocation,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            title: title.into(),
            message: message.into(),
            category: category.into(),
            severity,
            confidence,
            location,
            evidence: Vec::new(),
            suggested_fix: None,
            docs_url: None,
        }
    }

    /// Attaches a piece of evidence.
    pub fn with_evidence(mut self, evidence: Evidence) -> Self {
        self.evidence.push(evidence);
        self
    }

    /// Attaches a suggested fix.
    pub fn with_fix(mut self, fix: SuggestedFix) -> Self {
        self.suggested_fix = Some(fix);
        self
    }

    /// Attaches a documentation link.
    pub fn with_docs_url(mut self, url: impl Into<String>) -> Self {
        self.docs_url = Some(url.into());
        self
    }
}

/// A concrete piece of evidence backing a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    /// What this evidence shows, e.g. "`cookies()` is called here, forcing
    /// dynamic rendering".
    pub description: String,
    /// Where the evidence lives, when it differs from the primary location.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<SourceLocation>,
}

impl Evidence {
    /// Evidence described in prose only (applies to the primary location).
    pub fn note(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            location: None,
        }
    }

    /// Evidence anchored to a specific secondary location.
    pub fn at(description: impl Into<String>, location: SourceLocation) -> Self {
        Self {
            description: description.into(),
            location: Some(location),
        }
    }
}

/// A deterministic, reviewable fix suggestion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestedFix {
    /// Human description of the fix.
    pub description: String,
    /// Replacement text for the diagnostic's span, when the fix is a pure
    /// textual substitution. `None` means the fix requires manual work.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::{Position, Span};

    fn sample_location() -> SourceLocation {
        SourceLocation::new(
            "src/lib/db.ts",
            Span::new(Position::new(10, 1), Position::new(10, 25), 200, 224),
        )
    }

    #[test]
    fn builder_roundtrip() {
        let diag = Diagnostic::new(
            "security/no-eval",
            "Use of eval()",
            "eval() executes arbitrary strings as code.",
            "security",
            Severity::Critical,
            Confidence::Certain,
            sample_location(),
        )
        .with_evidence(Evidence::note("Direct call to eval() with user input"))
        .with_fix(SuggestedFix {
            description: "Use JSON.parse for data parsing".into(),
            replacement: Some("JSON.parse(input)".into()),
        })
        .with_docs_url("https://snowbros.dev/rules/security/no-eval");

        let json = serde_json::to_string_pretty(&diag).unwrap();
        let back: Diagnostic = serde_json::from_str(&json).unwrap();
        assert_eq!(diag, back);
        assert_eq!(back.evidence.len(), 1);
    }

    #[test]
    fn optional_fields_omitted_from_json() {
        let diag = Diagnostic::new(
            "perf/large-component",
            "Large React component",
            "Component exceeds 300 lines.",
            "performance",
            Severity::Low,
            Confidence::Likely,
            sample_location(),
        );
        let json = serde_json::to_string(&diag).unwrap();
        assert!(!json.contains("suggested_fix"));
        assert!(!json.contains("docs_url"));
    }
}
