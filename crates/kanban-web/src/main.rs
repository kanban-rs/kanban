use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use kanban_web::context::{router, SharedCtx};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let path = std::env::var("KANBAN_WEB_FILE").unwrap_or_else(|_| "kanban.json".into());
    let backend: Arc<dyn KanbanBackend> = Arc::new(JsonDataStore::new(Arc::new(
        JsonFileStore::new(std::path::Path::new(&path)),
    )));
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let shared: SharedCtx = Arc::new(Mutex::new(ctx));

    topcoat::start(router(shared)).await.unwrap();
}
