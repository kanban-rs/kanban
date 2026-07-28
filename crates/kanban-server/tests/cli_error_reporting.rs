use assert_cmd::Command;
use predicates::prelude::*;
use std::io::{BufRead, BufReader};
use std::process::{Command as StdCommand, Stdio};
use std::time::Duration;
use tempfile::tempdir;

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
    let dir = tempdir().unwrap();
    let bin_path = assert_cmd::cargo_bin!("kanban-server");

    // `tracing_subscriber::fmt::init()` (used by main.rs) defaults to stdout,
    // not stderr — that's the stream that carries the "listening addr=..."
    // line this test needs to read.
    let mut child = StdCommand::new(bin_path)
        .current_dir(dir.path())
        .env_remove("KANBAN_FILE")
        .env("NO_COLOR", "1")
        .env("RUST_LOG", "info")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn kanban-server");

    let stdout = child.stdout.take().expect("stdout must be piped");

    // `BufReader::read_line` blocks with no timeout of its own, so a deadline
    // checked only between calls would never fire against a single stuck
    // read — do the blocking read on a background thread and bound the wait
    // with a channel `recv_timeout` instead.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF: process exited without ever logging the port
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

    // A real write is required to prove which filename the server actually
    // resolved to — the file is created lazily on first write, not at
    // startup, so merely starting the server proves nothing either way.
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:{port}/v1/boards"))
        .json(&serde_json::json!({"name": "Default Location Probe", "card_prefix": "DL"}))
        .send()
        .expect("failed to POST board");
    assert!(resp.status().is_success(), "POST /v1/boards must succeed");

    let _ = child.kill();
    let _ = child.wait();

    assert!(
        dir.path().join("boards.json").exists(),
        "boards.json (the shared kanban-cli/kanban-mcp default) must be created after a write"
    );
    assert!(
        !dir.path().join("kanban.json").exists(),
        "kanban.json (the old kanban-server-specific default) must not be created"
    );
}
