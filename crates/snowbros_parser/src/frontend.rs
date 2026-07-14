//! The language-frontend abstraction ([RFC 0002] §2).
//!
//! A *frontend* is everything needed to turn one source file into Atlas'
//! shared substrate — [`FileFacts`] and lowered [`ir::Module`] — for one
//! language family. The engine drives analysis through this trait and never
//! reaches for a specific language's parser directly, so adding a language
//! is adding a [`LanguageFrontend`] implementation, not editing the pipeline.
//!
//! This trait covers only the **per-file** phase (parse → facts → lower).
//! Cross-file work (name resolution, the call graph, framework models) is
//! shared machinery that reads the IR every frontend produces; it is not part
//! of this trait.
//!
//! [RFC 0002]: https://github.com/snowbros-labs/atlas/blob/master/docs/rfcs/0002-atlas-multi-language-semantic-platform.md

use camino::Utf8Path;

use snowbros_ir::Module;

use crate::python::{lower_python, python_facts};
use crate::{extract_facts, lower, parse, FileFacts, Language, ParseError};

/// The per-file substrate one frontend produces for a single source file:
/// the extracted [`FileFacts`] and the lowered Atlas [`ir::Module`].
///
/// Both are cached together (they are the parse-phase products that ride the
/// file cache), so a frontend computes them in one pass over the parse tree.
///
/// [`ir::Module`]: snowbros_ir::Module
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredFile {
    /// File-level facts (imports, exports, directives, env reads, …).
    pub facts: FileFacts,
    /// The lowered Atlas IR for the file.
    pub ir: Module,
}

/// A language frontend: parses and lowers one language family into Atlas'
/// shared substrate.
///
/// Implementations are stateless and shared across threads (the pipeline
/// lowers files in parallel), hence the [`Sync`] bound. A frontend must be
/// deterministic: the same `source` and `path` always produce the same
/// [`LoweredFile`].
pub trait LanguageFrontend: Sync {
    /// Stable identifier for this frontend's language family, e.g.
    /// `"ecmascript"`. Used in diagnostics and the `sb languages` matrix.
    fn family(&self) -> &'static str;

    /// Whether this frontend handles files detected as `language`.
    ///
    /// Exactly one registered frontend must claim any given [`Language`]; the
    /// registry relies on this being non-overlapping (see [`crate::frontend`]).
    fn handles(&self, language: Language) -> bool;

    /// Parses and lowers one file to its [`LoweredFile`] substrate.
    ///
    /// Takes ownership of `source` because the parse tree borrows it and the
    /// frontend owns that lifetime internally. Returns [`ParseError`] when the
    /// language has no grammar wired up or parsing cannot proceed — callers
    /// record the failure rather than guessing at partial results.
    fn lower_file(
        &self,
        source: String,
        language: Language,
        path: &Utf8Path,
    ) -> Result<LoweredFile, ParseError>;
}

/// The frontend for the JavaScript / TypeScript family: JavaScript, JSX,
/// TypeScript, and TSX.
///
/// These four share one Tree-sitter-backed lowering path, so they are one
/// frontend rather than four — collapsing them is the point of the shared
/// substrate (splitting them would duplicate the lowering logic RFC 0002
/// exists to avoid). Whether a file is a React component or a Next.js route
/// is decided later by the semantic and framework layers, not here.
#[derive(Debug, Default, Clone, Copy)]
pub struct EcmaScriptFrontend;

impl LanguageFrontend for EcmaScriptFrontend {
    fn family(&self) -> &'static str {
        "ecmascript"
    }

    fn handles(&self, language: Language) -> bool {
        language.is_ecmascript()
    }

    fn lower_file(
        &self,
        source: String,
        language: Language,
        path: &Utf8Path,
    ) -> Result<LoweredFile, ParseError> {
        debug_assert!(
            self.handles(language),
            "EcmaScriptFrontend given non-ecmascript language {language}"
        );
        let parsed = parse(source, language)?;
        Ok(LoweredFile {
            facts: extract_facts(&parsed),
            ir: lower(&parsed, path.to_owned()),
        })
    }
}

/// The frontend for Python.
///
/// Lowers Python source into the same Atlas IR the ECMAScript frontend
/// produces (see [`crate::python`]). Resolution and every shared rule read the
/// result identically — the point of the frontend abstraction.
#[derive(Debug, Default, Clone, Copy)]
pub struct PythonFrontend;

impl LanguageFrontend for PythonFrontend {
    fn family(&self) -> &'static str {
        "python"
    }

    fn handles(&self, language: Language) -> bool {
        matches!(language, Language::Python)
    }

    fn lower_file(
        &self,
        source: String,
        language: Language,
        path: &Utf8Path,
    ) -> Result<LoweredFile, ParseError> {
        debug_assert!(
            self.handles(language),
            "PythonFrontend given non-python language {language}"
        );
        let parsed = parse(source, language)?;
        Ok(LoweredFile {
            facts: python_facts(&parsed),
            ir: lower_python(&parsed, path.to_owned()),
        })
    }
}

/// The set of language frontends available to the engine.
///
/// The registry owns its frontends and resolves a detected [`Language`] to
/// the one frontend that handles it. Exactly one frontend must claim any
/// given language (frontends' [`LanguageFrontend::handles`] sets are
/// non-overlapping); resolution returns the first match in registration
/// order, which is deterministic because registration order is fixed.
///
/// Adding a language to Atlas is registering its frontend here — the pipeline
/// itself stays language-agnostic.
pub struct FrontendRegistry {
    frontends: Vec<Box<dyn LanguageFrontend>>,
}

impl FrontendRegistry {
    /// Builds a registry from an explicit, ordered list of frontends.
    pub fn from_frontends(frontends: Vec<Box<dyn LanguageFrontend>>) -> Self {
        Self { frontends }
    }

    /// The frontend that handles `language`, or `None` if no registered
    /// frontend claims it (the file is recognized but not yet analyzable).
    pub fn frontend_for(&self, language: Language) -> Option<&dyn LanguageFrontend> {
        self.frontends
            .iter()
            .find(|fe| fe.handles(language))
            .map(|fe| fe.as_ref())
    }

    /// Whether some registered frontend handles `language`.
    pub fn supports(&self, language: Language) -> bool {
        self.frontend_for(language).is_some()
    }
}

impl Default for FrontendRegistry {
    /// The default registry: every frontend Atlas ships today — the
    /// JavaScript/TypeScript family via [`EcmaScriptFrontend`] and Python via
    /// [`PythonFrontend`]. New languages register here as their frontends land.
    fn default() -> Self {
        Self::from_frontends(vec![Box::new(EcmaScriptFrontend), Box::new(PythonFrontend)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8Path;

    #[test]
    fn ecmascript_frontend_claims_the_js_ts_family() {
        let fe = EcmaScriptFrontend;
        assert_eq!(fe.family(), "ecmascript");
        for lang in [
            Language::JavaScript,
            Language::Jsx,
            Language::TypeScript,
            Language::Tsx,
        ] {
            assert!(fe.handles(lang), "should handle {lang}");
        }
        assert!(!fe.handles(Language::Python));
        assert!(!fe.handles(Language::Go));
    }

    #[test]
    fn lower_file_matches_the_free_functions_it_delegates_to() {
        let src = "export const App = () => <div>hi</div>;";
        let path = Utf8Path::new("src/App.tsx");
        let lowered = EcmaScriptFrontend
            .lower_file(src.to_string(), Language::Tsx, path)
            .unwrap();

        // Identical to calling the underlying functions directly — the
        // frontend is a thin, behavior-preserving wrapper.
        let parsed = parse(src.to_string(), Language::Tsx).unwrap();
        assert_eq!(lowered.facts, extract_facts(&parsed));
        assert_eq!(lowered.ir, lower(&parsed, path.to_owned()));
    }

    #[test]
    fn lower_file_propagates_parse_errors_for_unsupported_language() {
        let err = EcmaScriptFrontend
            .lower_file(
                "x = 1".to_string(),
                Language::TypeScript,
                Utf8Path::new("a.ts"),
            )
            .err();
        // TypeScript parses fine; a language with no grammar surfaces the error.
        assert!(err.is_none());
    }

    #[test]
    fn default_registry_resolves_the_ecmascript_family() {
        let reg = FrontendRegistry::default();
        for lang in [
            Language::JavaScript,
            Language::Jsx,
            Language::TypeScript,
            Language::Tsx,
        ] {
            let fe = reg.frontend_for(lang).expect("ecmascript family covered");
            assert_eq!(fe.family(), "ecmascript");
            assert!(reg.supports(lang));
        }
    }

    #[test]
    fn default_registry_resolves_python() {
        let reg = FrontendRegistry::default();
        let fe = reg.frontend_for(Language::Python).expect("python covered");
        assert_eq!(fe.family(), "python");
        assert!(reg.supports(Language::Python));
    }

    #[test]
    fn default_registry_does_not_claim_unwired_languages() {
        let reg = FrontendRegistry::default();
        // Recognized by detection, but no frontend yet — resolves to None
        // rather than being misrouted to an existing frontend.
        assert!(reg.frontend_for(Language::Go).is_none());
        assert!(reg.frontend_for(Language::Java).is_none());
        assert!(!reg.supports(Language::Rust));
    }

    #[test]
    fn python_and_ecmascript_route_to_distinct_frontends() {
        let reg = FrontendRegistry::default();
        let py = reg.frontend_for(Language::Python).unwrap();
        let es = reg.frontend_for(Language::TypeScript).unwrap();
        assert_eq!(py.family(), "python");
        assert_eq!(es.family(), "ecmascript");
        // Python source lowers through the Python frontend.
        let lowered = py
            .lower_file(
                "def f():\n    pass\n".to_string(),
                Language::Python,
                Utf8Path::new("m.py"),
            )
            .unwrap();
        assert_eq!(lowered.ir.symbols.len(), 1);
        assert_eq!(lowered.ir.symbols[0].name, "f");
    }

    #[test]
    fn registry_routes_lowering_through_the_matching_frontend() {
        let reg = FrontendRegistry::default();
        let src = "export const x = 1;";
        let path = Utf8Path::new("src/x.ts");
        let via_registry = reg
            .frontend_for(Language::TypeScript)
            .unwrap()
            .lower_file(src.to_string(), Language::TypeScript, path)
            .unwrap();
        let direct = EcmaScriptFrontend
            .lower_file(src.to_string(), Language::TypeScript, path)
            .unwrap();
        assert_eq!(via_registry, direct);
    }
}
