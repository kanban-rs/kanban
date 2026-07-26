use kanban_service::api::ChangeEventFrame;
use kanban_service::KanbanContext;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Shared state for every axum handler.
///
/// `ctx` uses a `tokio::sync::Mutex`, not an `RwLock`: `KanbanContext`'s
/// mutating operations are synchronous but its persistence (`save`/`reload`)
/// is `async`, so a write path can hold the guard across an `.await`. Mixing
/// that with a sync `RwLock` write guard is a `Send`/deadlock hazard;
/// `tokio::sync::Mutex` is async-aware and used for both read and write
/// handlers. Concurrent-read optimization is out of scope for this scaffold.
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
