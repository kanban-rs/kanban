#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command as StdCommand, Stdio};
use std::time::{Duration, Instant};
use tempfile::tempdir;

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

fn send_signal(pid: u32, sig: &str) {
    let status = StdCommand::new("/bin/sh")
        .arg("-c")
        .arg(format!("kill -{sig} {pid}"))
        .status()
        .expect("failed to invoke kill");
    assert!(status.success(), "kill -{sig} {pid} failed");
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> std::process::ExitStatus {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("try_wait failed") {
            return status;
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "process did not exit within {:?} (waited {:?})",
                timeout,
                start.elapsed()
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn test_sigterm_exits_zero_after_drain() {
    let dir = tempdir().unwrap();
    let data_file = dir.path().join("test_data.json");

    let (mut child, _port) = spawn_and_wait_for_port(
        dir.path(),
        &[("KANBAN_SHUTDOWN_GRACE_SECS", "1")],
        &["--addr", "127.0.0.1:0", data_file.to_str().unwrap()],
    );

    send_signal(child.id(), "TERM");
    let status = wait_for_exit(&mut child, Duration::from_secs(5));
    assert_eq!(status.code(), Some(0));
}

#[test]
fn test_sigint_exits_zero_after_drain() {
    let dir = tempdir().unwrap();
    let data_file = dir.path().join("test_data.json");

    let (mut child, _port) = spawn_and_wait_for_port(
        dir.path(),
        &[("KANBAN_SHUTDOWN_GRACE_SECS", "1")],
        &["--addr", "127.0.0.1:0", data_file.to_str().unwrap()],
    );

    send_signal(child.id(), "INT");
    let status = wait_for_exit(&mut child, Duration::from_secs(5));
    assert_eq!(status.code(), Some(0));
}

#[test]
fn test_sigterm_with_open_sse_connection_exits_within_grace() {
    let dir = tempdir().unwrap();
    let data_file = dir.path().join("test_data.json");

    let (mut child, port) = spawn_and_wait_for_port(
        dir.path(),
        &[("KANBAN_SHUTDOWN_GRACE_SECS", "1")],
        &["--addr", "127.0.0.1:0", data_file.to_str().unwrap()],
    );

    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).expect("failed to connect to /v1/events");
    stream
        .write_all(
            b"GET /v1/events HTTP/1.1\r\nHost: 127.0.0.1\r\nAccept: text/event-stream\r\n\r\n",
        )
        .expect("failed to write request");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("failed to set read timeout");
    let mut buf = [0u8; 1024];
    let n = stream.read(&mut buf).expect("failed to read response");
    let text = String::from_utf8_lossy(&buf[..n]);
    assert!(
        text.contains("200"),
        "expected a 200 response establishing the SSE stream, got: {text}"
    );

    send_signal(child.id(), "TERM");
    let status = wait_for_exit(&mut child, Duration::from_secs(5));
    assert_eq!(status.code(), Some(0));

    drop(stream);
}
