//! `kanban-server` accepts a positional data-file path, matching the
//! established `kanban-cli`/`kanban-mcp` convention (`kanban <path>`,
//! `kanban-mcp <path>`), rather than only reading `KANBAN_FILE`.

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command as StdCommand, Stdio};
use std::time::Duration;
use tempfile::tempdir;

fn kanban_server() -> Command {
    assert_cmd::cargo_bin_cmd!("kanban-server")
}

/// Spawns `kanban-server` with the given extra env vars and CLI args, waits
/// for it to log its bound port on stdout, and returns the child (for
/// cleanup) plus the port. Mirrors `cli_error_reporting.rs`'s own
/// `test_no_env_var_falls_back_to_shared_config_default` spawn/read approach.
fn spawn_and_wait_for_port(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> (Child, u16) {
    let bin_path = assert_cmd::cargo_bin!("kanban-server");

    let mut cmd = StdCommand::new(bin_path);
    cmd.current_dir(dir)
        .env_remove("KANBAN_FILE")
        .env("NO_COLOR", "1")
        .env("RUST_LOG", "info")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (k, v) in env {
        cmd.env(k, v);
    }

    let mut child = cmd.spawn().expect("failed to spawn kanban-server");
    let stdout = child.stdout.take().expect("stdout must be piped");

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if let Some(idx) = line.find("addr=127.0.0.1:") {
                        let rest = &line[idx + "addr=127.0.0.1:".len()..];
                        if let Ok(port) = rest.trim().parse::<u16>() {
                            let _ = tx.send(port);
                        }
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let port = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("server did not report its bound port within 5s");
    (child, port)
}

fn post_board(port: u16) {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/v1/boards"))
        .json(&serde_json::json!({"name": "Positional Arg Probe", "card_prefix": "PA"}))
        .send()
        .expect("failed to POST board");
    assert!(resp.status().is_success(), "POST /v1/boards must succeed");
}

#[test]
fn test_positional_file_arg_is_used_when_provided() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("via-positional.json");

    let (mut child, port) = spawn_and_wait_for_port(dir.path(), &[], &[target.to_str().unwrap()]);
    post_board(port);
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        target.exists(),
        "the positionally-specified file must be created"
    );
    assert!(
        !dir.path().join("boards.json").exists(),
        "the default filename must not be used when a positional arg is given"
    );
}

#[test]
fn test_positional_file_arg_takes_precedence_over_kanban_file_env() {
    let dir = tempdir().unwrap();
    let positional_target = dir.path().join("via-positional.json");
    let env_target = dir.path().join("via-env.json");

    let (mut child, port) = spawn_and_wait_for_port(
        dir.path(),
        &[("KANBAN_FILE", env_target.to_str().unwrap())],
        &[positional_target.to_str().unwrap()],
    );
    post_board(port);
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        positional_target.exists(),
        "an explicit positional argument must win over KANBAN_FILE"
    );
    assert!(
        !env_target.exists(),
        "KANBAN_FILE's file must not be used when a positional arg is also given"
    );
}

#[test]
fn test_help_documents_the_file_argument() {
    kanban_server()
        .env_remove("KANBAN_FILE")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("FILE").or(predicate::str::contains("file")));
}
