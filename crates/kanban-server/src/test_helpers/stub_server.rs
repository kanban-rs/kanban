use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::response::IntoResponse;
use axum::Router;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

#[derive(Debug, Clone)]
pub struct StubReply {
    pub status: u16,
    pub body: String,
}

impl StubReply {
    pub fn json(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
        }
    }

    pub fn empty(status: u16) -> Self {
        Self {
            status,
            body: String::new(),
        }
    }
}

/// Serves `respond(method, path)` for every request, so a test can reproduce
/// a server version whose routes or response shapes differ from this one.
pub struct StubServer {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    handle: JoinHandle<()>,
    requests: Arc<Mutex<Vec<(String, String)>>>,
}

impl StubServer {
    /// An empty body goes out with no content type, like axum's own 404/405/204.
    pub async fn start<F>(respond: F) -> Self
    where
        F: Fn(&str, &str) -> StubReply + Send + Sync + 'static,
    {
        let respond = Arc::new(respond);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&requests);
        let router = Router::new().fallback(move |req: Request<Body>| {
            let respond = Arc::clone(&respond);
            let log = Arc::clone(&log);
            async move {
                let method = req.method().as_str().to_string();
                let path = req.uri().path().to_string();
                log.lock().unwrap().push((method.clone(), path.clone()));
                let reply = respond(&method, &path);
                let status = StatusCode::from_u16(reply.status).unwrap();
                if reply.body.is_empty() {
                    status.into_response()
                } else {
                    (
                        status,
                        [(header::CONTENT_TYPE, "application/json")],
                        reply.body,
                    )
                        .into_response()
                }
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
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
            requests,
        }
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Every request received, as `(method, path)` in arrival order.
    pub fn requests(&self) -> Vec<(String, String)> {
        self.requests.lock().unwrap().clone()
    }

    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        let _ = (&mut self.handle).await;
    }
}

impl Drop for StubServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
