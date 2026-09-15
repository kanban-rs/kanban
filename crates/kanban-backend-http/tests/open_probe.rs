use kanban_backend_http::HttpBackend;
use kanban_domain::KanbanError;
use kanban_server::test_helpers::TestServer;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;

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
