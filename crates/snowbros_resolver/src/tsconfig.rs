//! `tsconfig.json` path-alias support.
//!
//! Reads `compilerOptions.baseUrl` and `compilerOptions.paths` (JSONC
//! tolerated, single-level `extends` followed) and turns them into a
//! deterministic alias matcher: exact patterns first, then wildcard
//! patterns by descending prefix length — mirroring TypeScript itself.

use std::fs;

use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::jsonc;

/// Raw shape of the parts of tsconfig we read.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawTsConfig {
    extends: Option<String>,
    compiler_options: RawCompilerOptions,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RawCompilerOptions {
    base_url: Option<String>,
    paths: Option<std::collections::BTreeMap<String, Vec<String>>>,
}

/// One alias mapping, pre-split at the `*`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AliasPattern {
    /// Text before `*` (whole pattern when no `*`).
    prefix: String,
    /// Text after `*`, if the pattern has a wildcard.
    suffix: Option<String>,
    /// Substitution targets, root-relative, with `*` intact.
    targets: Vec<String>,
}

/// Compiled tsconfig alias table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TsPaths {
    /// Patterns in match priority order: exact first, then wildcards by
    /// descending prefix length.
    patterns: Vec<AliasPattern>,
}

impl TsPaths {
    /// Loads alias mappings from `<root>/tsconfig.json`, following one
    /// level of `extends`. Returns an empty table when no config or no
    /// paths exist — absence of aliases is not an error.
    pub fn load(root: &Utf8Path) -> Self {
        Self::load_file(root, &root.join("tsconfig.json"), true)
    }

    fn load_file(root: &Utf8Path, file: &Utf8Path, follow_extends: bool) -> Self {
        let Ok(text) = fs::read_to_string(file) else {
            return Self::default();
        };
        let Ok(raw) = serde_json::from_str::<RawTsConfig>(&jsonc::to_json(&text)) else {
            return Self::default();
        };

        let mut base = Self::default();
        if follow_extends {
            if let Some(parent) = &raw.extends {
                // Only local file extends; package extends ("@tsconfig/…")
                // would require node_modules resolution we don't do here.
                if parent.starts_with('.') {
                    let parent_path = file
                        .parent()
                        .unwrap_or(Utf8Path::new(""))
                        .join(ensure_json_ext(parent));
                    base = Self::load_file(root, &parent_path, false);
                }
            }
        }

        let Some(paths) = raw.compiler_options.paths else {
            return base;
        };
        let base_url = raw.compiler_options.base_url.unwrap_or_else(|| ".".into());
        let config_dir = file.parent().unwrap_or(Utf8Path::new(""));

        let mut patterns: Vec<AliasPattern> = paths
            .into_iter()
            .map(|(pattern, targets)| {
                let (prefix, suffix) = match pattern.split_once('*') {
                    Some((p, s)) => (p.to_string(), Some(s.to_string())),
                    None => (pattern, None),
                };
                let targets = targets
                    .into_iter()
                    .map(|t| {
                        // Targets are relative to baseUrl, which is
                        // relative to the config file's directory.
                        let joined = config_dir.join(&base_url).join(t);
                        let rel = joined.strip_prefix(root).unwrap_or(&joined);
                        rel.as_str().replace('\\', "/")
                    })
                    .collect();
                AliasPattern {
                    prefix,
                    suffix,
                    targets,
                }
            })
            .collect();

        // Child paths win over extended-parent paths with the same pattern.
        base.patterns.retain(|p| {
            !patterns
                .iter()
                .any(|c| c.prefix == p.prefix && c.suffix == p.suffix)
        });
        patterns.extend(base.patterns);

        // Priority: exact matches, then longest prefix.
        patterns.sort_by(|a, b| {
            a.suffix
                .is_some()
                .cmp(&b.suffix.is_some())
                .then(b.prefix.len().cmp(&a.prefix.len()))
        });

        Self { patterns }
    }

    /// Whether any alias patterns are configured.
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Expands a specifier into candidate root-relative paths, in match
    /// priority order. Empty when no pattern matches.
    pub fn expand(&self, specifier: &str) -> Vec<Utf8PathBuf> {
        for pattern in &self.patterns {
            match &pattern.suffix {
                None => {
                    if specifier == pattern.prefix {
                        return pattern
                            .targets
                            .iter()
                            .map(|t| normalize_slashes(t))
                            .collect();
                    }
                }
                Some(suffix) => {
                    if let Some(rest) = specifier.strip_prefix(pattern.prefix.as_str()) {
                        if let Some(captured) = rest.strip_suffix(suffix.as_str()) {
                            return pattern
                                .targets
                                .iter()
                                .map(|t| normalize_slashes(&t.replace('*', captured)))
                                .collect();
                        }
                    }
                }
            }
        }
        Vec::new()
    }
}

fn ensure_json_ext(path: &str) -> String {
    if path.ends_with(".json") {
        path.to_string()
    } else {
        format!("{path}.json")
    }
}

fn normalize_slashes(path: &str) -> Utf8PathBuf {
    let unified = path.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for component in unified.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    Utf8PathBuf::from(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(root: &std::path::Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn root_of(dir: &tempfile::TempDir) -> Utf8PathBuf {
        Utf8PathBuf::from(dir.path().to_str().unwrap())
    }

    #[test]
    fn next_style_alias() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "tsconfig.json",
            r#"{
  "compilerOptions": {
    // Next.js default
    "paths": { "@/*": ["./src/*"], }
  }
}"#,
        );
        let ts = TsPaths::load(&root_of(&dir));
        assert_eq!(
            ts.expand("@/components/ui/button"),
            vec![Utf8PathBuf::from("src/components/ui/button")]
        );
        assert!(ts.expand("react").is_empty());
    }

    #[test]
    fn base_url_applies() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "tsconfig.json",
            r#"{ "compilerOptions": { "baseUrl": "src", "paths": { "~lib/*": ["lib/*"] } } }"#,
        );
        let ts = TsPaths::load(&root_of(&dir));
        assert_eq!(
            ts.expand("~lib/api"),
            vec![Utf8PathBuf::from("src/lib/api")]
        );
    }

    #[test]
    fn exact_pattern_beats_wildcard() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "tsconfig.json",
            r#"{ "compilerOptions": { "paths": {
                "config": ["./src/special/config"],
                "conf*": ["./src/generic/*"]
            } } }"#,
        );
        let ts = TsPaths::load(&root_of(&dir));
        assert_eq!(
            ts.expand("config"),
            vec![Utf8PathBuf::from("src/special/config")]
        );
    }

    #[test]
    fn extends_merges_child_wins() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "tsconfig.base.json",
            r##"{ "compilerOptions": { "paths": {
                "@/*": ["./old/*"],
                "#shared/*": ["./shared/*"]
            } } }"##,
        );
        write(
            dir.path(),
            "tsconfig.json",
            r#"{ "extends": "./tsconfig.base.json",
                 "compilerOptions": { "paths": { "@/*": ["./src/*"] } } }"#,
        );
        let ts = TsPaths::load(&root_of(&dir));
        assert_eq!(ts.expand("@/x"), vec![Utf8PathBuf::from("src/x")]);
        assert_eq!(ts.expand("#shared/y"), vec![Utf8PathBuf::from("shared/y")]);
    }

    #[test]
    fn missing_tsconfig_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(TsPaths::load(&root_of(&dir)).is_empty());
    }
}
