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

fn kanban_server_command(dir: &Path, env: &[(String, String)], args: &[String]) -> StdCommand {
    let mut cmd = StdCommand::new(assert_cmd::cargo_bin!("kanban-server"));
    cmd.current_dir(dir)
        .env_remove("KANBAN_FILE")
        .env_remove("KANBAN_ADDR")
        .env_remove("KANBAN_CONFIG")
        .env("NO_COLOR", "1")
        .env("RUST_LOG", "info")
        .args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd
}

/// Spawns `cmd`, waits for an `addr=HOST:PORT` line on stdout, and returns the
/// child plus that port. On failure the child is reaped and its stderr is
/// included in the error.
fn try_spawn_and_wait_for_port(mut cmd: StdCommand) -> Result<(Child, u16), String> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("failed to spawn: {e}"))?;
    let stdout = child.stdout.take().expect("stdout must be piped");
    let mut stderr = child.stderr.take().expect("stderr must be piped");

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
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
    let stderr_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut stderr, &mut s);
        s
    });

    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(port) => Ok((child, port)),
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            let stderr = stderr_reader.join().unwrap_or_default();
            Err(format!(
                "server did not report its bound port within 5s: {e:?}\n--- child stderr ---\n{stderr}"
            ))
        }
    }
}

fn with_retries<T>(
    attempts: u32,
    mut f: impl FnMut(u32) -> Result<T, String>,
) -> Result<T, String> {
    let mut last = String::new();
    for attempt in 1..=attempts {
        match f(attempt) {
            Ok(v) => return Ok(v),
            Err(msg) => {
                eprintln!("attempt {attempt}/{attempts} failed: {msg}");
                last = msg;
            }
        }
    }
    Err(format!(
        "gave up after {attempts} attempts; last error: {last}"
    ))
}

/// Probes `n` fresh ports per attempt and lets `build` turn them into env and
/// args (and write any config file), so a port lost to a concurrent binder is
/// replaced on the next attempt.
fn spawn_with_fresh_ports(
    dir: &Path,
    n: usize,
    build: impl Fn(&[u16]) -> (Vec<(String, String)>, Vec<String>),
) -> (Child, u16, Vec<u16>) {
    with_retries(3, |_attempt| {
        let ports = get_free_ports(n);
        let (env, args) = build(&ports);
        try_spawn_and_wait_for_port(kanban_server_command(dir, &env, &args))
            .map(|(child, port)| (child, port, ports.clone()))
    })
    .unwrap_or_else(|e| panic!("{e}"))
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

fn failing_child_that_writes_stderr(marker: &str) -> StdCommand {
    #[cfg(windows)]
    {
        let mut c = StdCommand::new("cmd");
        c.args(["/C", &format!("echo {marker} 1>&2 & exit 1")]);
        c
    }
    #[cfg(not(windows))]
    {
        let mut c = StdCommand::new("sh");
        c.args(["-c", &format!("echo {marker} 1>&2; exit 1")]);
        c
    }
}

#[test]
fn test_try_spawn_and_wait_for_port_includes_child_stderr_on_failure() {
    let result = try_spawn_and_wait_for_port(failing_child_that_writes_stderr("kan1700-marker"));
    let msg = result.expect_err(
        "spawning a command that writes to stderr and never reports a port should fail",
    );
    assert!(
        msg.contains("kan1700-marker"),
        "error message should include child stderr: {msg}"
    );
    assert!(
        msg.contains("Disconnected"),
        "error message should show the Debug-formatted RecvTimeoutError: {msg}"
    );
}

#[test]
fn test_with_retries_returns_first_success_without_further_attempts() {
    let mut calls = 0u32;
    let result = with_retries(3, |_attempt| {
        calls += 1;
        if calls == 1 {
            Err("not yet".to_string())
        } else {
            Ok(7)
        }
    });
    assert_eq!(result, Ok(7));
    assert_eq!(calls, 2);
}

#[test]
fn test_with_retries_gives_up_after_the_attempt_limit() {
    let mut calls = 0u32;
    let result: Result<i32, String> = with_retries(3, |_attempt| {
        calls += 1;
        Err("nope".to_string())
    });
    let err = result.expect_err("a closure that always fails should exhaust all attempts");
    assert!(
        err.contains("nope"),
        "error message should include the last failure: {err}"
    );
    assert_eq!(calls, 3);
}

#[test]
fn test_kanban_addr_env_binds_requested_port() {
    let dir = tempdir().unwrap();
    let data_file = dir.path().join("test_data.json");
    let data_arg = data_file.to_str().unwrap().to_string();

    let (mut child, actual_port, ports) = spawn_with_fresh_ports(dir.path(), 1, |p| {
        (
            vec![("KANBAN_ADDR".to_string(), format!("127.0.0.1:{}", p[0]))],
            vec![data_arg.clone()],
        )
    });

    assert_eq!(
        actual_port, ports[0],
        "server should bind to the requested port from KANBAN_ADDR"
    );

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
    let data_arg = data_file.to_str().unwrap().to_string();

    let (mut child, actual_port, ports) = spawn_with_fresh_ports(dir.path(), 2, |p| {
        (
            vec![("KANBAN_ADDR".to_string(), format!("127.0.0.1:{}", p[0]))],
            vec![
                "--addr".to_string(),
                format!("127.0.0.1:{}", p[1]),
                data_arg.clone(),
            ],
        )
    });

    assert_eq!(
        actual_port, ports[1],
        "--addr flag should take precedence over KANBAN_ADDR env"
    );

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn test_config_server_addr_binds_when_no_flag_or_env() {
    let dir = tempdir().unwrap();
    let data_file = dir.path().join("test_data.json");
    let data_arg = data_file.to_str().unwrap().to_string();
    let cfg_path = dir.path().join("config.toml");
    let cfg_path_str = cfg_path.to_str().unwrap().to_string();

    let (mut child, actual_port, ports) = spawn_with_fresh_ports(dir.path(), 1, |p| {
        std::fs::write(&cfg_path, format!("server_addr = \"127.0.0.1:{}\"\n", p[0])).unwrap();
        (
            vec![("KANBAN_CONFIG".to_string(), cfg_path_str.clone())],
            vec![data_arg.clone()],
        )
    });

    assert_eq!(
        actual_port, ports[0],
        "server should bind server_addr from the config file when no flag or env is set"
    );

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn test_env_addr_overrides_config_server_addr() {
    let dir = tempdir().unwrap();
    let data_file = dir.path().join("test_data.json");
    let data_arg = data_file.to_str().unwrap().to_string();
    let cfg_path = dir.path().join("config.toml");
    let cfg_path_str = cfg_path.to_str().unwrap().to_string();

    let (mut child, actual_port, ports) = spawn_with_fresh_ports(dir.path(), 2, |p| {
        std::fs::write(&cfg_path, format!("server_addr = \"127.0.0.1:{}\"\n", p[0])).unwrap();
        (
            vec![
                ("KANBAN_CONFIG".to_string(), cfg_path_str.clone()),
                ("KANBAN_ADDR".to_string(), format!("127.0.0.1:{}", p[1])),
            ],
            vec![data_arg.clone()],
        )
    });

    assert_eq!(
        actual_port, ports[1],
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
