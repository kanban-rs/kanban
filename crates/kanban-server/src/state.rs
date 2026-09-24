use kanban_core::ClientId;
use kanban_domain::{Invalidation, MutationOperations};
use kanban_service::api::{ChangeEventFrame, ChangeKind, EntityType, InvalidationDto};
use kanban_service::{KanbanContext, KanbanResult};
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};
use uuid::Uuid;

/// Every handler reaches the context through this guard; nothing is
/// retained between requests. `Deref`/`DerefMut` to the context keep every
/// existing `state.ctx.lock()` call site reading as it did before.
pub struct Session {
    pub ctx: KanbanContext,
}

impl std::ops::Deref for Session {
    type Target = KanbanContext;

    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}

impl std::ops::DerefMut for Session {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ctx
    }
}

/// Run a mutation against the session's context. Returns the operation's
/// value together with the `Invalidation` it produced, for the caller to
/// describe the change with.
pub(crate) fn mutate<T>(
    session: &mut Session,
    op: impl FnOnce(&mut dyn MutationOperations) -> KanbanResult<(T, Invalidation)>,
) -> KanbanResult<(T, Invalidation)> {
    op(&mut session.ctx)
}

/// Like [`mutate`], for operations that return only an `Invalidation`.
pub(crate) fn mutate_unit(
    session: &mut Session,
    op: impl FnOnce(&mut dyn MutationOperations) -> KanbanResult<Invalidation>,
) -> KanbanResult<Invalidation> {
    op(&mut session.ctx)
}

/// Shared state for every axum handler.
///
/// `tokio::sync::Mutex`, not `RwLock`: `KanbanContext`'s write path is async
/// (`save`/`reload`), so holding a sync `RwLock` write guard across an
/// `.await` would be a `Send`/deadlock hazard.
#[derive(Clone)]
pub struct AppState {
    pub ctx: Arc<Mutex<Session>>,
    pub instance_id: Uuid,
    pub event_tx: tokio::sync::broadcast::Sender<ChangeEventFrame>,
}

impl AppState {
    pub fn new(ctx: KanbanContext) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(256);
        Self {
            ctx: Arc::new(Mutex::new(Session { ctx })),
            instance_id: Uuid::new_v4(),
            event_tx,
        }
    }

    /// The single seam every handler acquires the session lock through.
    pub async fn lock_session(&self) -> MutexGuard<'_, Session> {
        self.ctx.lock().await
    }

    /// Like [`lock_session`][Self::lock_session], but stamps the context with
    /// `client` for the lifetime of the returned guard. The identity resets
    /// to nil when the guard drops, so it never survives past the request
    /// that set it.
    pub async fn lock_for_write(&self, client: ClientId) -> WriteSession<'_> {
        let mut guard = self.lock_session().await;
        guard.ctx.set_issued_by(client);
        WriteSession { guard }
    }

    fn emit(
        &self,
        issued_by: ClientId,
        entity_type: Option<EntityType>,
        entity_id: Option<Uuid>,
        kind: Option<ChangeKind>,
        invalidation: InvalidationDto,
    ) {
        let _ = self.event_tx.send(
            ChangeEventFrame::for_entity(
                self.instance_id,
                Uuid::new_v4(),
                issued_by,
                entity_type,
                entity_id,
                kind,
            )
            .with_invalidation(invalidation),
        );
    }

    /// Broadcast a change event naming the entity a mutation touched. Shared
    /// across every entity's write routes so each doesn't reimplement it;
    /// call after the context lock guard has been dropped. A missing
    /// subscriber (no SSE consumer connected yet) is not an error, hence the
    /// discarded result.
    pub fn broadcast_change(
        &self,
        issued_by: ClientId,
        entity_type: EntityType,
        entity_id: Uuid,
        kind: ChangeKind,
        invalidation: &Invalidation,
    ) {
        self.emit(
            issued_by,
            Some(entity_type),
            Some(entity_id),
            Some(kind),
            InvalidationDto::from(invalidation),
        );
    }

    /// Broadcast a change event whose origin is outside this process (an
    /// external writer changed the file), so the specific entity touched is
    /// unknowable and the invalidation is the unconditional `All`.
    pub fn broadcast_unscoped_change(&self) {
        self.emit(ClientId::nil(), None, None, None, InvalidationDto::All);
    }

    /// Durably persist any pending changes, then broadcast that `entity_id`
    /// changed, carrying the `invalidation` the caller's mutation returned.
    /// Call this from *inside* the context lock (needs `&KanbanContext` —
    /// `save()` takes `&self`, so no reacquire is needed), immediately after
    /// a successful mutation and before the lock guard drops. A write whose
    /// `save()` fails must not report success to the client — callers
    /// propagate the error via `AppError::from(&e)` exactly like every other
    /// `KanbanResult` in this crate, so a 201/200 response is only ever
    /// returned once the data is actually on disk (or in SQLite's case,
    /// harmlessly redundant — `flush()` is a no-op cost there since each
    /// statement already committed).
    pub async fn persist_and_broadcast(
        &self,
        ctx: &KanbanContext,
        entity_type: EntityType,
        entity_id: Uuid,
        kind: ChangeKind,
        invalidation: &Invalidation,
    ) -> KanbanResult<()> {
        ctx.save().await?;
        self.broadcast_change(ctx.issued_by(), entity_type, entity_id, kind, invalidation);
        Ok(())
    }
}

/// A [`Session`] lock scoped to one client's write request. Deref/DerefMut
/// forward to `Session` exactly like `lock_session`'s guard; dropping this
/// guard resets `KanbanContext::issued_by` to nil so the identity cannot
/// leak into a later request that acquires the lock without going through
/// `lock_for_write`.
pub struct WriteSession<'a> {
    guard: MutexGuard<'a, Session>,
}

impl std::ops::Deref for WriteSession<'_> {
    type Target = Session;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl std::ops::DerefMut for WriteSession<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

impl Drop for WriteSession<'_> {
    fn drop(&mut self) {
        self.guard.ctx.set_issued_by(ClientId::nil());
    }
}

#[cfg(all(test, feature = "test-helpers"))]
mod tests {
    use super::*;
    use kanban_backend_memory::InMemoryStore;
    use kanban_domain::BoardUpdate;
    use kanban_persistence_json::{JsonDataStore, JsonFileStore};
    use kanban_service::{AppConfig, KanbanBackend, KanbanOperations};
    use std::cell::Cell;

    async fn seeded_ctx() -> (Session, Uuid) {
        let backend: Arc<dyn KanbanBackend> = Arc::new(InMemoryStore::new());
        let mut ctx = KanbanContext::open(backend, AppConfig::default())
            .await
            .unwrap();
        let board = ctx.create_board("Board1".into(), None).unwrap();
        (Session { ctx }, board.id)
    }

    fn json_state(dir: &std::path::Path) -> AppState {
        let backend: Arc<dyn KanbanBackend> = Arc::new(JsonDataStore::new(Arc::new(
            JsonFileStore::new(dir.join("s.json")),
        )));
        let ctx = KanbanContext::open_deferred(backend, AppConfig::default());
        AppState::new(ctx)
    }

    #[tokio::test]
    async fn test_mutate_seam_accepts_a_mutation_operations_closure() {
        let (mut session, board_id) = seeded_ctx().await;

        let (board, _inv) = mutate(
            &mut session,
            |c: &mut dyn kanban_domain::MutationOperations| {
                c.update_board_impl(
                    board_id,
                    BoardUpdate {
                        name: Some("Renamed".into()),
                        ..Default::default()
                    },
                )
            },
        )
        .unwrap();
        assert_eq!(board.name, "Renamed");

        let _inv = mutate_unit(
            &mut session,
            |c: &mut dyn kanban_domain::MutationOperations| c.delete_board_impl(board_id),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn test_mutate_returns_the_invalidation_the_operation_produced() {
        let dir = tempfile::tempdir().unwrap();
        let state = json_state(dir.path());
        let mut guard = state.lock_session().await;

        let board_id = guard.ctx.create_board("A".into(), None).unwrap().id;

        let (board, invalidation) = mutate(&mut guard, |c| {
            c.update_board_impl(
                board_id,
                BoardUpdate {
                    name: Some("Renamed".into()),
                    ..Default::default()
                },
            )
        })
        .unwrap();

        assert_eq!(board.name, "Renamed");
        match invalidation {
            Invalidation::Entities(ids) => assert!(ids.boards.contains(&board_id)),
            Invalidation::All => panic!("expected Invalidation::Entities naming the board"),
        }
    }

    #[tokio::test]
    async fn test_mutate_unit_returns_the_invalidation_the_operation_produced() {
        let dir = tempfile::tempdir().unwrap();
        let state = json_state(dir.path());
        let mut guard = state.lock_session().await;

        let board_id = guard.ctx.create_board("A".into(), None).unwrap().id;

        let invalidation = mutate_unit(&mut guard, |c| c.delete_board_impl(board_id)).unwrap();

        assert!(
            matches!(invalidation, Invalidation::All)
                || matches!(&invalidation, Invalidation::Entities(ids) if ids.boards.contains(&board_id))
        );
    }

    #[tokio::test]
    async fn test_mutate_returns_the_operations_value() {
        let (mut ctx, board_id) = seeded_ctx().await;

        let (updated, _invalidation) = mutate(&mut ctx, |c| {
            c.update_board_impl(
                board_id,
                kanban_domain::BoardUpdate {
                    name: Some("Renamed".into()),
                    ..Default::default()
                },
            )
        })
        .unwrap();

        assert_eq!(updated.name, "Renamed");
    }

    #[tokio::test]
    async fn test_mutate_propagates_the_error_and_leaves_the_store_untouched() {
        let (mut session, _board_id) = seeded_ctx().await;
        let absent_id = Uuid::new_v4();

        let result = mutate(&mut session, |c| {
            c.update_board_impl(absent_id, kanban_domain::BoardUpdate::default())
        });

        assert!(result.is_err());
        assert_eq!(session.ctx.list_boards().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_the_broadcast_frame_carries_the_invalidation_the_seam_returned() {
        let dir = tempfile::tempdir().unwrap();
        let state = json_state(dir.path());
        let mut rx = state.event_tx.subscribe();

        let board_id = {
            let mut guard = state.lock_for_write(ClientId::nil()).await;
            guard.ctx.create_board("A".into(), None).unwrap().id
        };

        let mut guard = state.lock_for_write(ClientId::nil()).await;
        let invalidation = mutate_unit(&mut guard, |c| c.delete_board_impl(board_id)).unwrap();
        state
            .persist_and_broadcast(
                &guard,
                EntityType::Board,
                board_id,
                ChangeKind::Deleted,
                &invalidation,
            )
            .await
            .unwrap();
        drop(guard);

        let frame = rx.try_recv().unwrap();
        assert_eq!(
            frame.invalidation,
            Some(kanban_service::api::InvalidationDto::from(&invalidation))
        );
    }

    #[tokio::test]
    async fn test_mutate_runs_the_operation_exactly_once() {
        let (mut ctx, board_id) = seeded_ctx().await;
        let calls = Cell::new(0u32);

        let (_board, _invalidation) = mutate(&mut ctx, |c| {
            calls.set(calls.get() + 1);
            c.update_board_impl(board_id, kanban_domain::BoardUpdate::default())
        })
        .unwrap();

        assert_eq!(calls.get(), 1);
    }
}
