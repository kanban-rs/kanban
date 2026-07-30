pub mod remote_writes;
pub use remote_writes::RemoteWrites;

use async_trait::async_trait;
use kanban_domain::command_store::CommandStore;
use kanban_domain::data_store::DataStore;
use kanban_domain::{InMemoryStore, KanbanError, KanbanResult};
use kanban_persistence::PersistenceMetadata;
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

    /// Metadata about the underlying persistence store (file format version,
    /// writer kanban version, writer commit, last save time). Returns `None`
    /// for in-memory backends or when no metadata has been observed yet.
    /// Surfaced by the TUI's F12 diagnostics panel.
    fn persistence_metadata(&self) -> Option<PersistenceMetadata> {
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

    /// Run `f` as an atomic batch: every mutation commits or rolls
    /// back together. The default impl snapshots state before `f`
    /// runs and restores it on failure — cheap for in-memory backends,
    /// expensive on disk. Disk-backed backends should override with a
    /// native transaction.
    fn with_transaction(&self, f: &mut dyn FnMut() -> KanbanResult<()>) -> KanbanResult<()> {
        let before = self.snapshot()?;
        match f() {
            Ok(()) => Ok(()),
            Err(e) => {
                if let Err(rollback_err) = self.apply_snapshot(before) {
                    return Err(KanbanError::Internal(format!(
                        "Batch failed ({e}) and rollback also failed ({rollback_err}). State may be inconsistent."
                    )));
                }
                Err(e)
            }
        }
    }
}

// ─── InMemoryStore ───────────────────────────────────────────────────────────

impl KanbanBackend for InMemoryStore {
    fn as_data_store(&self) -> &dyn DataStore {
        self
    }
    // All lifecycle defaults are correct for in-memory: flush=noop, reload=noop,
    // needs_flush=false, needs_save_worker=false.
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use kanban_domain::InMemoryStore;

    #[test]
    fn test_kanban_backend_is_object_safe() {
        let store = InMemoryStore::new();
        let _: &dyn KanbanBackend = &store;
    }

    #[test]
    fn test_as_data_store_returns_data_store_ref() {
        let store = InMemoryStore::new();
        let backend: &dyn KanbanBackend = &store;
        let _: &dyn DataStore = backend.as_data_store();
    }

    #[test]
    fn test_in_memory_store_needs_flush_returns_false() {
        let store = InMemoryStore::new();
        assert!(!store.needs_flush());
    }

    #[test]
    fn test_in_memory_store_needs_save_worker_returns_false() {
        let store = InMemoryStore::new();
        assert!(!store.needs_save_worker());
    }

    #[tokio::test]
    async fn test_in_memory_store_flush_is_noop() {
        let store = InMemoryStore::new();
        store.flush().await.expect("flush should be a no-op");
    }

    #[tokio::test]
    async fn test_in_memory_store_reload_is_noop() {
        let store = InMemoryStore::new();
        store.reload().await.expect("reload should be a no-op");
    }

    #[test]
    fn test_in_memory_backend_returns_none_persistence_metadata() {
        let store = InMemoryStore::new();
        let backend: &dyn KanbanBackend = &store;
        assert!(backend.persistence_metadata().is_none());
    }

    #[test]
    fn test_in_memory_backend_health_checker_returns_none() {
        let store = InMemoryStore::new();
        let backend: &dyn KanbanBackend = &store;
        assert!(backend.health_checker().is_none());
    }

    #[test]
    fn test_remote_writes_defaults_to_none_for_local_backends() {
        let store = InMemoryStore::new();
        let backend: &dyn KanbanBackend = &store;
        assert!(backend.remote_writes().is_none());
    }

    // SQLite KanbanBackend lifecycle tests
}

#[cfg(any(test, feature = "test-helpers"))]
pub mod test_support {
    use crate::KanbanBackend;
    use crate::RemoteWrites;
    use kanban_domain::{
        Board, BoardUpdate, Card, CardUpdate, Column, ColumnUpdate, CommandBatch, CommandStore,
        DataStore, InMemoryStore, KanbanResult, NewBoard, NewCard, NewColumn, Snapshot,
    };
    use uuid::Uuid;

    pub struct MockRemoteWritesImpl;

    impl RemoteWrites for MockRemoteWritesImpl {
        fn create_board(&self, _id: Option<Uuid>, _spec: &NewBoard) -> KanbanResult<Board> {
            unimplemented!("test should not call this")
        }
        fn update_board(&self, _id: Uuid, _updates: &BoardUpdate) -> KanbanResult<Board> {
            unimplemented!("test should not call this")
        }
        fn delete_board(&self, _id: Uuid) -> KanbanResult<()> {
            unimplemented!("test should not call this")
        }
        fn create_column(&self, _board_id: Uuid, _spec: &NewColumn) -> KanbanResult<Column> {
            unimplemented!("test should not call this")
        }
        fn update_column(&self, _id: Uuid, _updates: &ColumnUpdate) -> KanbanResult<Column> {
            unimplemented!("test should not call this")
        }
        fn delete_column(&self, _id: Uuid) -> KanbanResult<()> {
            unimplemented!("test should not call this")
        }
        fn create_card(&self, _id: Option<Uuid>, _spec: &NewCard) -> KanbanResult<Card> {
            unimplemented!("test should not call this")
        }
        fn update_card(&self, _id: Uuid, _updates: &CardUpdate) -> KanbanResult<Card> {
            unimplemented!("test should not call this")
        }
        fn delete_card(&self, _id: Uuid) -> KanbanResult<()> {
            unimplemented!("test should not call this")
        }
    }

    pub struct MockBackend {
        inner: InMemoryStore,
        mock: MockRemoteWritesImpl,
    }

    impl MockBackend {
        pub fn new() -> Self {
            Self {
                inner: InMemoryStore::new(),
                mock: MockRemoteWritesImpl,
            }
        }
    }

    impl Default for MockBackend {
        fn default() -> Self {
            Self::new()
        }
    }

    impl DataStore for MockBackend {
        fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
            self.inner.get_board(id)
        }
        fn list_boards(&self) -> KanbanResult<Vec<Board>> {
            self.inner.list_boards()
        }
        fn upsert_board(&self, board: Board) -> KanbanResult<()> {
            self.inner.upsert_board(board)
        }
        fn delete_board(&self, id: Uuid) -> KanbanResult<()> {
            self.inner.delete_board(id)
        }
        fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
            self.inner.get_column(id)
        }
        fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
            self.inner.list_columns_by_board(board_id)
        }
        fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
            self.inner.list_all_columns()
        }
        fn upsert_column(&self, column: Column) -> KanbanResult<()> {
            self.inner.upsert_column(column)
        }
        fn delete_column(&self, id: Uuid) -> KanbanResult<()> {
            self.inner.delete_column(id)
        }
        fn delete_columns_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
            self.inner.delete_columns_by_board(board_id)
        }
        fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
            self.inner.get_card(id)
        }
        fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
            self.inner.list_all_cards()
        }
        fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
            self.inner.list_cards_by_column(column_id)
        }
        fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
            self.inner.list_cards_by_sprint(sprint_id)
        }
        fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
            self.inner.count_cards_in_column(column_id)
        }
        fn count_cards_in_column_excluding(
            &self,
            column_id: Uuid,
            exclude_ids: &[Uuid],
        ) -> KanbanResult<usize> {
            self.inner
                .count_cards_in_column_excluding(column_id, exclude_ids)
        }
        fn upsert_card(&self, card: Card) -> KanbanResult<()> {
            self.inner.upsert_card(card)
        }
        fn delete_card(&self, id: Uuid) -> KanbanResult<()> {
            self.inner.delete_card(id)
        }
        fn delete_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<()> {
            self.inner.delete_cards_by_columns(column_ids)
        }
        fn clear_sprint_from_cards(
            &self,
            sprint_id: Uuid,
            cleared_at: chrono::DateTime<chrono::Utc>,
        ) -> KanbanResult<()> {
            self.inner.clear_sprint_from_cards(sprint_id, cleared_at)
        }
        fn get_archived_card(
            &self,
            card_id: Uuid,
        ) -> KanbanResult<Option<kanban_domain::ArchivedCard>> {
            self.inner.get_archived_card(card_id)
        }
        fn list_archived_cards(&self) -> KanbanResult<Vec<kanban_domain::ArchivedCard>> {
            self.inner.list_archived_cards()
        }
        fn insert_archived_card(&self, ac: kanban_domain::ArchivedCard) -> KanbanResult<()> {
            self.inner.insert_archived_card(ac)
        }
        fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
            self.inner.delete_archived_card(card_id)
        }
        fn get_archived_board(
            &self,
            board_id: Uuid,
        ) -> KanbanResult<Option<kanban_domain::ArchivedBoard>> {
            self.inner.get_archived_board(board_id)
        }
        fn list_archived_boards(&self) -> KanbanResult<Vec<kanban_domain::ArchivedBoard>> {
            self.inner.list_archived_boards()
        }
        fn insert_archived_board(&self, ab: kanban_domain::ArchivedBoard) -> KanbanResult<()> {
            self.inner.insert_archived_board(ab)
        }
        fn delete_archived_board(&self, board_id: Uuid) -> KanbanResult<()> {
            self.inner.delete_archived_board(board_id)
        }
        fn unarchive_board(&self, board_id: Uuid) -> KanbanResult<()> {
            self.inner.unarchive_board(board_id)
        }
        fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<kanban_domain::Sprint>> {
            self.inner.get_sprint(id)
        }
        fn list_sprints_by_board(
            &self,
            board_id: Uuid,
        ) -> KanbanResult<Vec<kanban_domain::Sprint>> {
            self.inner.list_sprints_by_board(board_id)
        }
        fn list_all_sprints(&self) -> KanbanResult<Vec<kanban_domain::Sprint>> {
            self.inner.list_all_sprints()
        }
        fn upsert_sprint(&self, sprint: kanban_domain::Sprint) -> KanbanResult<()> {
            self.inner.upsert_sprint(sprint)
        }
        fn delete_sprint(&self, id: Uuid) -> KanbanResult<()> {
            self.inner.delete_sprint(id)
        }
        fn delete_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
            self.inner.delete_sprints_by_board(board_id)
        }
        fn get_graph(&self) -> KanbanResult<kanban_domain::DependencyGraph> {
            self.inner.get_graph()
        }
        fn set_graph(&self, graph: kanban_domain::DependencyGraph) -> KanbanResult<()> {
            self.inner.set_graph(graph)
        }
        fn snapshot(&self) -> KanbanResult<Snapshot> {
            self.inner.snapshot()
        }
        fn apply_snapshot(&self, snapshot: Snapshot) -> KanbanResult<()> {
            self.inner.apply_snapshot(snapshot)
        }
    }

    impl CommandStore for MockBackend {
        fn append_batch(&self, batch: &CommandBatch) -> KanbanResult<u64> {
            self.inner.append_batch(batch)
        }
        fn batch_count(&self) -> KanbanResult<u64> {
            self.inner.batch_count()
        }
        fn load_batches(&self, offset: u64, limit: u64) -> KanbanResult<Vec<CommandBatch>> {
            self.inner.load_batches(offset, limit)
        }
    }

    impl KanbanBackend for MockBackend {
        fn as_data_store(&self) -> &dyn DataStore {
            self
        }

        fn remote_writes(&self) -> Option<&dyn RemoteWrites> {
            Some(&self.mock)
        }
    }
}
