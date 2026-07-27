use kanban_core::ClientId;
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

    /// Broadcast a change event after a successful mutation. Shared across
    /// every entity's write routes so each doesn't reimplement it; call after
    /// the context lock guard has been dropped. A missing subscriber (no SSE
    /// consumer connected yet) is not an error, hence the discarded result.
    pub fn broadcast_change(&self) {
        let _ = self.event_tx.send(ChangeEventFrame::now(
            self.instance_id,
            Uuid::new_v4(),
            ClientId::nil(),
        ));
    }
}
