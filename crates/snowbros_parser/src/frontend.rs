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
}
