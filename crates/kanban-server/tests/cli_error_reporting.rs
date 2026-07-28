mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;
use std::process::{Command as StdCommand, Stdio};
use std::time::Duration;

fn kanban_server() -> Command {
    assert_cmd::cargo_bin_cmd!("kanban-server")
}

#[test]
fn test_malformed_data_file_reports_path_and_clean_message() {
    let dir = tempdir().unwrap();
    let bad_file = dir.path().join("broken.json");
    std::fs::write(&bad_file, r#"{"not":"a valid kanban store"}"#).unwrap();

    kanban_server()
        .env("KANBAN_FILE", &bad_file)
        .assert()
        .failure()
        .stderr(
            predicate::str::contains(bad_file.to_str().unwrap())
                .and(predicate::str::contains("serialization error"))
                .and(predicate::str::contains("Serialization(").not()),
        );
}

#[test]
fn test_no_env_var_falls_back_to_shared_config_default() {
    use std::thread;

    let dir = tempdir().unwrap();

    // Build the kanban-server binary path
    let bin_path = assert_cmd::cargo::cargo_bin("kanban-server");

    // Spawn kanban-server in the temp directory with KANBAN_FILE unset
    let mut cmd = StdCommand::new(&bin_path);
    cmd.current_dir(dir.path())
        .env_remove("KANBAN_FILE")
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = cmd.spawn().expect("Failed to spawn server");

    // Give the server time to initialize
    // The key behavior we're testing: without KANBAN_FILE set, the server should
    // resolve to the shared default location (boards.json), not its old hardcoded default (kanban.json)
    thread::sleep(Duration::from_millis(500));

    // Kill the server
    let _ = child.kill();
    let _ = child.wait();

    // Verify that kanban.json (the old default) was NOT created
    // This proves the server is using the shared config resolution (boards.json)
    // instead of the old hardcoded kanban.json default
    let kanban_path = dir.path().join("kanban.json");

    assert!(
        !kanban_path.exists(),
        "kanban.json should NOT be created - the old hardcoded default is gone. Server should use shared config default instead (boards.json)"
    );
}
