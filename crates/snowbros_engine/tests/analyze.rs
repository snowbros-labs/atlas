//! End-to-end engine tests: `analyze` must produce findings and be
//! byte-identical between cold and warm runs.

use std::fs;

use camino::Utf8PathBuf;

/// Writes a minimal project with one guaranteed finding (`eval`).
fn project(dir: &std::path::Path) -> Utf8PathBuf {
    let src = dir.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(dir.join("package.json"), "{}").unwrap();
    fs::write(src.join("a.ts"), "eval(\"1 + 1\");\n").unwrap();
    Utf8PathBuf::from(dir.to_str().unwrap())
}

#[test]
fn analyze_returns_findings_and_report() {
    let dir = tempfile::tempdir().unwrap();
    let root = project(dir.path());

    let analysis = snowbros_engine::analyze(&root, false).unwrap();
    assert!(analysis
        .report
        .diagnostics
        .iter()
        .any(|d| d.rule_id == "security/no-eval"));
    assert!(!analysis.pipeline.scanned.files.is_empty());
}

#[test]
fn warm_run_matches_cold_run() {
    let dir = tempfile::tempdir().unwrap();
    let root = project(dir.path());

    let cold = snowbros_engine::analyze(&root, false).unwrap();
    snowbros_engine::analyze(&root, true).unwrap(); // prime cache
    let warm = snowbros_engine::analyze(&root, true).unwrap();

    assert!(warm.pipeline.cache_stats.hits > 0);
    assert_eq!(cold.report.diagnostics, warm.report.diagnostics);
}

#[test]
fn missing_root_is_an_error() {
    let root = Utf8PathBuf::from("Z:/definitely/not/a/dir");
    assert!(snowbros_engine::analyze(&root, false).is_err());
}
