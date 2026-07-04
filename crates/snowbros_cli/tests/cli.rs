//! End-to-end CLI tests.

use assert_cmd::Command;
use predicates::prelude::*;

fn snowbros() -> Command {
    Command::cargo_bin("snowbros").expect("binary builds")
}

#[test]
fn version_flag_works() {
    snowbros()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("snowbros"));
}

#[test]
fn init_creates_config() {
    let dir = tempfile::tempdir().unwrap();
    snowbros()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success();
    let written = std::fs::read_to_string(dir.path().join("snowbros.toml")).unwrap();
    assert!(written.contains("[analysis]"));
}

#[test]
fn init_refuses_overwrite_without_force() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("snowbros.toml"), "# existing").unwrap();
    snowbros()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn analyze_detects_circular_imports() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("a.ts"),
        r#"import { b } from "./b"; export const a = 1;"#,
    )
    .unwrap();
    std::fs::write(
        src.join("b.ts"),
        r#"import { a } from "./a"; export const b = 2;"#,
    )
    .unwrap();
    std::fs::write(src.join("clean.ts"), "export const c = 3;").unwrap();

    snowbros()
        .current_dir(dir.path())
        .args(["analyze", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("graph/no-circular-imports"))
        .stdout(predicate::str::contains("src/a.ts"))
        .stdout(predicate::str::contains("src/b.ts"));
}

#[test]
fn analyze_clean_project_reports_nothing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("main.ts"),
        r#"import { helper } from "./util"; helper();"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("util.ts"),
        "export const helper = () => {};",
    )
    .unwrap();

    snowbros()
        .current_dir(dir.path())
        .args(["analyze", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"total\": 0"));
}

#[test]
fn warm_run_output_identical_to_cold_run() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.ts"),
        r#"import { b } from "./b"; export const a = 1;"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("b.ts"),
        r#"import { a } from "./a"; export const b = 2;"#,
    )
    .unwrap();

    let cold = snowbros()
        .current_dir(dir.path())
        .args(["analyze", "--format", "json"])
        .output()
        .unwrap();
    // Second run: everything served from cache.
    let warm = snowbros()
        .current_dir(dir.path())
        .args(["analyze", "--format", "json"])
        .output()
        .unwrap();
    // No-cache run: forced cold.
    let no_cache = snowbros()
        .current_dir(dir.path())
        .args(["analyze", "--format", "json", "--no-cache"])
        .output()
        .unwrap();

    assert_eq!(
        String::from_utf8_lossy(&cold.stdout),
        String::from_utf8_lossy(&warm.stdout),
        "warm run must be byte-identical to cold run"
    );
    assert_eq!(
        String::from_utf8_lossy(&cold.stdout),
        String::from_utf8_lossy(&no_cache.stdout)
    );
}

#[test]
fn cache_picks_up_file_changes_and_deletions() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.ts"), "export const x = 1;").unwrap();
    std::fs::write(dir.path().join("orphan.ts"), "export const o = 1;").unwrap();

    // Prime the cache; orphan.ts reported as dead file.
    snowbros()
        .current_dir(dir.path())
        .args(["analyze", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("orphan.ts"));

    // Change: main.ts now imports orphan → dead-file finding must vanish.
    std::fs::write(
        dir.path().join("main.ts"),
        r#"import { o } from "./orphan"; export const x = o;"#,
    )
    .unwrap();
    snowbros()
        .current_dir(dir.path())
        .args(["analyze", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("graph/dead-file").not());

    // Deletion: orphan.ts removed → its import becomes unresolved.
    std::fs::remove_file(dir.path().join("orphan.ts")).unwrap();
    snowbros()
        .current_dir(dir.path())
        .args(["analyze", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("imports/unresolved-import"));
}

#[test]
fn init_force_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("snowbros.toml"), "# existing").unwrap();
    snowbros()
        .current_dir(dir.path())
        .args(["init", "--force"])
        .assert()
        .success();
    let written = std::fs::read_to_string(dir.path().join("snowbros.toml")).unwrap();
    assert!(written.contains("[analysis]"));
}
