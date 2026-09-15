#![cfg(feature = "test-helpers")]

use axum::body::Body;
use axum::http::Request;
use kanban_server::app;
use kanban_server::test_helpers::make_state;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;
use tower::ServiceExt;
use tracing_subscriber::fmt::MakeWriter;

#[derive(Clone)]
struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
struct CaptureMakeWriter(Arc<Mutex<Vec<u8>>>);

impl<'a> MakeWriter<'a> for CaptureMakeWriter {
    type Writer = CaptureWriter;

    fn make_writer(&'a self) -> Self::Writer {
        CaptureWriter(self.0.clone())
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_trace_layer_emits_tower_http_request_events() {
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(CaptureMakeWriter(buffer.clone()))
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let router = app::router(state);

    let request = Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert!(response.status().is_success());

    let captured = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
    assert!(!captured.is_empty());
    assert!(captured.contains("tower_http::trace"));
}
