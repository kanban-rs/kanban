use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

fn kanban_server() -> Command {
    assert_cmd::cargo_bin_cmd!("kanban-server")
}

fn version_or_unstable() -> predicates::str::RegexPredicate {
    predicate::str::is_match(concat!("(", env!("CARGO_PKG_VERSION"), "|unstable)")).unwrap()
}

#[test]
fn test_dash_capital_v_prints_version_or_unstable_and_exits_zero() {
    let dir = tempdir().unwrap();

    kanban_server()
        .current_dir(dir.path())
        .env_remove("KANBAN_FILE")
        .arg("-V")
        .assert()
        .success()
        .stdout(version_or_unstable());
}

#[test]
fn test_dash_dash_version_prints_version_or_unstable_and_exits_zero() {
    let dir = tempdir().unwrap();

    kanban_server()
        .current_dir(dir.path())
        .env_remove("KANBAN_FILE")
        .arg("--version")
        .assert()
        .success()
        .stdout(version_or_unstable());
}

#[test]
fn test_unstable_output_always_carries_a_commit_line() {
    let dir = tempdir().unwrap();

    let output = kanban_server()
        .current_dir(dir.path())
        .env_remove("KANBAN_FILE")
        .arg("-V")
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    if stdout.starts_with("unstable") {
        assert!(
            stdout.contains("commit: "),
            "unstable output must carry a commit line, got: {stdout}"
        );
    }
}

#[test]
fn test_kanban_version_const_remains_semver_parseable() {
    let parts: Vec<&str> = kanban_core::version::KANBAN_VERSION.split('.').collect();
    assert_eq!(
        parts.len(),
        3,
        "KANBAN_VERSION must stay MAJOR.MINOR.PATCH, got: {}",
        kanban_core::version::KANBAN_VERSION
    );
    for part in parts {
        assert!(
            part.parse::<u64>().is_ok(),
            "KANBAN_VERSION component {part:?} must be numeric"
        );
    }
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
