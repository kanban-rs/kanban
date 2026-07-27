use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn kanban_server() -> Command {
    assert_cmd::cargo_bin_cmd!("kanban-server")
}

#[test]
fn test_dash_capital_v_prints_version_and_exits_zero() {
    let dir = tempdir().unwrap();

    kanban_server()
        .current_dir(dir.path())
        .env_remove("KANBAN_FILE")
        .arg("-V")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_dash_dash_version_prints_version_and_exits_zero() {
    let dir = tempdir().unwrap();

    kanban_server()
        .current_dir(dir.path())
        .env_remove("KANBAN_FILE")
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_dash_dash_help_exits_zero() {
    let dir = tempdir().unwrap();

    kanban_server()
        .current_dir(dir.path())
        .env_remove("KANBAN_FILE")
        .arg("--help")
        .assert()
        .success();
}
