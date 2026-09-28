//! Integration test for `KANBAN_FILE` env var support, mirroring
//! `kanban-server`'s `cli_error_reporting.rs`.

use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;
use tempfile::tempdir;

fn kanban_mcp() -> Command {
    assert_cmd::cargo_bin_cmd!("kanban-mcp")
}

#[test]
fn test_kanban_file_env_var_is_honored_with_no_positional_arg() {
    let dir = tempdir().unwrap();
    let bad_file = dir.path().join("broken.json");
    std::fs::write(&bad_file, r#"{"not":"a valid kanban store"}"#).unwrap();

    kanban_mcp()
        .current_dir(dir.path())
        .env("KANBAN_FILE", &bad_file)
        .env_remove("KANBAN_CONFIG")
        .env("RUST_LOG", "off")
        .timeout(Duration::from_secs(5))
        .assert()
        .failure()
        .stderr(
            predicate::str::contains(bad_file.to_str().unwrap())
                .and(predicate::str::contains("serialization error")),
        );
}
