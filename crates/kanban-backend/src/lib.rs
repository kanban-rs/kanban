pub mod factory;
pub mod local_persistence;
pub mod remote_writes;
pub use factory::{KanbanBackendFactory, KanbanBackendRegistry};
pub use local_persistence::LocalPersistence;
pub use remote_writes::RemoteWrites;

use async_trait::async_trait;
use kanban_domain::command_store::CommandStore;
use kanban_domain::data_store::DataStore;
use kanban_domain::{KanbanError, KanbanResult};
use uuid::Uuid;

/// Combines the entity-level CRUD interface (`DataStore`) with the command
/// log (`CommandStore`) and lifecycle methods needed for pluggable backends.
///
/// # Lifecycle methods
///
/// - `flush()`: persist in-memory state to durable storage. For SQLite this
///   runs an explicit WAL checkpoint (TRUNCATE); for JSON this serialises the
///   cache to disk.
/// - `reload()`: discard cached state so the next read re-fetches from
///   durable storage. For SQLite this is a no-op (reads are always live).
/// - `needs_flush()`: returns `true` when there are uncommitted writes that
///   a subsequent `flush()` would persist.
/// - `needs_save_worker()`: returns `true` for backends (JSON) that require
///   a background worker to call `flush()` asynchronously after mutations.
/// - `instance_id()`: stable ID used to distinguish file writes by this
///   instance from external modifications.
#[async_trait]
pub trait KanbanBackend: DataStore + CommandStore + Send + Sync {
    /// Upcast to `&dyn DataStore`.
    fn as_data_store(&self) -> &dyn DataStore;

    /// Persist any cached writes to durable storage.
    async fn flush(&self) -> KanbanResult<()> {
        Ok(())
    }

    /// Discard cached state so the next read re-fetches from storage.
    async fn reload(&self) -> KanbanResult<()> {
        Ok(())
    }

    /// Returns `true` when there are writes that have not been flushed yet.
    fn needs_flush(&self) -> bool {
        false
    }

    /// Returns `true` for backends (JSON) that require a background flush
    /// worker. Always `false` for write-through backends (SQLite, in-memory).
    fn needs_save_worker(&self) -> bool {
        false
    }

    /// Stable instance UUID used for own-write detection in file watchers.
    fn instance_id(&self) -> Uuid {
        Uuid::nil()
    }

    /// Some(...) when this backend can report format version, writer kanban
    /// version, writer commit, and last save time (JSON, SQLite). `None`
    /// (the default) for backends with no durable local store (InMemory,
    /// Http) or that don't track it (MockBackend).
    fn local_persistence(&self) -> Option<&dyn crate::LocalPersistence> {
        None
    }

    /// Returns a health checker for this backend, if supported.
    /// The default returns `None`; backends that can self-diagnose
    /// (file readable, connection pool alive) override this.
    fn health_checker(&self) -> Option<Box<dyn kanban_core::HealthChecker>> {
        None
    }

    /// Some(...) when this backend wants create/update/delete for board/column/
    /// card to bypass local command execution entirely (an HTTP backend, where
    /// the remote server is authoritative). `None` (the default) for every local
    /// backend — zero behavior change for JSON/SQLite/InMemory.
    fn remote_writes(&self) -> Option<&dyn crate::RemoteWrites> {
        None
    }

    /// Run `f` as an atomic batch: every mutation commits or rolls back
    /// together.
    ///
    /// `f` runs at most once — never twice, so it may consume what it
    /// captures, and possibly not at all: a backend that cannot offer a
    /// transaction (`HttpBackend`, where the remote server owns the state)
    /// declines with an unsupported error *before* invoking it, rather than
    /// running the mutations unprotected. Do not rely on a side effect inside
    /// `f` having happened unless this returned `Ok`.
    ///
    /// There is deliberately no default implementation. A generic one can only
    /// roll back by snapshotting the whole store and restoring it, which is
    /// cheap in memory and ruinous on disk; making the method required forces
    /// each backend to answer with its own native mechanism, and turns a new
    /// backend that forgot to into a compile error rather than a silent
    /// whole-store copy.
    ///
    /// # Single-writer requirement
    ///
    /// Implementations that roll back by restoring a whole-store snapshot
    /// cannot tell the batch's own writes apart from anyone else's, so a
    /// mutation committed by another task between the capture and the failure
    /// is reverted along with the batch. Every consumer today serialises its
    /// mutations (`kanban-server` and `kanban-mcp` behind
    /// `Arc<Mutex<KanbanContext>>`, the TUI on its main loop), which is what
    /// makes this sound. Any new concurrent access to a shared backend must
    /// serialise too, and must revisit `SqliteStore::db_conn`, whose ambient
    /// transaction fails the same way but worse.
    fn with_transaction(&self, f: TransactionFn<'_>) -> KanbanResult<()>;
}

/// Closure passed to [`KanbanBackend::with_transaction`], boxed so the method
/// stays callable on `dyn KanbanBackend`. `FnOnce` because the batch never runs
/// twice and closures need to move owned values in.
pub type TransactionFn<'f> = Box<dyn FnOnce() -> KanbanResult<()> + 'f>;

/// Combines a batch failure with a failure to roll it back. Shared by the
/// snapshot-restoring backends so the wording of the one message a user sees
/// when the store is left in an unknown state cannot drift between them.
pub fn rollback_failed(batch_err: KanbanError, rollback_err: KanbanError) -> KanbanError {
    KanbanError::Internal(format!(
        "Batch failed ({batch_err}) and rollback also failed ({rollback_err}). State may be inconsistent."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::command_batch::CommandBatch;
    use kanban_domain::*;

    #[test]
    fn test_rollback_failed_names_both_the_batch_and_the_rollback_error() {
        let err = rollback_failed(
            KanbanError::Internal("batch boom".into()),
            KanbanError::Internal("restore boom".into()),
        );

        let msg = err.to_string();
        assert!(
            msg.contains("batch boom"),
            "batch error lost (got: {msg:?})"
        );
        assert!(
            msg.contains("restore boom"),
            "rollback error lost (got: {msg:?})"
        );
        assert!(
            msg.contains("State may be inconsistent"),
            "a failed rollback must say the store is left in an unknown state \
             (got: {msg:?})"
        );
    }

    struct StubBackend;

    impl DataStore for StubBackend {
        fn get_board(&self, _id: Uuid) -> KanbanResult<Option<Board>> {
            unimplemented!()
        }
        fn list_boards(&self) -> KanbanResult<Vec<Board>> {
            unimplemented!()
        }
        fn upsert_board(&self, _board: Board) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_board(&self, _id: Uuid) -> KanbanResult<()> {
            unimplemented!()
        }
        fn get_column(&self, _id: Uuid) -> KanbanResult<Option<Column>> {
            unimplemented!()
        }
        fn list_columns_by_board(&self, _board_id: Uuid) -> KanbanResult<Vec<Column>> {
            unimplemented!()
        }
        fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
            unimplemented!()
        }
        fn upsert_column(&self, _column: Column) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_column(&self, _id: Uuid) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_columns_by_board(&self, _board_id: Uuid) -> KanbanResult<()> {
            unimplemented!()
        }
        fn get_card(&self, _id: Uuid) -> KanbanResult<Option<Card>> {
            unimplemented!()
        }
        fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
            unimplemented!()
        }
        fn list_cards_by_column(&self, _column_id: Uuid) -> KanbanResult<Vec<Card>> {
            unimplemented!()
        }
        fn list_cards_by_sprint(&self, _sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
            unimplemented!()
        }
        fn count_cards_in_column(&self, _column_id: Uuid) -> KanbanResult<usize> {
            unimplemented!()
        }
        fn count_cards_in_column_excluding(
            &self,
            _column_id: Uuid,
            _exclude: &[Uuid],
        ) -> KanbanResult<usize> {
            unimplemented!()
        }
        fn upsert_card(&self, _card: Card) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_card(&self, _id: Uuid) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_cards_by_columns(&self, _column_ids: &[Uuid]) -> KanbanResult<()> {
            unimplemented!()
        }
        fn clear_sprint_from_cards(
            &self,
            _sprint_id: Uuid,
            _timestamp: chrono::DateTime<chrono::Utc>,
        ) -> KanbanResult<()> {
            unimplemented!()
        }
        fn get_archived_card(&self, _card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
            unimplemented!()
        }
        fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
            unimplemented!()
        }
        fn insert_archived_card(&self, _ac: ArchivedCard) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_archived_card(&self, _card_id: Uuid) -> KanbanResult<()> {
            unimplemented!()
        }
        fn get_sprint(&self, _id: Uuid) -> KanbanResult<Option<Sprint>> {
            unimplemented!()
        }
        fn list_sprints_by_board(&self, _board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
            unimplemented!()
        }
        fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
            unimplemented!()
        }
        fn upsert_sprint(&self, _sprint: Sprint) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_sprint(&self, _id: Uuid) -> KanbanResult<()> {
            unimplemented!()
        }
        fn delete_sprints_by_board(&self, _board_id: Uuid) -> KanbanResult<()> {
            unimplemented!()
        }
        fn get_graph(&self) -> KanbanResult<DependencyGraph> {
            unimplemented!()
        }
        fn set_graph(&self, _graph: DependencyGraph) -> KanbanResult<()> {
            unimplemented!()
        }
        fn snapshot(&self) -> KanbanResult<Snapshot> {
            unimplemented!()
        }
        fn apply_snapshot(&self, _snapshot: Snapshot) -> KanbanResult<()> {
            unimplemented!()
        }
    }

    impl CommandStore for StubBackend {
        fn append_batch(&self, _batch: &CommandBatch) -> KanbanResult<u64> {
            unimplemented!()
        }
        fn batch_count(&self) -> KanbanResult<u64> {
            unimplemented!()
        }
        fn load_batches(&self, _offset: u64, _limit: u64) -> KanbanResult<Vec<CommandBatch>> {
            unimplemented!()
        }
    }

    impl KanbanBackend for StubBackend {
        fn as_data_store(&self) -> &dyn DataStore {
            self
        }
        fn with_transaction(&self, _f: TransactionFn<'_>) -> KanbanResult<()> {
            unimplemented!()
        }
    }

    #[test]
    fn test_backend_without_local_persistence_returns_none() {
        let backend = StubBackend;
        let backend: &dyn KanbanBackend = &backend;
        assert!(backend.local_persistence().is_none());
    }
}
