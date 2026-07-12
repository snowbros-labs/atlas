//! Tree-sitter integration for the JS/TS family.
//!
//! Produces error-tolerant concrete syntax trees. Grammar selection is
//! driven by [`Language`]; unsupported languages return
//! [`ParseError::UnsupportedLanguage`] instead of guessing.

use tree_sitter::{Parser, Tree};

use crate::language::Language;

/// Errors from the parsing layer.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// The language has no grammar wired up yet.
    #[error("no grammar available for language `{0}`")]
    UnsupportedLanguage(Language),
    /// Tree-sitter rejected the grammar (version mismatch — a build
    /// configuration bug, not a user error).
    #[error("grammar failed to load: {0}")]
    Grammar(#[from] tree_sitter::LanguageError),
    /// Tree-sitter returned no tree (cancellation or timeout; we set
    /// neither, so this indicates an internal bug).
    #[error("parser produced no tree")]
    NoTree,
}

/// A parsed source file: the tree plus the source it was parsed from.
///
/// Keeping source and tree together lets callers slice node text without
/// re-reading files.
#[derive(Debug)]
pub struct ParsedFile {
    /// Language the file was parsed as.
    pub language: Language,
    /// Source text.
    pub source: String,
    /// Tree-sitter concrete syntax tree.
    pub tree: Tree,
}

impl ParsedFile {
    /// Byte slice of the source covered by a node.
    pub fn text_of(&self, node: tree_sitter::Node<'_>) -> &str {
        &self.source[node.byte_range()]
    }

    /// Whether the tree contains any syntax errors (parsing is
    /// error-tolerant; analysis may still proceed on the valid parts).
    pub fn has_errors(&self) -> bool {
        self.tree.root_node().has_error()
    }
}

/// Returns the Tree-sitter grammar for a language, if wired up.
fn grammar(language: Language) -> Option<tree_sitter::Language> {
    match language {
        Language::JavaScript | Language::Jsx => Some(tree_sitter_javascript::LANGUAGE.into()),
        Language::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        Language::Tsx => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        Language::Python => Some(tree_sitter_python::LANGUAGE.into()),
        _ => None,
    }
}

/// Parses source text as the given language.
pub fn parse(source: impl Into<String>, language: Language) -> Result<ParsedFile, ParseError> {
    let grammar = grammar(language).ok_or(ParseError::UnsupportedLanguage(language))?;
    let mut parser = Parser::new();
    parser.set_language(&grammar)?;
    let source = source.into();
    let tree = parser.parse(&source, None).ok_or(ParseError::NoTree)?;
    Ok(ParsedFile {
        language,
        source,
        tree,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_typescript() {
        let parsed = parse("const x: number = 1;", Language::TypeScript).unwrap();
        assert!(!parsed.has_errors());
        assert_eq!(parsed.tree.root_node().kind(), "program");
    }

    #[test]
    fn parses_tsx_component() {
        let src = "export const App = () => <div>hello</div>;";
        let parsed = parse(src, Language::Tsx).unwrap();
        assert!(!parsed.has_errors());
    }

    #[test]
    fn tolerates_syntax_errors() {
        let parsed = parse("const = ;;;", Language::JavaScript).unwrap();
        assert!(parsed.has_errors());
    }

    #[test]
    fn parses_valid_python() {
        let parsed = parse("def greet(name):\n    return name\n", Language::Python).unwrap();
        assert!(!parsed.has_errors());
        assert_eq!(parsed.tree.root_node().kind(), "module");
    }

    #[test]
    fn rejects_unsupported_language() {
        // Go has no grammar wired up yet.
        let err = parse("package main", Language::Go).unwrap_err();
        assert!(matches!(err, ParseError::UnsupportedLanguage(_)));
    }
}
