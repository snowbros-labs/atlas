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

use crate::{FileFacts, Language, ParseError};

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
