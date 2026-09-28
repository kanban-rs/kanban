//! Integration tests for `KANBAN_FILE` env var support, mirroring
//! `kanban-server`'s `cli_error_reporting.rs` and
//! `cli_positional_file_arg.rs`.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::{tempdir, TempDir};

fn kanban_mcp() -> Command {
    assert_cmd::cargo_bin_cmd!("kanban-mcp")
}

/// The server names the data file after `validate_path` canonicalizes it, so
/// the scratch dir is canonicalized up front; otherwise Windows' 8.3 short
/// temp path (`RUNNER~1`) never matches the long form in stderr.
fn scratch_dir() -> (TempDir, PathBuf) {
    let dir = tempdir().unwrap();
    let canonical = dunce::canonicalize(dir.path()).unwrap();
    (dir, canonical)
}

fn write_broken_store(dir: &Path, name: &str) -> PathBuf {
    let file = dir.join(name);
    std::fs::write(&file, r#"{"not":"a valid kanban store"}"#).unwrap();
    file
}

fn kanban_mcp_in(dir: &Path) -> Command {
    let mut cmd = kanban_mcp();
    cmd.current_dir(dir)
        .env("KANBAN_CONFIG", dir.join("config.toml"))
        .env("RUST_LOG", "off")
        .timeout(Duration::from_secs(5));
    cmd
}

#[test]
fn test_kanban_file_env_var_is_honored_with_no_positional_arg() {
    let (_guard, dir) = scratch_dir();
    let bad_file = write_broken_store(&dir, "broken.json");

    kanban_mcp_in(&dir)
        .env("KANBAN_FILE", &bad_file)
        .assert()
        .failure()
        .stderr(
            predicate::str::contains(bad_file.to_str().unwrap())
                .and(predicate::str::contains("serialization error")),
        );
}

#[test]
fn test_positional_file_arg_takes_precedence_over_kanban_file_env() {
    let (_guard, dir) = scratch_dir();
    let positional = write_broken_store(&dir, "via-positional.json");
    let via_env = write_broken_store(&dir, "via-env.json");

    kanban_mcp_in(&dir)
        .env("KANBAN_FILE", &via_env)
        .arg(&positional)
        .assert()
        .failure()
        .stderr(
            predicate::str::contains(positional.to_str().unwrap())
                .and(predicate::str::contains(via_env.to_str().unwrap()).not()),
        );
}
