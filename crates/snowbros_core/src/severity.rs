//! Severity and confidence scales.
//!
//! Two independent axes for every finding:
//! - [`Severity`]: how bad is it if the finding is real?
//! - [`Confidence`]: how sure is the engine that the finding is real?
//!
//! The engine never exaggerates confidence — a rule that cannot prove an
//! issue exists must not report [`Confidence::Certain`].

use std::fmt;

use serde::{Deserialize, Serialize};

/// Impact level of a finding. Ordered: `Info < Low < Medium < High < Critical`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Purely informational; no action required.
    #[default]
    Info,
    /// Minor issue; fix opportunistically.
    Low,
    /// Meaningful issue; should be scheduled.
    Medium,
    /// Serious issue; fix soon.
    High,
    /// Must fix; correctness, security, or availability is at risk.
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Info => "info",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        };
        f.write_str(s)
    }
}

/// How certain the engine is that a finding is real.
/// Ordered: `Unknown < Possible < Likely < Certain`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// The engine cannot judge likelihood; treat as a hint only.
    #[default]
    Unknown,
    /// The pattern matched but legitimate uses exist.
    Possible,
    /// Strong evidence; false positives are rare.
    Likely,
    /// Proven by deterministic analysis (100%).
    Certain,
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Unknown => "unknown",
            Self::Possible => "possible",
            Self::Likely => "likely",
            Self::Certain => "certain",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ordering() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
        assert!(Severity::Low > Severity::Info);
    }

    #[test]
    fn confidence_ordering() {
        assert!(Confidence::Certain > Confidence::Likely);
        assert!(Confidence::Likely > Confidence::Possible);
        assert!(Confidence::Possible > Confidence::Unknown);
    }

    #[test]
    fn serde_lowercase() {
        assert_eq!(
            serde_json::to_string(&Severity::Critical).unwrap(),
            "\"critical\""
        );
        assert_eq!(
            serde_json::from_str::<Confidence>("\"likely\"").unwrap(),
            Confidence::Likely
        );
    }
}
