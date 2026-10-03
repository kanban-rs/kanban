//! Shared test utilities for `kanban-server` integration tests.
//!
//! This module is shared across separate integration-test binaries; each binary only uses a
//! subset, so unused items here would otherwise warn as dead code per-binary.
#![allow(dead_code)]

use crate::app;
use crate::state::AppState;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use kanban_backend_memory::InMemoryStore;
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use serde_json::Value;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tower::ServiceExt;

/// A log of every request a [`TestServer::start_recording`] router received,
/// as `(method, path)` in arrival order.
pub type RequestLog = Arc<Mutex<Vec<(Method, String)>>>;

/// `(method, path)` that a [`TestServer::start_with_fault`] router answers with a
/// bodiless `503 Service Unavailable` instead of routing, while armed.
pub type FaultSwitch = Arc<Mutex<Option<(&'static str, String)>>>;

/// Build an `AppState` over a fresh `JsonDataStore` at `path`, for the
/// `tower::ServiceExt::oneshot` in-process route tests (as opposed to
/// `TestServer`'s real-socket harness below).
pub fn make_state(path: &std::path::Path) -> AppState {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    let ctx = KanbanContext::open_deferred(backend, AppConfig::default());
    AppState::new(ctx)
}

/// Like [`make_state`], but over a real `SqliteBackend`.
pub async fn make_sqlite_state(path: &std::path::Path) -> AppState {
    let backend: Arc<dyn KanbanBackend> = Arc::new(
        kanban_persistence_sqlite::SqliteBackend::open(path.to_str().unwrap())
            .await
            .unwrap(),
    );
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
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

/// Like [`send`], but with extra request headers.
pub async fn send_with_headers(
    state: &AppState,
    method: &str,
    uri: &str,
    body: Option<&Value>,
    headers: &[(&str, &str)],
) -> Response {
    let mut builder = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(v) => {
            builder = builder.header("content-type", "application/json");
            Body::from(serde_json::to_string(v).unwrap())
        }
        None => Body::empty(),
    };
    for (k, v) in headers {
        builder = builder.header(*k, *v);
    }
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
        Self::start_full(|_| {}, crate::layers::LayerConfig::default(), None, None).await
    }

    /// Like [`Self::start`], but runs `seed` against the fresh `KanbanContext`
    /// before the router starts serving, so a test can put the context in a
    /// state (e.g. an archived card) that no HTTP write route can reach.
    pub async fn start_with(seed: impl FnOnce(&mut KanbanContext)) -> Self {
        Self::start_full(seed, crate::layers::LayerConfig::default(), None, None).await
    }

    /// Like [`Self::start`], but with an explicit [`crate::layers::LayerConfig`].
    pub async fn start_with_layers(config: crate::layers::LayerConfig) -> Self {
        Self::start_full(|_| {}, config, None, None).await
    }

    /// Like [`Self::start_with`], but also returns a log of every request the
    /// router received, as `(method, path)` in arrival order. Recorded by an
    /// `axum::middleware::from_fn` layered on the router that
    /// [`crate::app::router_with`] returns.
    pub async fn start_recording(seed: impl FnOnce(&mut KanbanContext)) -> (Self, RequestLog) {
        let log: RequestLog = Arc::new(Mutex::new(Vec::new()));
        let server = Self::start_full(
            seed,
            crate::layers::LayerConfig::default(),
            Some(log.clone()),
            None,
        )
        .await;
        (server, log)
    }

    /// Like [`Self::start_with`], but also returns a [`FaultSwitch`]; every
    /// request not matching the armed `(method, path)` is served normally.
    pub async fn start_with_fault(seed: impl FnOnce(&mut KanbanContext)) -> (Self, FaultSwitch) {
        let fault: FaultSwitch = Arc::new(Mutex::new(None));
        let server = Self::start_full(
            seed,
            crate::layers::LayerConfig::default(),
            None,
            Some(fault.clone()),
        )
        .await;
        (server, fault)
    }

    async fn start_full(
        seed: impl FnOnce(&mut KanbanContext),
        config: crate::layers::LayerConfig,
        recorder: Option<RequestLog>,
        fault: Option<FaultSwitch>,
    ) -> Self {
        let backend: Arc<dyn KanbanBackend> = Arc::new(InMemoryStore::new());
        Self::start_full_on(backend, seed, config, recorder, fault).await
    }

    /// Serve a `KanbanContext` backed by a `JsonDataStore` at `path`.
    pub async fn start_on_json(path: &std::path::Path) -> Self {
        let backend: Arc<dyn KanbanBackend> =
            Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
        Self::start_full_on(
            backend,
            |_| {},
            crate::layers::LayerConfig::default(),
            None,
            None,
        )
        .await
    }

    /// Serve a `KanbanContext` backed by a `SqliteBackend` at `path`.
    pub async fn start_on_sqlite(path: &std::path::Path) -> Self {
        let backend: Arc<dyn KanbanBackend> = Arc::new(
            kanban_persistence_sqlite::SqliteBackend::open(path.to_str().unwrap())
                .await
                .unwrap(),
        );
        Self::start_full_on(
            backend,
            |_| {},
            crate::layers::LayerConfig::default(),
            None,
            None,
        )
        .await
    }

    async fn start_full_on(
        backend: Arc<dyn KanbanBackend>,
        seed: impl FnOnce(&mut KanbanContext),
        config: crate::layers::LayerConfig,
        recorder: Option<RequestLog>,
        fault: Option<FaultSwitch>,
    ) -> Self {
        let mut ctx = KanbanContext::open(backend, AppConfig::default())
            .await
            .unwrap();
        seed(&mut ctx);
        let state = AppState::new(ctx);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let router = app::router_with(state, config);
        let router = match recorder {
            Some(log) => router.layer(middleware::from_fn(
                move |req: Request<Body>, next: Next| {
                    let log = log.clone();
                    async move {
                        log.lock()
                            .unwrap()
                            .push((req.method().clone(), req.uri().path().to_string()));
                        next.run(req).await
                    }
                },
            )),
            None => router,
        };
        let router = match fault {
            Some(switch) => router.layer(middleware::from_fn(
                move |req: Request<Body>, next: Next| {
                    let switch = switch.clone();
                    async move {
                        let armed = switch.lock().unwrap().clone();
                        if let Some((method, path)) = armed {
                            if req.method().as_str() == method && req.uri().path() == path {
                                return StatusCode::SERVICE_UNAVAILABLE.into_response();
                            }
                        }
                        next.run(req).await
                    }
                },
            )),
            None => router,
        };
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
