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
