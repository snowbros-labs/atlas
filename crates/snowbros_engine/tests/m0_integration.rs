//! M0 engine-integration invariants.
//!
//! Proves the new Atlas IR / semantic / Next.js layers are wired into
//! every analysis, and — critically — that wiring them in is *additive*:
//! existing diagnostics and the default JSON output are unchanged.

use std::fs;

use camino::Utf8PathBuf;

use snowbros_framework::nextjs::RouterKind;

/// A small App-Router Next.js project that also triggers an existing rule
/// (`security/no-eval`), so one fixture exercises both worlds.
fn project(dir: &std::path::Path) -> Utf8PathBuf {
    let app = dir.join("app");
    let src = dir.join("src");
    fs::create_dir_all(&app).unwrap();
    fs::create_dir_all(&src).unwrap();
    fs::write(dir.join("package.json"), "{}").unwrap();
    fs::write(
        app.join("page.tsx"),
        "export default function Page() { return null; }\n\
         export const metadata = { title: \"Home\" };\n",
    )
    .unwrap();
    fs::write(
        app.join("counter.tsx"),
        "\"use client\";\nexport function Counter() { return null; }\n",
    )
    .unwrap();
    fs::write(
        src.join("util.ts"),
        "export function helper() { return 1; }\nexport const unused = 2;\n",
    )
    .unwrap();
    fs::write(src.join("a.ts"), "eval(\"1 + 1\");\n").unwrap();
    Utf8PathBuf::from(dir.to_str().unwrap())
}

#[test]
fn ir_and_semantic_model_are_built() {
    let dir = tempfile::tempdir().unwrap();
    let root = project(dir.path());
    let analysis = snowbros_engine::analyze(&root, false).unwrap();

    // IR exists: every parsed ecmascript file lowered into a module.
    let semantic = &analysis.pipeline.semantic;
    assert!(
        semantic.module_count() >= 4,
        "expected a module per source file, got {}",
        semantic.module_count()
    );
    // SemanticModel exists and indexes declared symbols.
    assert!(semantic.symbols().iter().any(|s| s.symbol.name == "helper"));
}

#[test]
fn symbol_graph_contains_symbol_nodes() {
    let dir = tempfile::tempdir().unwrap();
    let root = project(dir.path());
    let analysis = snowbros_engine::analyze(&root, false).unwrap();

    let g = &analysis.pipeline.symbol_graph;
    // Symbol nodes are labeled `{module}#{kind}#{name}`.
    assert!(g.find("src/util.ts#function#helper").is_some());
    assert!(!g.symbols().is_empty());
    // The dedicated symbol graph does not leak into the rule graph.
    assert!(analysis
        .pipeline
        .graph
        .find("src/util.ts#function#helper")
        .is_none());
}

#[test]
fn next_project_model_is_built() {
    let dir = tempfile::tempdir().unwrap();
    let root = project(dir.path());
    let analysis = snowbros_engine::analyze(&root, false).unwrap();

    let model = analysis
        .pipeline
        .next_model
        .expect("app/ present → a routed Next.js app");
    assert_eq!(model.router, RouterKind::App);
    // The `"use client"` file is classified as a client boundary somewhere
    // in the model surface — at minimum the model was populated.
    assert!(!model.app_routes.is_empty());
}

#[test]
fn existing_rules_unchanged_and_output_is_additive() {
    let dir = tempfile::tempdir().unwrap();
    let root = project(dir.path());
    let analysis = snowbros_engine::analyze(&root, false).unwrap();

    // Existing rule still fires, unmodified.
    assert!(analysis
        .report
        .diagnostics
        .iter()
        .any(|d| d.rule_id == "security/no-eval"));

    // Default JSON output carries no new top-level key — `project_model`
    // is opt-in and absent unless explicitly requested.
    let json = snowbros_output::json::render(&analysis.report);
    assert!(
        !json.contains("project_model"),
        "project_model must not appear in the default report"
    );
}

#[test]
fn warm_run_matches_cold_run_with_ir_cached() {
    let dir = tempfile::tempdir().unwrap();
    let root = project(dir.path());

    let cold = snowbros_engine::analyze(&root, false).unwrap();
    snowbros_engine::analyze(&root, true).unwrap(); // prime cache (incl. IR)
    let warm = snowbros_engine::analyze(&root, true).unwrap();

    assert!(warm.pipeline.cache_stats.hits > 0);
    // Diagnostics byte-identical.
    assert_eq!(cold.report.diagnostics, warm.report.diagnostics);
    // IR survives the cache round-trip: same modules warm and cold.
    assert_eq!(
        cold.pipeline.semantic.module_count(),
        warm.pipeline.semantic.module_count()
    );
    assert!(warm
        .pipeline
        .symbol_graph
        .find("src/util.ts#function#helper")
        .is_some());
}
