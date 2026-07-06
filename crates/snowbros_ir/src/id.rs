//! Stable identifiers for IR entities.
//!
//! Ids are the join key between the IR, the semantic layer, the symbol
//! graph, and the incremental cache. They must be **stable** (the same
//! declaration in unchanged source yields the same id across re-parses)
//! and **sortable** (so any collection keyed by id has deterministic
//! order).
//!
//! The scheme is `path#kind#name@startByte-endByte`:
//! - `path` scopes the id to a module, so equal names in different files
//!   never collide;
//! - `kind` disambiguates, e.g. a value and a type sharing a name;
//! - `name` is the declared identifier;
//! - the byte range makes two identically-named declarations in one file
//!   (rare, but legal for overloads / re-declarations) distinct, and is
//!   stable as long as the surrounding bytes do not shift.

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use snowbros_core::Span;

/// Identifier of a module (one source file).
///
/// A module's id is simply its project-relative path, wrapped so callers
/// cannot confuse it with a symbol id or a bare string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ModuleId(String);

impl ModuleId {
    /// Builds a module id from its project-relative path.
    pub fn new(path: impl AsRef<Utf8Path>) -> Self {
        Self(path.as_ref().as_str().to_string())
    }

    /// The underlying string form.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ModuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Globally-unique, stable, sortable identifier of a declared symbol.
///
/// Construct via [`SymbolId::new`]. The string form is
/// `path#kind#name@startByte-endByte` (see the module docs).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SymbolId(String);

impl SymbolId {
    /// Builds a symbol id from the owning module path, the symbol's kind
    /// tag (see [`crate::SymbolKind::tag`]), its name, and its span.
    pub fn new(path: impl AsRef<Utf8Path>, kind_tag: &str, name: &str, span: Span) -> Self {
        Self(format!(
            "{}#{}#{}@{}-{}",
            path.as_ref().as_str(),
            kind_tag,
            name,
            span.start_byte,
            span.end_byte
        ))
    }

    /// The underlying string form.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SymbolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_core::Position;

    fn span(a: u32, b: u32) -> Span {
        Span::new(Position::new(1, 1), Position::new(1, 1), a, b)
    }

    #[test]
    fn symbol_id_format_is_stable() {
        let id = SymbolId::new("src/app/page.tsx", "function", "Page", span(40, 120));
        assert_eq!(id.as_str(), "src/app/page.tsx#function#Page@40-120");
    }

    #[test]
    fn same_name_different_span_are_distinct() {
        let a = SymbolId::new("a.ts", "function", "f", span(0, 10));
        let b = SymbolId::new("a.ts", "function", "f", span(20, 30));
        assert_ne!(a, b);
    }

    #[test]
    fn same_name_different_kind_are_distinct() {
        // A value `Foo` and a type `Foo` at the same location must differ.
        let val = SymbolId::new("a.ts", "const", "Foo", span(0, 10));
        let ty = SymbolId::new("a.ts", "class", "Foo", span(0, 10));
        assert_ne!(val, ty);
    }

    #[test]
    fn ids_sort_lexicographically() {
        let mut ids = [
            SymbolId::new("b.ts", "function", "z", span(0, 1)),
            SymbolId::new("a.ts", "function", "a", span(0, 1)),
            SymbolId::new("a.ts", "function", "a", span(2, 3)),
        ];
        ids.sort();
        assert_eq!(ids[0].as_str(), "a.ts#function#a@0-1");
        assert_eq!(ids[1].as_str(), "a.ts#function#a@2-3");
        assert_eq!(ids[2].as_str(), "b.ts#function#z@0-1");
    }

    #[test]
    fn serde_roundtrip() {
        let id = SymbolId::new("x.ts", "class", "C", span(5, 9));
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"x.ts#class#C@5-9\"");
        let back: SymbolId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }
}
