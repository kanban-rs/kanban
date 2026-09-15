use assert_cmd::{cargo_bin_cmd, Command};
use predicates::prelude::*;
use std::time::Duration;

fn kanban() -> Command {
    cargo_bin_cmd!("kanban")
}

fn kanban_no_config(dir: &std::path::Path) -> Command {
    let mut cmd = kanban();
    cmd.current_dir(dir)
        .env_remove("KANBAN_FILE")
        .env_remove("XDG_CONFIG_HOME")
        .env("HOME", dir)
        .env("KANBAN_CONFIG", dir.join("config.toml"));
    cmd
}

#[test]
fn test_cli_subcommand_on_a_url_locator_is_not_a_missing_file_error() {
    let dir = tempfile::tempdir().unwrap();
    kanban_no_config(dir.path())
        .args(["http://127.0.0.1:1", "board", "list"])
        .timeout(Duration::from_secs(60))
        .assert()
        .failure()
        .stderr(predicate::str::contains("Board file not found").not())
        .stderr(predicate::str::contains("/http:/").not());
}

#[cfg(feature = "http")]
#[test]
fn test_cli_on_an_unreachable_url_locator_exits_with_a_transport_error_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    kanban_no_config(dir.path())
        .args(["http://127.0.0.1:1", "board", "list"])
        .timeout(Duration::from_secs(60))
        .assert()
        .code(1)
        .stderr(predicate::str::contains("transport error"))
        .stderr(predicate::str::contains("health probe"))
        .stderr(predicate::str::contains("Cannot drop a runtime").not());
}

#[test]
fn test_cli_init_on_a_url_locator_is_rejected_with_guidance() {
    let dir = tempfile::tempdir().unwrap();
    kanban_no_config(dir.path())
        .args(["http://127.0.0.1:1", "init"])
        .timeout(Duration::from_secs(60))
        .assert()
        .failure()
        .stderr(predicate::str::contains("is a remote server"));
}

#[test]
fn test_cli_migrate_from_a_url_source_is_rejected_with_guidance() {
    let dir = tempfile::tempdir().unwrap();
    kanban_no_config(dir.path())
        .args(["migrate", "http://127.0.0.1:1", "json"])
        .timeout(Duration::from_secs(60))
        .assert()
        .failure()
        .stderr(predicate::str::contains("is a remote server"));

    let src = dir.path().join("src.json");
    std::fs::write(&src, "{}").unwrap();
    kanban_no_config(dir.path())
        .args([
            "migrate",
            src.to_str().unwrap(),
            "json",
            "-o",
            "http://127.0.0.1:1",
        ])
        .timeout(Duration::from_secs(60))
        .assert()
        .failure()
        .stderr(predicate::str::contains("is a remote server"));
}
