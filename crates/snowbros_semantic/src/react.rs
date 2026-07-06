//! React semantic enrichment (M1).
//!
//! Structural classification of symbols into React roles — **component**
//! and **custom hook** — layered on top of the language-agnostic
//! [`SemanticModel`]. This reads only Atlas IR affordances (a function's
//! `returns_jsx` flag and its name), so it stays deterministic and never
//! touches a tree-sitter node.
//!
//! Scope is deliberately minimal for v0.2.1: recognize components and
//! hooks with high precision. Rules-of-Hooks flow analysis and prop/effect
//! modeling are later work.

use crate::model::{SemanticModel, SymbolRef};
use snowbros_ir::SymbolKind;

/// The React role a symbol plays, when it plays one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactRole {
    /// A component: a function that returns JSX and is PascalCase (or a
    /// default-exported JSX-returning function).
    Component,
    /// A custom hook: a function named `useX` (camelCase `use` prefix).
    Hook,
}

/// Whether a name is PascalCase (starts with an ASCII uppercase letter).
/// React uses the leading capital to distinguish components from host
/// elements, so this is the canonical structural test.
fn is_pascal_case(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
}

/// Whether a name is a custom-hook name: `use` followed by an uppercase
/// letter or digit (`useState`, `use2fa`) — the React naming contract.
pub fn is_hook_name(name: &str) -> bool {
    name.strip_prefix("use")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// Classifies a symbol's React role, if any. Hooks take precedence over
/// components: a `useX` function is a hook even if it returns JSX.
pub fn role_of(symbol: SymbolRef<'_>) -> Option<ReactRole> {
    let SymbolKind::Function(data) = &symbol.symbol.kind else {
        return None;
    };
    let name = &symbol.symbol.name;
    if is_hook_name(name) {
        return Some(ReactRole::Hook);
    }
    // A default-exported JSX function is a component even though its
    // recorded name is `default`.
    if data.returns_jsx && (is_pascal_case(name) || name == "default") {
        return Some(ReactRole::Component);
    }
    None
}

impl SemanticModel {
    /// Every symbol classified as a React component, in model order.
    pub fn react_components(&self) -> Vec<SymbolRef<'_>> {
        self.symbols()
            .into_iter()
            .filter(|s| role_of(*s) == Some(ReactRole::Component))
            .collect()
    }

    /// Every symbol classified as a custom hook, in model order.
    pub fn react_hooks(&self) -> Vec<SymbolRef<'_>> {
        self.symbols()
            .into_iter()
            .filter(|s| role_of(*s) == Some(ReactRole::Hook))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snowbros_parser::{lower, parse, Language};

    fn model(src: &str) -> SemanticModel {
        SemanticModel::from_modules([lower(&parse(src, Language::Tsx).unwrap(), "c.tsx")])
    }

    #[test]
    fn pascal_jsx_function_is_a_component() {
        let m = model("export function Card() { return <div/>; }");
        let names: Vec<&str> = m
            .react_components()
            .iter()
            .map(|s| s.symbol.name.as_str())
            .collect();
        assert_eq!(names, vec!["Card"]);
        assert!(m.react_hooks().is_empty());
    }

    #[test]
    fn arrow_component_detected() {
        let m = model("export const Panel = () => <section/>;");
        assert_eq!(m.react_components().len(), 1);
    }

    #[test]
    fn default_export_component_detected() {
        let m = model("export default function Page() { return <main/>; }");
        let comps = m.react_components();
        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0].symbol.name, "default");
    }

    #[test]
    fn use_prefixed_function_is_a_hook() {
        let m = model("export function useCounter() { return 0; }");
        let names: Vec<&str> = m
            .react_hooks()
            .iter()
            .map(|s| s.symbol.name.as_str())
            .collect();
        assert_eq!(names, vec!["useCounter"]);
        assert!(m.react_components().is_empty());
    }

    #[test]
    fn hook_wins_over_component_when_jsx_returned() {
        // `useX` that returns JSX is still classified as a hook.
        let m = model("export function useView() { return <div/>; }");
        assert_eq!(m.react_hooks().len(), 1);
        assert!(m.react_components().is_empty());
    }

    #[test]
    fn lowercase_jsx_helper_is_not_a_component() {
        // Returns JSX but camelCase and not a hook → neither role.
        let m = model("export function renderRow() { return <tr/>; }");
        assert!(m.react_components().is_empty());
        assert!(m.react_hooks().is_empty());
    }

    #[test]
    fn plain_function_and_use_word_are_ignored() {
        // `use` not followed by uppercase is not a hook (e.g. `useful`).
        let m = model("export function useful() { return 1; }\nexport const x = 2;");
        assert!(m.react_hooks().is_empty());
        assert!(m.react_components().is_empty());
    }
}
