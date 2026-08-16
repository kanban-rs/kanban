use kanban_backend_http::HttpBackend;
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use kanban_web::context::{router, SharedCtx};
use std::sync::Arc;
use tokio::sync::Mutex;

/// `KANBAN_SERVER_URL`, when set, points `kanban-web` at a remote
/// `kanban-server` instance via `HttpBackend` instead of a local JSON file.
/// `HttpBackend`'s write path is still unimplemented, so this is read-only
/// today -- fine for the current single home page.
fn build_backend() -> Arc<dyn KanbanBackend> {
    if let Ok(url) = std::env::var("KANBAN_SERVER_URL") {
        return Arc::new(HttpBackend::new(&url).expect("invalid KANBAN_SERVER_URL"));
    }
    let path = std::env::var("KANBAN_FILE").unwrap_or_else(|_| "kanban.json".into());
    Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(
        std::path::Path::new(&path),
    ))))
}

#[tokio::main]
async fn main() {
    let backend = build_backend();
    // open_deferred, not open: HttpBackend's CommandStore is still
    // unsupported() (batch_count included), so open()'s eager
    // batch_count() check would fail immediately against a remote backend.
    let ctx = KanbanContext::open_deferred(backend, AppConfig::default());
    let shared: SharedCtx = Arc::new(Mutex::new(ctx));

    topcoat::start(router(shared)).await.unwrap();
}
