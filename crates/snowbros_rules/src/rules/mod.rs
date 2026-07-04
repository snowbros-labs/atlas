//! Built-in rule implementations.

pub mod circular;
pub mod dead_files;
pub mod unresolved;
pub mod unused_deps;

use snowbros_core::{Position, SourceLocation, Span};

/// A whole-file location (used when a finding concerns the file itself,
/// not a specific span in it).
pub(crate) fn file_location(path: impl Into<camino::Utf8PathBuf>) -> SourceLocation {
    SourceLocation::new(
        path.into(),
        Span::new(Position::new(1, 1), Position::new(1, 1), 0, 0),
    )
}
