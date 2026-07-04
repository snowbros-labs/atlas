//! Minimal project/workspace model.
//!
//! Grows in later sprints (framework detection, file inventory). For now it
//! anchors a root directory and its configuration.

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::config::Config;

/// A project under analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    /// Absolute root directory of the project.
    pub root: Utf8PathBuf,
    /// Display name (from config, or the root directory name).
    pub name: String,
    /// Effective configuration.
    pub config: Config,
}

impl Project {
    /// Creates a project rooted at `root` with the given configuration.
    /// The name falls back to the root directory's file name.
    pub fn new(root: impl Into<Utf8PathBuf>, config: Config) -> Self {
        let root = root.into();
        let name = config
            .project
            .name
            .clone()
            .or_else(|| root.file_name().map(str::to_string))
            .unwrap_or_else(|| "unnamed".to_string());
        Self { root, name, config }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_falls_back_to_directory() {
        let project = Project::new("C:/work/my-app", Config::default());
        assert_eq!(project.name, "my-app");
    }

    #[test]
    fn name_prefers_config() {
        let mut config = Config::default();
        config.project.name = Some("Custom".into());
        let project = Project::new("C:/work/my-app", config);
        assert_eq!(project.name, "Custom");
    }
}
