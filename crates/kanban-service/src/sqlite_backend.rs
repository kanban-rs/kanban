use async_trait::async_trait;
use chrono::{DateTime, Utc};
use kanban_backend_memory::InMemoryStore;
use kanban_domain::command_batch::CommandBatch;
use kanban_domain::command_store::CommandStore;
use kanban_domain::data_store::DataStore;
use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, DependencyGraph, GraphMutFn, KanbanError,
    KanbanResult, Snapshot, Sprint,
};
use kanban_persistence::{PersistenceMetadata, PersistenceStore};
use kanban_persistence_sqlite::SqliteStore;
use uuid::Uuid;

pub struct SqliteBackend {
    db: SqliteStore,
    /// In-session command log. The on-disk `command_log` table exists
    /// in the schema but is not yet wired through this backend.
    mem: InMemoryStore,
    /// Most-recent metadata observed from the underlying DB. Populated on
    /// `open()` and refreshed inside `flush()` after the writer-stamp UPDATE.
    /// Mirrors `JsonDataStore::last_metadata` so `persistence_metadata()` —
    /// called once per TUI render via the F12 diagnostics panel — is a
    /// RwLock read instead of a SELECT round-trip.
    last_metadata: std::sync::RwLock<Option<PersistenceMetadata>>,
}

impl SqliteBackend {
    pub async fn open(locator: &str) -> KanbanResult<Self> {
        let db = SqliteStore::open(locator).await?;
        let initial = db.read_metadata_sync()?;
        Ok(Self {
            db,
            mem: InMemoryStore::new(),
            last_metadata: std::sync::RwLock::new(initial),
        })
    }
}

// ─── DataStore ───────────────────────────────────────────────────────────────

impl DataStore for SqliteBackend {
    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.db.get_board(id)
    }
    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.db.list_boards()
    }
    fn upsert_board(&self, board: Board) -> KanbanResult<()> {
        self.db.upsert_board(board)
    }
    fn delete_board(&self, id: Uuid) -> KanbanResult<()> {
        self.db.delete_board(id)
    }

    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.db.get_column(id)
    }
    fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.db.list_columns_by_board(board_id)
    }
    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        self.db.list_all_columns()
    }
    fn upsert_column(&self, column: Column) -> KanbanResult<()> {
        self.db.upsert_column(column)
    }
    fn delete_column(&self, id: Uuid) -> KanbanResult<()> {
        self.db.delete_column(id)
    }
    fn delete_columns_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.db.delete_columns_by_board(board_id)
    }

    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        self.db.get_card(id)
    }
    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        self.db.list_all_cards()
    }
    fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.db.list_cards_by_column(column_id)
    }
    fn list_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<Vec<Card>> {
        self.db.list_cards_by_columns(column_ids)
    }
    fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.db.list_cards_by_sprint(sprint_id)
    }
    fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
        self.db.count_cards_in_column(column_id)
    }
    fn count_cards_in_column_excluding(
        &self,
        column_id: Uuid,
        exclude: &[Uuid],
    ) -> KanbanResult<usize> {
        self.db.count_cards_in_column_excluding(column_id, exclude)
    }
    fn list_cards_by_column_filtered(
        &self,
        column_id: Uuid,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<Vec<Card>> {
        self.db.list_cards_by_column_filtered(column_id, archived)
    }
    fn count_cards_in_column_filtered(
        &self,
        column_id: Uuid,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<usize> {
        self.db.count_cards_in_column_filtered(column_id, archived)
    }
    fn upsert_card(&self, card: Card) -> KanbanResult<()> {
        self.db.upsert_card(card)
    }
    fn delete_card(&self, id: Uuid) -> KanbanResult<()> {
        self.db.delete_card(id)
    }
    fn delete_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<()> {
        self.db.delete_cards_by_columns(column_ids)
    }
    fn clear_sprint_from_cards(
        &self,
        sprint_id: Uuid,
        timestamp: DateTime<Utc>,
    ) -> KanbanResult<()> {
        self.db.clear_sprint_from_cards(sprint_id, timestamp)
    }

    fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        self.db.get_archived_card(card_id)
    }
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.db.list_archived_cards()
    }
    fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
        self.db.insert_archived_card(ac)
    }
    fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
        self.db.delete_archived_card(card_id)
    }
    fn get_archived_board(&self, board_id: Uuid) -> KanbanResult<Option<ArchivedBoard>> {
        self.db.get_archived_board(board_id)
    }
    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        self.db.list_archived_boards()
    }
    fn insert_archived_board(&self, ab: ArchivedBoard) -> KanbanResult<()> {
        self.db.insert_archived_board(ab)
    }
    fn delete_archived_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.db.delete_archived_board(board_id)
    }
    fn unarchive_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.db.unarchive_board(board_id)
    }
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        self.db.list_archived_cards_by_board(board_id)
    }
    fn clear_sprint_from_archived_cards(
        &self,
        sprint_id: Uuid,
        timestamp: DateTime<Utc>,
    ) -> KanbanResult<()> {
        self.db
            .clear_sprint_from_archived_cards(sprint_id, timestamp)
    }

    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.db.get_sprint(id)
    }
    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.db.list_sprints_by_board(board_id)
    }
    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        self.db.list_all_sprints()
    }
    fn upsert_sprint(&self, sprint: Sprint) -> KanbanResult<()> {
        self.db.upsert_sprint(sprint)
    }
    fn delete_sprint(&self, id: Uuid) -> KanbanResult<()> {
        self.db.delete_sprint(id)
    }
    fn delete_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.db.delete_sprints_by_board(board_id)
    }

    fn get_graph(&self) -> KanbanResult<DependencyGraph> {
        self.db.get_graph()
    }
    fn set_graph(&self, graph: DependencyGraph) -> KanbanResult<()> {
        self.db.set_graph(graph)
    }
    fn modify_graph(&self, f: GraphMutFn) -> KanbanResult<()> {
        self.db.modify_graph(f)
    }

    fn snapshot(&self) -> KanbanResult<Snapshot> {
        self.db.snapshot()
    }
    fn apply_snapshot(&self, snapshot: Snapshot) -> KanbanResult<()> {
        self.db.apply_snapshot(snapshot)
    }
}

// ─── CommandStore ─────────────────────────────────────────────────────────────

// Routes to the in-memory mirror; the on-disk command_log table stays
// unwritten until a separate piece of work wires it up.
impl CommandStore for SqliteBackend {
    fn append_batch(&self, batch: &CommandBatch) -> KanbanResult<u64> {
        self.mem.append_batch(batch)
    }
    fn batch_count(&self) -> KanbanResult<u64> {
        self.mem.batch_count()
    }
    fn load_batches(&self, from: u64, to: u64) -> KanbanResult<Vec<CommandBatch>> {
        self.mem.load_batches(from, to)
    }
    /// Delegate to the mirror's atomic count+load rather than the
    /// non-atomic trait default, so a concurrent append cannot land
    /// between the count and the load.
    fn load_all_batches(&self) -> KanbanResult<(Vec<CommandBatch>, u64)> {
        self.mem.load_all_batches()
    }
}

// ─── KanbanBackend ────────────────────────────────────────────────────────────

#[async_trait]
impl crate::backend::KanbanBackend for SqliteBackend {
    fn as_data_store(&self) -> &dyn DataStore {
        self
    }

    async fn flush(&self) -> KanbanResult<()> {
        // Stamp before truncating the WAL: anything in the WAL is about to
        // land in the main DB, so the writer attribution should land
        // alongside it.
        self.db.stamp_writer().await?;
        self.db.checkpoint().await?;
        // Refresh the cached metadata so subsequent persistence_metadata()
        // calls reflect what was just stamped without re-issuing a SELECT.
        // Propagate poison errors rather than swallowing — symmetric with
        // JsonDataStore::do_flush's handling of the same RwLock pattern.
        let fresh = self.db.read_metadata_sync()?;
        let mut guard = self.last_metadata.write().map_err(|_| {
            KanbanError::Internal("sqlite_backend: last_metadata RwLock poisoned".into())
        })?;
        *guard = fresh;
        Ok(())
    }

    fn instance_id(&self) -> Uuid {
        <SqliteStore as PersistenceStore>::instance_id(&self.db)
    }

    fn persistence_metadata(&self) -> Option<PersistenceMetadata> {
        self.last_metadata.read().ok().and_then(|g| g.clone())
    }
}

#[cfg(test)]
mod tests {
    use kanban_domain::{Board, DataStore};

    use super::SqliteBackend;
    use crate::backend::KanbanBackend;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sqlite_backend_needs_flush_returns_false() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.sqlite3");
        let backend = SqliteBackend::open(path.to_str().unwrap()).await.unwrap();
        assert!(!backend.needs_flush());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sqlite_backend_flush_executes_wal_checkpoint() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.sqlite3");
        let backend = SqliteBackend::open(path.to_str().unwrap()).await.unwrap();
        backend
            .upsert_board(Board::new("B", None::<String>))
            .unwrap();
        backend
            .flush()
            .await
            .expect("WAL checkpoint should not error");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sqlite_backend_exposes_metadata_after_flush() {
        use super::SqliteBackend;
        use crate::backend::KanbanBackend;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("md.sqlite3");
        let backend = SqliteBackend::open(path.to_str().unwrap()).await.unwrap();
        backend.flush().await.unwrap();
        let meta = backend
            .persistence_metadata()
            .expect("flushed sqlite backend must expose metadata");
        assert_eq!(
            meta.writer_version.as_deref(),
            Some(kanban_core::KANBAN_VERSION),
            "flushed sqlite backend must stamp writer_version"
        );
        assert_eq!(
            meta.writer_commit.as_deref(),
            Some(kanban_core::KANBAN_COMMIT),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_sqlite_backend_reload_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.sqlite3");
        let backend = SqliteBackend::open(path.to_str().unwrap()).await.unwrap();
        backend
            .upsert_board(Board::new("A", None::<String>))
            .unwrap();
        backend.reload().await.unwrap();
        let boards = backend.list_boards().unwrap();
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].name, "A");
    }
}
