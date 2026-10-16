use kanban_backend::CompatibilityNotice;
use kanban_backend_http::HttpBackend;
use kanban_domain::KanbanError;
use kanban_server::test_helpers::{StubReply, StubServer, TestServer};
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;

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
async fn test_open_against_a_server_without_a_version_succeeds_with_an_unknown_version_notice() {
    let stub = StubServer::pre_handshake(|_, _| StubReply::empty(404)).await;
    let base_url = stub.base_url();
    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new(&base_url).unwrap());

    let result = KanbanContext::open(Arc::clone(&backend), AppConfig::default()).await;

    let ctx = match result {
        Ok(ctx) => ctx,
        Err(e) => panic!("expected open to succeed against a server without a version, got: {e}"),
    };

    let http_backend = backend
        .as_any()
        .and_then(|a| a.downcast_ref::<HttpBackend>())
        .expect("backend must downcast to HttpBackend");
    assert_eq!(
        http_backend.compatibility_notice(),
        Some(CompatibilityNotice::UnknownServerVersion {
            url: base_url.clone(),
            reported: None,
            client_version: kanban_core::KANBAN_VERSION.to_string(),
        })
    );

    let write_result = backend
        .remote_graph_writes()
        .expect("http backend")
        .unblock(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    match write_result {
        Err(e) => assert!(
            e.is_unsupported(),
            "expected the write to degrade with UnsupportedByServer, got: {e}"
        ),
        Ok(_) => panic!("expected the write to fail against a server with no route for it"),
    }

    drop(ctx);
    drop(backend);
    stub.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_open_against_an_older_minor_server_succeeds_with_an_older_server_notice() {
    let id = uuid::Uuid::new_v4();
    let stub = StubServer::start(move |method, path| match (method, path) {
        ("GET", "/health") => StubReply::json(
            200,
            format!(r#"{{"status":"ok","instance_id":"{id}","version":"0.0.1"}}"#),
        ),
        _ => StubReply::empty(404),
    })
    .await;
    let base_url = stub.base_url();
    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new(&base_url).unwrap());

    let result = KanbanContext::open(Arc::clone(&backend), AppConfig::default()).await;

    let ctx = match result {
        Ok(ctx) => ctx,
        Err(e) => panic!("expected open to succeed against an older minor server, got: {e}"),
    };

    let http_backend = backend
        .as_any()
        .and_then(|a| a.downcast_ref::<HttpBackend>())
        .expect("backend must downcast to HttpBackend");
    assert_eq!(
        http_backend.compatibility_notice(),
        Some(CompatibilityNotice::OlderServer {
            url: base_url.clone(),
            server_version: "0.0.1".to_string(),
            client_version: kanban_core::KANBAN_VERSION.to_string(),
        })
    );

    drop(ctx);
    drop(backend);
    stub.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_open_against_a_newer_server_succeeds_without_a_notice() {
    let id = uuid::Uuid::new_v4();
    let stub = StubServer::start(move |method, path| match (method, path) {
        ("GET", "/health") => StubReply::json(
            200,
            format!(r#"{{"status":"ok","instance_id":"{id}","version":"999.0.0"}}"#),
        ),
        _ => StubReply::empty(404),
    })
    .await;
    let base_url = stub.base_url();
    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new(&base_url).unwrap());

    let result = KanbanContext::open(Arc::clone(&backend), AppConfig::default()).await;

    let ctx = match result {
        Ok(ctx) => ctx,
        Err(e) => panic!("expected open to succeed against a newer server, got: {e}"),
    };

    let http_backend = backend
        .as_any()
        .and_then(|a| a.downcast_ref::<HttpBackend>())
        .expect("backend must downcast to HttpBackend");
    assert_eq!(http_backend.compatibility_notice(), None);

    drop(ctx);
    drop(backend);
    stub.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_open_over_http_backend_against_live_server_succeeds_without_a_notice() {
    let server = TestServer::start().await;
    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new(&server.base_url()).unwrap());

    let result = KanbanContext::open(Arc::clone(&backend), AppConfig::default()).await;

    let ctx = match result {
        Ok(ctx) => ctx,
        Err(e) => panic!("expected open() to succeed against a live server, got: {e}"),
    };

    let http_backend = backend
        .as_any()
        .and_then(|a| a.downcast_ref::<HttpBackend>())
        .expect("backend must downcast to HttpBackend");
    assert_eq!(http_backend.compatibility_notice(), None);

    drop(ctx);
    drop(backend);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_open_against_a_non_kanban_health_body_fails_with_transport_error() {
    let stub = StubServer::start(|method, path| match (method, path) {
        ("GET", "/health") => StubReply::json(200, "not json"),
        _ => StubReply::empty(404),
    })
    .await;
    let base_url = stub.base_url();
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
    stub.shutdown().await;
}
