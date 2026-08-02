//! Integration tests for configurable server bind address.

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command as StdCommand, Stdio};
use std::time::Duration;
use tempfile::tempdir;

fn kanban_server() -> Command {
    assert_cmd::cargo_bin_cmd!("kanban-server")
}

/// Spawns `kanban-server` with the given env vars and CLI args, waits for it
/// to log its bound address on stdout, and returns the child (for cleanup)
/// plus the actual port it bound to.
fn spawn_and_wait_for_port(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> (Child, u16) {
    let bin_path = assert_cmd::cargo_bin!("kanban-server");

    let mut cmd = StdCommand::new(bin_path);
    cmd.current_dir(dir)
        .env_remove("KANBAN_FILE")
        .env_remove("KANBAN_ADDR")
        .env_remove("KANBAN_CONFIG")
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
                    // Look for "addr=" in the log line (e.g. "addr=127.0.0.1:12345")
                    if let Some(idx) = line.find("addr=") {
                        let rest = &line[idx + "addr=".len()..];
                        if let Some(end) = rest.find([' ', '\n']) {
                            let addr_str = &rest[..end];
                            if let Some(colon_idx) = addr_str.rfind(':') {
                                if let Ok(port) = addr_str[colon_idx + 1..].parse::<u16>() {
                                    let _ = tx.send(port);
                                }
                            }
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

/// Helper to find a free port by binding to 127.0.0.1:0 and reading the port.
fn get_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind to free port");
    listener
        .local_addr()
        .expect("failed to get local addr")
        .port()
}

/// Returns `n` distinct free loopback ports. All listeners are held open at
/// once so the OS hands out different ports, then dropped together. The usual
/// bind-time race still applies, but the returned ports are guaranteed
/// distinct from one another -- which matters for precedence tests that must
/// tell two candidate addresses apart.
fn get_free_ports(n: usize) -> Vec<u16> {
    let listeners: Vec<TcpListener> = (0..n)
        .map(|_| TcpListener::bind("127.0.0.1:0").expect("failed to bind to free port"))
        .collect();
    listeners
        .iter()
        .map(|l| l.local_addr().expect("failed to get local addr").port())
        .collect()
}

#[test]
fn test_kanban_addr_env_binds_requested_port() {
    let dir = tempdir().unwrap();
    let data_file = dir.path().join("test_data.json");
    let requested_port = get_free_port();

    let (mut child, actual_port) = spawn_and_wait_for_port(
        dir.path(),
        &[("KANBAN_ADDR", &format!("127.0.0.1:{}", requested_port))],
        &[data_file.to_str().unwrap()],
    );

    assert_eq!(
        actual_port, requested_port,
        "server should bind to the requested port from KANBAN_ADDR"
    );

    // Try to hit /health on the bound port to confirm it's listening
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/health", actual_port))
        .timeout(Duration::from_secs(2))
        .send()
        .expect("failed to GET /health");
    assert!(resp.status().is_success(), "/health must return 200");

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn test_addr_precedence_flag_over_env() {
    let dir = tempdir().unwrap();
    let data_file = dir.path().join("test_data.json");
    let ports = get_free_ports(2);
    let (env_port, flag_port) = (ports[0], ports[1]);

    let (mut child, actual_port) = spawn_and_wait_for_port(
        dir.path(),
        &[("KANBAN_ADDR", &format!("127.0.0.1:{}", env_port))],
        &[
            "--addr",
            &format!("127.0.0.1:{}", flag_port),
            data_file.to_str().unwrap(),
        ],
    );

    assert_eq!(
        actual_port, flag_port,
        "--addr flag should take precedence over KANBAN_ADDR env"
    );

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn test_config_server_addr_binds_when_no_flag_or_env() {
    let dir = tempdir().unwrap();
    let data_file = dir.path().join("test_data.json");
    let cfg_port = get_free_port();
    let cfg_path = dir.path().join("config.toml");
    std::fs::write(
        &cfg_path,
        format!("server_addr = \"127.0.0.1:{}\"\n", cfg_port),
    )
    .unwrap();

    let (mut child, actual_port) = spawn_and_wait_for_port(
        dir.path(),
        &[("KANBAN_CONFIG", cfg_path.to_str().unwrap())],
        &[data_file.to_str().unwrap()],
    );

    assert_eq!(
        actual_port, cfg_port,
        "server should bind server_addr from the config file when no flag or env is set"
    );

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn test_env_addr_overrides_config_server_addr() {
    let dir = tempdir().unwrap();
    let data_file = dir.path().join("test_data.json");
    let ports = get_free_ports(2);
    let (cfg_port, env_port) = (ports[0], ports[1]);
    let cfg_path = dir.path().join("config.toml");
    std::fs::write(
        &cfg_path,
        format!("server_addr = \"127.0.0.1:{}\"\n", cfg_port),
    )
    .unwrap();

    let (mut child, actual_port) = spawn_and_wait_for_port(
        dir.path(),
        &[
            ("KANBAN_CONFIG", cfg_path.to_str().unwrap()),
            ("KANBAN_ADDR", &format!("127.0.0.1:{}", env_port)),
        ],
        &[data_file.to_str().unwrap()],
    );

    assert_eq!(
        actual_port, env_port,
        "KANBAN_ADDR should take precedence over the config file's server_addr"
    );

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn test_invalid_addr_exits_nonzero() {
    let dir = tempdir().unwrap();
    let data_file = dir.path().join("test_data.json");

    kanban_server()
        .current_dir(dir.path())
        .env("KANBAN_ADDR", "not-an-addr")
        .arg(data_file.to_str().unwrap())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error").or(predicate::str::contains("failed")));
}
