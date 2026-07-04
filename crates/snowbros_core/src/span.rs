//! Source location types.
//!
//! Positions are 1-based for lines and columns (what editors and humans
//! expect) and byte offsets are 0-based (what parsers produce).

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

/// A 1-based line/column position in a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Position {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number (in UTF-8 bytes, matching LSP `utf-8` encoding).
    pub column: u32,
}

impl Position {
    /// Creates a new position. Both `line` and `column` are 1-based.
    pub fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

/// A contiguous region of a single source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    /// Inclusive start position.
    pub start: Position,
    /// Exclusive end position.
    pub end: Position,
    /// 0-based byte offset of the start (for slicing file contents).
    pub start_byte: u32,
    /// 0-based byte offset one past the end.
    pub end_byte: u32,
}

impl Span {
    /// Creates a span from start/end positions and byte offsets.
    pub fn new(start: Position, end: Position, start_byte: u32, end_byte: u32) -> Self {
        Self {
            start,
            end,
            start_byte,
            end_byte,
        }
    }

    /// Length of the span in bytes.
    pub fn len(&self) -> u32 {
        self.end_byte.saturating_sub(self.start_byte)
    }

    /// Whether the span covers zero bytes.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A span anchored to a concrete file — the unit every finding points at.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceLocation {
    /// Project-relative UTF-8 path of the file.
    pub file: Utf8PathBuf,
    /// Region within the file.
    pub span: Span,
}

impl SourceLocation {
    /// Creates a location from a file path and span.
    pub fn new(file: impl Into<Utf8PathBuf>, span: Span) -> Self {
        Self {
            file: file.into(),
            span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_len() {
        let span = Span::new(Position::new(1, 1), Position::new(1, 5), 0, 4);
        assert_eq!(span.len(), 4);
        assert!(!span.is_empty());
    }

    #[test]
    fn empty_span() {
        let span = Span::new(Position::new(2, 3), Position::new(2, 3), 10, 10);
        assert!(span.is_empty());
    }

    #[test]
    fn location_serde_roundtrip() {
        let loc = SourceLocation::new(
            "src/app/page.tsx",
            Span::new(Position::new(3, 1), Position::new(3, 20), 40, 59),
        );
        let json = serde_json::to_string(&loc).unwrap();
        let back: SourceLocation = serde_json::from_str(&json).unwrap();
        assert_eq!(loc, back);
    }
}
