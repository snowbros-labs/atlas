//! M1 React engine-integration coverage.
//!
//! Proves the React classification and its rules run through the full
//! engine pipeline (lower → semantic → rules), that `returns_jsx` survives
//! the IR cache round-trip, and that results are deterministic.

use std::fs;

use camino::Utf8PathBuf;

/// A project exercising every M1 React rule exactly once.
fn project(dir: &std::path::Path) -> Utf8PathBuf {
    let src = dir.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(dir.join("package.json"), "{}").unwrap();
    // async Client Component → react/async-client-component
    fs::write(
        src.join("widget.tsx"),
        "\"use client\";\nexport default async function Widget() { return <div/>; }\n",
    )
    .unwrap();
    // lowercase JSX helper → react/component-naming
    fs::write(
        src.join("row.tsx"),
        "export function row() { return <tr/>; }\n",
    )
    .unwrap();
    // useX returning JSX → react/hook-returns-jsx
    fs::write(
        src.join("card.tsx"),
        "export function useCard() { return <div/>; }\n",
    )
    .unwrap();
    // hook call in a plain function → react/hook-in-non-component
    fs::write(
        src.join("setup.ts"),
        "export function setup() { const x = useState(0); return x; }\n",
    )
    .unwrap();
    Utf8PathBuf::from(dir.to_str().unwrap())
}

fn rule_ids(analysis: &snowbros_engine::Analysis) -> Vec<String> {
    let mut ids: Vec<String> = analysis
        .report
        .diagnostics
        .iter()
        .map(|d| d.rule_id.clone())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

#[test]
fn all_m1_react_rules_fire_through_the_engine() {
    let dir = tempfile::tempdir().unwrap();
    let root = project(dir.path());
    let analysis = snowbros_engine::analyze(&root, false).unwrap();
    let ids = rule_ids(&analysis);

    for expected in [
        "react/async-client-component",
        "react/component-naming",
        "react/hook-in-non-component",
        "react/hook-returns-jsx",
    ] {
        assert!(ids.contains(&expected.to_string()), "missing {expected}");
    }
}

#[test]
fn returns_jsx_survives_cache_and_results_are_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    let root = project(dir.path());

    let cold = snowbros_engine::analyze(&root, false).unwrap();
    snowbros_engine::analyze(&root, true).unwrap(); // prime cache (incl. IR)
    let warm = snowbros_engine::analyze(&root, true).unwrap();

    assert!(warm.pipeline.cache_stats.hits > 0);
    // Warm run must reproduce the React findings from cached IR.
    assert_eq!(cold.report.diagnostics, warm.report.diagnostics);
    // The component classification is intact after the cache round-trip.
    assert!(!warm.pipeline.semantic.react_components().is_empty());
}

#[test]
fn non_react_helpers_are_not_misclassified() {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from(dir.path().to_str().unwrap());
    let src = dir.path().join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(dir.path().join("package.json"), "{}").unwrap();
    // Plain TS: a value-returning function and a value-returning useX hook.
    fs::write(
        src.join("util.ts"),
        "export function add(a, b) { return a + b; }\nexport function useConfig() { return { on: true }; }\n",
    )
    .unwrap();

    let analysis = snowbros_engine::analyze(&root, false).unwrap();
    // No JSX anywhere → no component, and the value-returning hook does not
    // trip hook-returns-jsx.
    assert!(analysis.pipeline.semantic.react_components().is_empty());
    assert!(!rule_ids(&analysis)
        .iter()
        .any(|id| id == "react/hook-returns-jsx"));
}
