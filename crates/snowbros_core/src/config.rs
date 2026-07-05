//! Configuration loaded from `snowbros.toml`.
//!
//! Zero configuration is the default: every field has a sensible default
//! and an empty file (or no file at all) is valid.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::severity::{Confidence, Severity};

/// Root configuration, mirroring `snowbros.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Project-level settings.
    pub project: ProjectConfig,
    /// Analysis thresholds.
    pub analysis: AnalysisConfig,
    /// Rule enable/disable overrides.
    pub rules: RulesConfig,
}

/// Project-level settings.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ProjectConfig {
    /// Optional display name; defaults to the root directory name.
    pub name: Option<String>,
    /// Glob patterns excluded from analysis, in addition to `.gitignore`.
    pub exclude: Vec<String>,
}

/// Analysis thresholds controlling which findings are reported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AnalysisConfig {
    /// Findings below this severity are suppressed.
    pub min_severity: Severity,
    /// Findings below this confidence are suppressed.
    pub min_confidence: Confidence,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            min_severity: Severity::Info,
            min_confidence: Confidence::Possible,
        }
    }
}

/// Rule enable/disable overrides.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RulesConfig {
    /// Rule IDs (or `category/*` globs) to force-enable.
    pub enable: Vec<String>,
    /// Rule IDs (or `category/*` globs) to disable.
    pub disable: Vec<String>,
}

impl Config {
    /// File name the engine looks for at the project root.
    pub const FILE_NAME: &'static str = "snowbros.toml";

    /// Parses configuration from TOML text.
    pub fn from_toml_str(text: &str, path: &str) -> Result<Self, CoreError> {
        toml::from_str(text).map_err(|source| CoreError::ConfigParse {
            path: path.to_string(),
            source,
        })
    }

    /// Loads configuration from a file on disk.
    pub fn load(path: &Path) -> Result<Self, CoreError> {
        let text = fs::read_to_string(path).map_err(|source| CoreError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_toml_str(&text, &path.display().to_string())
    }

    /// The commented starter template written by `snowbros init`.
    pub fn starter_template() -> &'static str {
        r#"# Snowbros Atlas configuration.
# Every setting is optional — an empty file is valid.
# Docs: https://snowbros.dev/docs/configuration

[project]
# name = "my-app"
# Extra exclusions on top of .gitignore:
# exclude = ["**/generated/**"]

[analysis]
# Suppress findings below these thresholds.
# Severity: info | low | medium | high | critical
min_severity = "info"
# Confidence: unknown | possible | likely | certain
min_confidence = "possible"

[rules]
# enable = ["nursery/*"]
# disable = ["performance/large-component"]
"#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_is_valid() {
        let config = Config::from_toml_str("", "snowbros.toml").unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn starter_template_parses_to_default() {
        let config = Config::from_toml_str(Config::starter_template(), "snowbros.toml").unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn unknown_keys_rejected() {
        let result = Config::from_toml_str("[analysis]\nspeed = \"ludicrous\"\n", "snowbros.toml");
        assert!(result.is_err());
    }

    #[test]
    fn thresholds_parse() {
        let config = Config::from_toml_str(
            "[analysis]\nmin_severity = \"high\"\nmin_confidence = \"likely\"\n",
            "snowbros.toml",
        )
        .unwrap();
        assert_eq!(config.analysis.min_severity, Severity::High);
        assert_eq!(config.analysis.min_confidence, Confidence::Likely);
    }
}
