use kanban_service::api::ChangeEventFrame;
use kanban_service::KanbanContext;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Shared state for every axum handler.
///
/// `tokio::sync::Mutex`, not `RwLock`: `KanbanContext`'s write path is async
/// (`save`/`reload`), so holding a sync `RwLock` write guard across an
/// `.await` would be a `Send`/deadlock hazard.
#[derive(Clone)]
pub struct AppState {
    pub ctx: Arc<Mutex<KanbanContext>>,
    pub instance_id: Uuid,
    pub event_tx: tokio::sync::broadcast::Sender<ChangeEventFrame>,
}

impl AppState {
    pub fn new(ctx: KanbanContext) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(256);
        Self {
            ctx: Arc::new(Mutex::new(ctx)),
            instance_id: Uuid::new_v4(),
            event_tx,
        }
    }
}
