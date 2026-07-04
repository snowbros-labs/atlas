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
