//! Language detection.
//!
//! Detection is deterministic and layered, checked in this order:
//! 1. Well-known file names (`Dockerfile`, `Makefile`, …)
//! 2. File extension (case-insensitive)
//! 3. Shebang line (`#!/usr/bin/env node`, …) for extensionless scripts

use std::fmt;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

/// Languages the engine can recognize.
///
/// Recognition is broader than analysis: the parser recognizes every
/// variant here, but deep analysis initially covers the JS/TS family only
/// (P0 in the language roadmap).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// JavaScript (`.js`, `.mjs`, `.cjs`).
    JavaScript,
    /// JavaScript with JSX (`.jsx`).
    Jsx,
    /// TypeScript (`.ts`, `.mts`, `.cts`).
    TypeScript,
    /// TypeScript with JSX (`.tsx`).
    Tsx,
    /// JSON and JSONC (`.json`, `.jsonc`).
    Json,
    /// CSS (`.css`).
    Css,
    /// HTML (`.html`, `.htm`).
    Html,
    /// Vue single-file components (`.vue`).
    Vue,
    /// Svelte components (`.svelte`).
    Svelte,
    /// Astro components (`.astro`).
    Astro,
    /// Python (`.py`, `.pyi`).
    Python,
    /// Go (`.go`).
    Go,
    /// Rust (`.rs`).
    Rust,
    /// PHP (`.php`).
    Php,
    /// Ruby (`.rb`).
    Ruby,
    /// Java (`.java`).
    Java,
    /// C# (`.cs`).
    CSharp,
    /// SQL (`.sql`).
    Sql,
    /// TOML (`.toml`).
    Toml,
    /// YAML (`.yaml`, `.yml`).
    Yaml,
    /// Markdown (`.md`, `.mdx`).
    Markdown,
    /// Dockerfiles.
    Dockerfile,
    /// POSIX-ish shell scripts (`.sh`, `.bash`, `.zsh`).
    Shell,
}

impl Language {
    /// Detects the language of a file from its path, and optionally the
    /// first line of its contents (for shebang detection).
    ///
    /// Returns `None` for unrecognized files — callers must skip those,
    /// never guess.
    pub fn detect(path: &Utf8Path, first_line: Option<&str>) -> Option<Self> {
        if let Some(lang) = Self::from_file_name(path.file_name()?) {
            return Some(lang);
        }
        if let Some(lang) = path.extension().and_then(Self::from_extension) {
            return Some(lang);
        }
        first_line.and_then(Self::from_shebang)
    }

    /// Detection by well-known file name (checked before extension).
    fn from_file_name(name: &str) -> Option<Self> {
        // Dockerfile, Dockerfile.prod, dev.Dockerfile, …
        let lower = name.to_ascii_lowercase();
        if lower == "dockerfile"
            || lower.starts_with("dockerfile.")
            || lower.ends_with(".dockerfile")
        {
            return Some(Self::Dockerfile);
        }
        None
    }

    /// Detection by file extension (case-insensitive).
    pub fn from_extension(ext: &str) -> Option<Self> {
        let lang = match ext.to_ascii_lowercase().as_str() {
            "js" | "mjs" | "cjs" => Self::JavaScript,
            "jsx" => Self::Jsx,
            "ts" | "mts" | "cts" => Self::TypeScript,
            "tsx" => Self::Tsx,
            "json" | "jsonc" => Self::Json,
            "css" => Self::Css,
            "html" | "htm" => Self::Html,
            "vue" => Self::Vue,
            "svelte" => Self::Svelte,
            "astro" => Self::Astro,
            "py" | "pyi" => Self::Python,
            "go" => Self::Go,
            "rs" => Self::Rust,
            "php" => Self::Php,
            "rb" => Self::Ruby,
            "java" => Self::Java,
            "cs" => Self::CSharp,
            "sql" => Self::Sql,
            "toml" => Self::Toml,
            "yaml" | "yml" => Self::Yaml,
            "md" | "mdx" => Self::Markdown,
            "sh" | "bash" | "zsh" => Self::Shell,
            _ => return None,
        };
        Some(lang)
    }

    /// Detection from a shebang line, for extensionless executables.
    pub fn from_shebang(first_line: &str) -> Option<Self> {
        let line = first_line.strip_prefix("#!")?.trim();
        // Interpreter is the last path segment, or the argument to `env`.
        let mut parts = line.split_ascii_whitespace();
        let program = parts.next()?;
        let interpreter = match program.rsplit('/').next()? {
            "env" => parts.next()?,
            direct => direct,
        };
        // Strip trailing version suffix: python3.12 → python.
        let base = interpreter.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
        let lang = match base {
            "node" | "nodejs" => Self::JavaScript,
            "deno" | "bun" | "ts-node" | "tsx" => Self::TypeScript,
            "python" => Self::Python,
            "ruby" => Self::Ruby,
            "php" => Self::Php,
            "sh" | "bash" | "zsh" | "dash" | "ksh" => Self::Shell,
            _ => return None,
        };
        Some(lang)
    }

    /// Whether this language belongs to the JS/TS family (P0 deep analysis).
    pub fn is_ecmascript(self) -> bool {
        matches!(
            self,
            Self::JavaScript | Self::Jsx | Self::TypeScript | Self::Tsx
        )
    }

    /// Whether files of this language can contain JSX syntax.
    pub fn supports_jsx(self) -> bool {
        matches!(self, Self::Jsx | Self::Tsx)
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::JavaScript => "javascript",
            Self::Jsx => "jsx",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::Json => "json",
            Self::Css => "css",
            Self::Html => "html",
            Self::Vue => "vue",
            Self::Svelte => "svelte",
            Self::Astro => "astro",
            Self::Python => "python",
            Self::Go => "go",
            Self::Rust => "rust",
            Self::Php => "php",
            Self::Ruby => "ruby",
            Self::Java => "java",
            Self::CSharp => "csharp",
            Self::Sql => "sql",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
            Self::Markdown => "markdown",
            Self::Dockerfile => "dockerfile",
            Self::Shell => "shell",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8Path;

    fn detect(path: &str) -> Option<Language> {
        Language::detect(Utf8Path::new(path), None)
    }

    #[test]
    fn detects_js_ts_family() {
        assert_eq!(detect("src/index.js"), Some(Language::JavaScript));
        assert_eq!(detect("src/util.mjs"), Some(Language::JavaScript));
        assert_eq!(detect("src/App.jsx"), Some(Language::Jsx));
        assert_eq!(detect("src/main.ts"), Some(Language::TypeScript));
        assert_eq!(detect("src/Page.tsx"), Some(Language::Tsx));
    }

    #[test]
    fn extension_is_case_insensitive() {
        assert_eq!(detect("legacy/OLD.JS"), Some(Language::JavaScript));
    }

    #[test]
    fn detects_dockerfile_variants() {
        assert_eq!(detect("Dockerfile"), Some(Language::Dockerfile));
        assert_eq!(detect("Dockerfile.prod"), Some(Language::Dockerfile));
        assert_eq!(detect("dev.Dockerfile"), Some(Language::Dockerfile));
    }

    #[test]
    fn detects_shebang_when_no_extension() {
        let p = Utf8Path::new("scripts/deploy");
        assert_eq!(
            Language::detect(p, Some("#!/usr/bin/env node")),
            Some(Language::JavaScript)
        );
        assert_eq!(
            Language::detect(p, Some("#!/usr/bin/python3.12")),
            Some(Language::Python)
        );
        assert_eq!(
            Language::detect(p, Some("#!/bin/bash")),
            Some(Language::Shell)
        );
    }

    #[test]
    fn extension_wins_over_shebang() {
        assert_eq!(
            Language::detect(Utf8Path::new("tool.py"), Some("#!/usr/bin/env node")),
            Some(Language::Python)
        );
    }

    #[test]
    fn unknown_files_are_none_not_guessed() {
        assert_eq!(detect("binary.exe"), None);
        assert_eq!(detect("no_extension_no_shebang"), None);
        assert_eq!(
            Language::detect(Utf8Path::new("weird"), Some("#!/usr/bin/env perl")),
            None
        );
    }

    #[test]
    fn family_helpers() {
        assert!(Language::Tsx.is_ecmascript());
        assert!(Language::Tsx.supports_jsx());
        assert!(!Language::TypeScript.supports_jsx());
        assert!(!Language::Python.is_ecmascript());
    }

    #[test]
    fn serde_lowercase() {
        assert_eq!(
            serde_json::to_string(&Language::TypeScript).unwrap(),
            "\"typescript\""
        );
    }
}
