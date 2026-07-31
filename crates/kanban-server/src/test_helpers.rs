//! Shared test utilities for `kanban-server` integration tests.
//!
//! This module is shared across separate integration-test binaries; each binary only uses a
//! subset, so unused items here would otherwise warn as dead code per-binary.
#![allow(dead_code)]

use crate::app;
use crate::state::AppState;
use axum::body::Body;
use axum::http::Request;
use axum::response::Response;
use kanban_backend_memory::InMemoryStore;
use kanban_persistence_json::JsonFileStore;
use kanban_service::json_backend::JsonDataStore;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tower::ServiceExt;

/// Build an `AppState` over a fresh `JsonDataStore` at `path`, for the
/// `tower::ServiceExt::oneshot` in-process route tests (as opposed to
/// `TestServer`'s real-socket harness below).
pub fn make_state(path: &std::path::Path) -> AppState {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    let ctx = KanbanContext::open_deferred(backend, AppConfig::default());
    AppState::new(ctx)
}

/// Drive one request through `app::router` via `oneshot`, JSON-encoding `body` when present.
pub async fn send(state: &AppState, method: &str, uri: &str, body: Option<&Value>) -> Response {
    let mut builder = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(v) => {
            builder = builder.header("content-type", "application/json");
            Body::from(serde_json::to_string(v).unwrap())
        }
        None => Body::empty(),
    };
    app::router(state.clone())
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap()
}

pub async fn json_of(response: Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

pub struct TestServer {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    handle: JoinHandle<()>,
}

impl TestServer {
    /// Bind 127.0.0.1:0 (OS-assigned port), build AppState over a zero-I/O
    /// InMemoryStore-backed KanbanContext, spawn axum::serve with graceful
    /// shutdown, and return once the listener is bound (so base_url()/addr()
    /// are valid the moment start() returns -- no race with a test hitting
    /// the port before bind completes).
    pub async fn start() -> Self {
        let backend: Arc<dyn KanbanBackend> = Arc::new(InMemoryStore::new());
        let ctx = KanbanContext::open(backend, AppConfig::default())
            .await
            .unwrap();
        let state = AppState::new(ctx);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let router = app::router(state);
        let handle = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });

        Self {
            addr,
            shutdown_tx: Some(shutdown_tx),
            handle,
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn client(&self) -> reqwest::Client {
        reqwest::Client::new()
    }

    /// Send the graceful-shutdown signal and await the server task.
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        let _ = (&mut self.handle).await;
    }
}

impl Drop for TestServer {
    /// Safety net if a test panics before calling `shutdown()` -- aborts the
    /// spawned server task rather than leaking it for the rest of the test
    /// process. A no-op if `shutdown()` already ran.
    fn drop(&mut self) {
        self.handle.abort();
    }
}
