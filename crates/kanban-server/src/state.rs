use kanban_core::ClientId;
use kanban_service::api::{ChangeEventFrame, ChangeKind, EntityType};
use kanban_service::{KanbanContext, KanbanResult};
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

    fn emit(
        &self,
        entity_type: Option<EntityType>,
        entity_id: Option<Uuid>,
        kind: Option<ChangeKind>,
    ) {
        let _ = self.event_tx.send(ChangeEventFrame::for_entity(
            self.instance_id,
            Uuid::new_v4(),
            ClientId::nil(),
            entity_type,
            entity_id,
            kind,
        ));
    }

    /// Broadcast a change event naming the entity a mutation touched. Shared
    /// across every entity's write routes so each doesn't reimplement it;
    /// call after the context lock guard has been dropped. A missing
    /// subscriber (no SSE consumer connected yet) is not an error, hence the
    /// discarded result.
    pub fn broadcast_change(&self, entity_type: EntityType, entity_id: Uuid, kind: ChangeKind) {
        self.emit(Some(entity_type), Some(entity_id), Some(kind));
    }

    /// Broadcast a change event whose origin is outside this process (an
    /// external writer changed the file), so the specific entity touched is
    /// unknowable.
    pub fn broadcast_unscoped_change(&self) {
        self.emit(None, None, None);
    }

    /// Durably persist any pending changes, then broadcast that `entity_id`
    /// changed. Call this from *inside* the context lock (needs
    /// `&KanbanContext` — `save()` takes `&self`, so no reacquire is needed),
    /// immediately after a successful mutation and before the lock guard
    /// drops. A write whose `save()` fails must not report success to the
    /// client — callers propagate the error via `AppError::from(&e)` exactly
    /// like every other `KanbanResult` in this crate, so a 201/200 response
    /// is only ever returned once the data is actually on disk (or in
    /// SQLite's case, harmlessly redundant — `flush()` is a no-op cost there
    /// since each statement already committed).
    pub async fn persist_and_broadcast(
        &self,
        ctx: &KanbanContext,
        entity_type: EntityType,
        entity_id: Uuid,
        kind: ChangeKind,
    ) -> KanbanResult<()> {
        ctx.save().await?;
        self.broadcast_change(entity_type, entity_id, kind);
        Ok(())
    }
}
