use kanban_domain::InMemoryStore;
use kanban_server::app;
use kanban_server::state::AppState;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

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
