use assert_cmd::{cargo_bin_cmd, Command};
use serde_json::Value;
use std::time::Duration;
use tempfile::tempdir;

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

fn error_envelopes(stderr: &str) -> Vec<Value> {
    stderr
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v.get("success").and_then(Value::as_bool) == Some(false))
        .collect()
}

fn plain_error_lines(stderr: &str) -> Vec<&str> {
    stderr
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter(|line| line.starts_with("Error: "))
        .collect()
}

#[cfg(feature = "http")]
#[test]
fn test_cli_startup_probe_failure_emits_the_cli_response_error_envelope() {
    let dir = tempdir().unwrap();
    let assert = kanban_no_config(dir.path())
        .args(["http://127.0.0.1:1", "board", "list"])
        .timeout(Duration::from_secs(60))
        .assert()
        .code(1);

    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        plain_error_lines(&stderr).is_empty(),
        "expected no plain 'Error: ' line, got stderr: {stderr}"
    );

    let envelopes = error_envelopes(&stderr);
    assert_eq!(
        envelopes.len(),
        1,
        "expected exactly one CliResponse error envelope, got stderr: {stderr}"
    );

    let env = &envelopes[0];
    assert_eq!(env["success"], Value::Bool(false));
    let api_version = env["api_version"].as_str();
    assert!(
        api_version.is_some_and(|v| !v.is_empty()),
        "expected non-empty api_version, got {env:?}"
    );
    let error = env["error"].as_str().unwrap();
    assert!(error.contains("transport error"), "error was: {error}");
    assert!(error.contains("health probe"), "error was: {error}");
}

#[test]
fn test_cli_command_error_stderr_holds_exactly_one_envelope_and_no_plain_error_line() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("missing.json");
    let assert = kanban_no_config(dir.path())
        .args([missing.to_str().unwrap(), "board", "list"])
        .timeout(Duration::from_secs(60))
        .assert()
        .code(1);

    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        plain_error_lines(&stderr).is_empty(),
        "expected no plain 'Error: ' line, got stderr: {stderr}"
    );

    let envelopes = error_envelopes(&stderr);
    assert_eq!(
        envelopes.len(),
        1,
        "expected exactly one CliResponse error envelope, got stderr: {stderr}"
    );

    let error = envelopes[0]["error"].as_str().unwrap();
    assert!(error.contains("Board file not found"), "error was: {error}");
}

#[test]
fn test_cli_json_future_version_refusal_emits_the_cli_response_error_envelope() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("future.json");
    let contents = serde_json::json!({
        "version": 99,
        "metadata": {
            "instance_id": "550e8400-e29b-41d4-a716-446655440000",
            "saved_at": "2030-01-01T00:00:00Z"
        },
        "data": {}
    });
    std::fs::write(&file, serde_json::to_string(&contents).unwrap()).unwrap();
    let before = std::fs::read(&file).unwrap();

    let assert = kanban_no_config(dir.path())
        .args([file.to_str().unwrap(), "board", "list"])
        .timeout(Duration::from_secs(60))
        .assert()
        .code(1);

    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        plain_error_lines(&stderr).is_empty(),
        "expected no plain 'Error: ' line, got stderr: {stderr}"
    );

    let envelopes = error_envelopes(&stderr);
    assert_eq!(
        envelopes.len(),
        1,
        "expected exactly one CliResponse error envelope, got stderr: {stderr}"
    );

    let error = envelopes[0]["error"].as_str().unwrap();
    assert!(error.contains("v99"), "error was: {error}");
    assert!(error.contains("upgrade kanban"), "error was: {error}");

    let after = std::fs::read(&file).unwrap();
    assert_eq!(before, after, "refusal must not modify the file on disk");
}

#[test]
fn test_cli_sqlite_future_version_refusal_emits_the_cli_response_error_envelope() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("future.db");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        kanban_persistence_sqlite::write_test_metadata_with_schema_version(&file, 99)
            .await
            .unwrap();
    });

    let assert = kanban_no_config(dir.path())
        .args([file.to_str().unwrap(), "board", "list"])
        .timeout(Duration::from_secs(60))
        .assert()
        .code(1);

    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        plain_error_lines(&stderr).is_empty(),
        "expected no plain 'Error: ' line, got stderr: {stderr}"
    );

    let envelopes = error_envelopes(&stderr);
    assert_eq!(
        envelopes.len(),
        1,
        "expected exactly one CliResponse error envelope, got stderr: {stderr}"
    );

    let error = envelopes[0]["error"].as_str().unwrap();
    assert!(error.contains("v99"), "error was: {error}");
    assert!(error.contains("upgrade kanban"), "error was: {error}");

    rt.block_on(async {
        let schema_version = kanban_persistence_sqlite::read_test_schema_version(&file)
            .await
            .unwrap();
        assert_eq!(schema_version, Some(99));
    });
}
