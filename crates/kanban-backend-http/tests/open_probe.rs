use kanban_backend_http::HttpBackend;
use kanban_domain::KanbanError;
use kanban_server::test_helpers::TestServer;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn find_headers_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

async fn serve_health(body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{addr}");
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(conn) => conn,
                Err(_) => break,
            };
            let body = body.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                loop {
                    let n = match socket.read(&mut tmp).await {
                        Ok(n) => n,
                        Err(_) => return,
                    };
                    buf.extend_from_slice(&tmp[..n]);
                    if find_headers_end(&buf).is_some() || n == 0 {
                        break;
                    }
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    url
}

#[tokio::test(flavor = "multi_thread")]
async fn test_open_over_http_backend_against_live_server_succeeds() {
    let server = TestServer::start().await;
    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new(&server.base_url()).unwrap());

    let result = KanbanContext::open(Arc::clone(&backend), AppConfig::default()).await;

    let ctx = match result {
        Ok(ctx) => ctx,
        Err(e) => panic!("expected open() to succeed against a live server, got: {e}"),
    };

    drop(ctx);
    drop(backend);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_open_over_http_backend_against_dead_server_fails_with_transport_error() {
    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new("http://127.0.0.1:1").unwrap());

    let result = KanbanContext::open(Arc::clone(&backend), AppConfig::default()).await;

    match result {
        Err(KanbanError::Transport(msg)) => {
            assert!(
                msg.contains("/health"),
                "expected the probed URL in the error message, got: {msg:?}"
            );
        }
        Err(other) => panic!("expected KanbanError::Transport, got: {other:?}"),
        Ok(_) => panic!("expected open() to fail against an unreachable server"),
    }

    drop(backend);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_open_against_a_server_without_a_version_fails_with_the_upgrade_message() {
    let id = uuid::Uuid::new_v4();
    let base_url = serve_health(format!(r#"{{"status":"ok","instance_id":"{id}"}}"#)).await;
    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new(&base_url).unwrap());

    let result = KanbanContext::open(Arc::clone(&backend), AppConfig::default()).await;

    let err = match result {
        Ok(_) => panic!("expected open to fail against a server without a version"),
        Err(e) => e,
    };
    match err {
        KanbanError::UnsupportedServerVersion {
            server_version: None,
            ref client_version,
            ref url,
        } if client_version == kanban_core::KANBAN_VERSION && url == &base_url => {}
        other => panic!("expected UnsupportedServerVersion, got {other:?}"),
    }
    assert!(
        err.to_string()
            .contains("Upgrade the server before the client"),
        "msg: {err}"
    );

    drop(backend);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_open_against_an_older_minor_server_fails_naming_both_versions() {
    let id = uuid::Uuid::new_v4();
    let base_url = serve_health(format!(
        r#"{{"status":"ok","instance_id":"{id}","version":"0.0.1"}}"#
    ))
    .await;
    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new(&base_url).unwrap());

    let result = KanbanContext::open(Arc::clone(&backend), AppConfig::default()).await;

    let err = match result {
        Ok(_) => panic!("expected open to fail against an older minor server"),
        Err(e) => e,
    };
    match err {
        KanbanError::UnsupportedServerVersion {
            server_version: Some(ref v),
            ref client_version,
            ref url,
        } if v == "0.0.1" && client_version == kanban_core::KANBAN_VERSION && url == &base_url => {}
        other => panic!("expected UnsupportedServerVersion, got {other:?}"),
    }
    let msg = err.to_string();
    assert!(msg.contains("v0.0.1"), "msg: {msg}");
    assert!(
        msg.contains(&format!("v{}", kanban_core::KANBAN_VERSION)),
        "msg: {msg}"
    );

    drop(backend);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_open_against_a_newer_server_succeeds() {
    let id = uuid::Uuid::new_v4();
    let base_url = serve_health(format!(
        r#"{{"status":"ok","instance_id":"{id}","version":"999.0.0"}}"#
    ))
    .await;
    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new(&base_url).unwrap());

    let result = KanbanContext::open(Arc::clone(&backend), AppConfig::default()).await;

    match result {
        Ok(ctx) => drop(ctx),
        Err(e) => panic!("expected open to succeed against a newer server, got: {e}"),
    }

    drop(backend);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_open_against_a_non_kanban_health_body_fails_with_transport_error() {
    let base_url = serve_health("not json".to_string()).await;
    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new(&base_url).unwrap());

    let result = KanbanContext::open(Arc::clone(&backend), AppConfig::default()).await;

    match result {
        Err(KanbanError::Transport(msg)) => {
            assert!(msg.contains("/health"), "msg: {msg}");
        }
        Err(other) => panic!("expected KanbanError::Transport, got: {other:?}"),
        Ok(_) => panic!("expected open to fail against a non-kanban health body"),
    }

    drop(backend);
}
