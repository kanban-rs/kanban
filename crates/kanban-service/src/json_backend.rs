use crate::backend::KanbanBackend;
use async_trait::async_trait;
use kanban_domain::command_batch::CommandBatch;
use kanban_domain::data_store::GraphMutFn;
use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, CommandStore, DataStore, DependencyGraph,
    InMemoryStore, KanbanError, KanbanResult, Snapshot, Sprint,
};
use kanban_persistence::{
    snapshot_from_json_bytes, snapshot_to_json_bytes, PersistenceMetadata, PersistenceStore,
    StoreSnapshot,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};
use uuid::Uuid;

/// A lazy JSON backend that wraps a [`PersistenceStore`] (JSON file) with an
/// [`InMemoryStore`] cache. The file is not read until the first [`DataStore`]
/// or [`CommandStore`] method call.
///
/// Construction is always zero-I/O — `new()` never reads the file.
/// The file is read on the first [`DataStore`] or [`CommandStore`] method call.
pub struct JsonDataStore {
    file_store: Arc<dyn PersistenceStore + Send + Sync>,
    /// `None` until first access. Populated by `ensure_loaded()`.
    inner: RwLock<Option<InMemoryStore>>,
    /// Most-recent metadata observed from the underlying file. Updated on
    /// load (via `ensure_loaded`) and after each save. Backs the F12
    /// diagnostics panel.
    last_metadata: RwLock<Option<PersistenceMetadata>>,
    dirty: AtomicBool,
}

impl JsonDataStore {
    pub fn new(file_store: Arc<dyn PersistenceStore + Send + Sync>) -> Self {
        Self {
            file_store,
            inner: RwLock::new(None),
            last_metadata: RwLock::new(None),
            dirty: AtomicBool::new(false),
        }
    }

    /// Ensures the inner store is populated, loading from file if needed.
    /// Uses `file_store.load_sync()` — pure blocking I/O, no Tokio runtime dependency.
    fn ensure_loaded(&self) -> KanbanResult<()> {
        // Fast path: already loaded.
        {
            let guard = self
                .inner
                .read()
                .map_err(|_| KanbanError::Internal("json_backend: inner RwLock poisoned".into()))?;
            if guard.is_some() {
                return Ok(());
            }
        }
        // Read lock released here — no lock held during I/O below.

        // Perform all I/O and build the store outside any lock.
        let store = InMemoryStore::new();

        // Convert via the From impl rather than stringifying — that way typed
        // variants such as `UnsupportedFutureVersion` and `ConflictDetected`
        // survive across the persistence/service boundary and reach the user
        // surfaces (CLI, MCP, TUI) intact. Stringifying them flattens
        // everything into KanbanError::Internal and breaks discriminators
        // like KanbanError::is_unsupported_future_version().
        let loaded = self.file_store.load_sync().map_err(KanbanError::from)?;

        if let Some((ss, meta)) = loaded {
            let snapshot = snapshot_from_json_bytes(&ss.data).map_err(KanbanError::from)?;
            store.apply_snapshot(snapshot)?;
            let mut guard = self.last_metadata.write().map_err(|_| {
                KanbanError::Internal("json_backend: last_metadata RwLock poisoned".into())
            })?;
            *guard = Some(meta);
        }

        // Acquire write lock only to swap in the built store.
        let mut guard = self
            .inner
            .write()
            .map_err(|_| KanbanError::Internal("json_backend: inner RwLock poisoned".into()))?;

        // Another thread may have loaded while we were doing I/O — idempotent.
        if guard.is_some() {
            return Ok(());
        }

        *guard = Some(store);
        drop(guard);

        Ok(())
    }

    fn with_read<T>(&self, f: impl FnOnce(&InMemoryStore) -> KanbanResult<T>) -> KanbanResult<T> {
        self.ensure_loaded()?;
        let guard = self
            .inner
            .read()
            .map_err(|_| KanbanError::Internal("json_backend: inner RwLock poisoned".into()))?;
        f(guard.as_ref().expect("ensure_loaded guarantees Some"))
    }

    /// Performs the actual flush I/O. Called by `flush()` after the dirty flag
    /// has been cleared; `flush()` restores it if this returns an error.
    async fn do_flush(&self) -> KanbanResult<()> {
        // Collect everything we need from the inner store before any await.
        let snapshot = {
            let guard = self
                .inner
                .read()
                .map_err(|_| KanbanError::Internal("json_backend: inner RwLock poisoned".into()))?;

            let store = match guard.as_ref() {
                Some(s) => s,
                None => return Ok(()), // Never loaded — nothing to flush.
            };

            // `guard` is dropped here, before any await.
            store.snapshot()?
        };

        let data = snapshot_to_json_bytes(&snapshot).map_err(KanbanError::from)?;
        let metadata = PersistenceMetadata::new(self.file_store.instance_id());

        let returned = self
            .file_store
            .save(StoreSnapshot { data, metadata })
            .await
            .map_err(KanbanError::from)?;

        let mut guard = self.last_metadata.write().map_err(|_| {
            KanbanError::Internal("json_backend: last_metadata RwLock poisoned".into())
        })?;
        *guard = Some(returned);

        Ok(())
    }

    /// Delegates a mutating operation to the inner [`InMemoryStore`], then marks the backend dirty.
    ///
    /// A shared (read) lock on the outer `RwLock` is sufficient here because:
    /// - The write lock is only ever taken in `ensure_loaded()` to swap `inner` from `None` → `Some`.
    /// - Once `Some`, the inner value is **never replaced**, so concurrent mutation via
    ///   `InMemoryStore`'s own interior `RwLock`s is safe under a shared outer lock.
    fn with_mutate<T>(&self, f: impl FnOnce(&InMemoryStore) -> KanbanResult<T>) -> KanbanResult<T> {
        self.ensure_loaded()?;
        let guard = self
            .inner
            .read()
            .map_err(|_| KanbanError::Internal("json_backend: inner RwLock poisoned".into()))?;
        let result = f(guard.as_ref().expect("ensure_loaded guarantees Some"))?;
        self.dirty.store(true, Ordering::Release);
        Ok(result)
    }
}

// ─── DataStore ────────────────────────────────────────────────────────────────

impl DataStore for JsonDataStore {
    // Board
    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.with_read(|s| s.get_board(id))
    }
    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.with_read(|s| s.list_boards())
    }
    fn upsert_board(&self, board: Board) -> KanbanResult<()> {
        self.with_mutate(|s| s.upsert_board(board))
    }
    fn delete_board(&self, id: Uuid) -> KanbanResult<()> {
        self.with_mutate(|s| s.delete_board(id))
    }

    // Column
    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.with_read(|s| s.get_column(id))
    }
    fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.with_read(|s| s.list_columns_by_board(board_id))
    }
    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        self.with_read(|s| s.list_all_columns())
    }
    fn upsert_column(&self, column: Column) -> KanbanResult<()> {
        self.with_mutate(|s| s.upsert_column(column))
    }
    fn delete_column(&self, id: Uuid) -> KanbanResult<()> {
        self.with_mutate(|s| s.delete_column(id))
    }
    fn delete_columns_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.with_mutate(|s| s.delete_columns_by_board(board_id))
    }

    // Card
    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        self.with_read(|s| s.get_card(id))
    }
    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        self.with_read(|s| s.list_all_cards())
    }
    fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.with_read(|s| s.list_cards_by_column(column_id))
    }
    fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.with_read(|s| s.list_cards_by_sprint(sprint_id))
    }
    fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
        self.with_read(|s| s.count_cards_in_column(column_id))
    }
    fn count_cards_in_column_excluding(
        &self,
        column_id: Uuid,
        exclude: &[Uuid],
    ) -> KanbanResult<usize> {
        self.with_read(|s| s.count_cards_in_column_excluding(column_id, exclude))
    }
    fn upsert_card(&self, card: Card) -> KanbanResult<()> {
        self.with_mutate(|s| s.upsert_card(card))
    }
    fn delete_card(&self, id: Uuid) -> KanbanResult<()> {
        self.with_mutate(|s| s.delete_card(id))
    }
    fn delete_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<()> {
        self.with_mutate(|s| s.delete_cards_by_columns(column_ids))
    }
    fn clear_sprint_from_cards(
        &self,
        sprint_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        self.with_mutate(|s| s.clear_sprint_from_cards(sprint_id, timestamp))
    }

    // Archived card
    fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        self.with_read(|s| s.get_archived_card(card_id))
    }
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.with_read(|s| s.list_archived_cards())
    }
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        self.with_read(|s| s.list_archived_cards_by_board(board_id))
    }
    fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
        self.with_mutate(|s| s.insert_archived_card(ac))
    }
    fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
        self.with_mutate(|s| s.delete_archived_card(card_id))
    }
    fn get_archived_board(&self, board_id: Uuid) -> KanbanResult<Option<ArchivedBoard>> {
        self.with_read(|s| s.get_archived_board(board_id))
    }
    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        self.with_read(|s| s.list_archived_boards())
    }
    fn insert_archived_board(&self, ab: ArchivedBoard) -> KanbanResult<()> {
        self.with_mutate(|s| s.insert_archived_board(ab))
    }
    fn delete_archived_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.with_mutate(|s| s.delete_archived_board(board_id))
    }
    fn unarchive_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.with_mutate(|s| s.unarchive_board(board_id))
    }
    fn clear_sprint_from_archived_cards(
        &self,
        sprint_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        self.with_mutate(|s| s.clear_sprint_from_archived_cards(sprint_id, timestamp))
    }

    // Sprint
    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.with_read(|s| s.get_sprint(id))
    }
    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.with_read(|s| s.list_sprints_by_board(board_id))
    }
    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        self.with_read(|s| s.list_all_sprints())
    }
    fn upsert_sprint(&self, sprint: Sprint) -> KanbanResult<()> {
        self.with_mutate(|s| s.upsert_sprint(sprint))
    }
    fn delete_sprint(&self, id: Uuid) -> KanbanResult<()> {
        self.with_mutate(|s| s.delete_sprint(id))
    }
    fn delete_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.with_mutate(|s| s.delete_sprints_by_board(board_id))
    }

    // Graph
    fn get_graph(&self) -> KanbanResult<DependencyGraph> {
        self.with_read(|s| s.get_graph())
    }
    fn set_graph(&self, graph: DependencyGraph) -> KanbanResult<()> {
        self.with_mutate(|s| s.set_graph(graph))
    }
    fn modify_graph(&self, f: GraphMutFn) -> KanbanResult<()> {
        self.with_mutate(|s| s.modify_graph(f))
    }

    // Snapshot
    fn snapshot(&self) -> KanbanResult<Snapshot> {
        self.with_read(|s| s.snapshot())
    }
    fn apply_snapshot(&self, snapshot: Snapshot) -> KanbanResult<()> {
        self.with_mutate(|s| s.apply_snapshot(snapshot))
    }
}

// ─── CommandStore ─────────────────────────────────────────────────────────────

impl CommandStore for JsonDataStore {
    fn append_batch(&self, batch: &CommandBatch) -> KanbanResult<u64> {
        self.with_mutate(|s| s.append_batch(batch))
    }
    fn batch_count(&self) -> KanbanResult<u64> {
        self.with_read(|s| s.batch_count())
    }
    fn load_batches(&self, from: u64, to: u64) -> KanbanResult<Vec<CommandBatch>> {
        self.with_read(|s| s.load_batches(from, to))
    }
    fn load_all_batches(&self) -> KanbanResult<(Vec<CommandBatch>, u64)> {
        self.with_read(|s| s.load_all_batches())
    }
}

// ─── KanbanBackend ────────────────────────────────────────────────────────────

#[async_trait]
impl KanbanBackend for JsonDataStore {
    fn as_data_store(&self) -> &dyn DataStore {
        self
    }

    async fn flush(&self) -> KanbanResult<()> {
        if !self.dirty.swap(false, Ordering::AcqRel) {
            return Ok(());
        }
        let result = self.do_flush().await;
        if result.is_err() {
            self.dirty.store(true, Ordering::Release);
        }
        result
    }

    async fn reload(&self) -> KanbanResult<()> {
        {
            let mut guard = self
                .inner
                .write()
                .map_err(|_| KanbanError::Internal("json_backend: inner RwLock poisoned".into()))?;
            *guard = None;
        } // inner write lock released — mirrors ensure_loaded's ordering
        self.dirty.store(false, Ordering::Release);
        Ok(())
    }

    fn needs_flush(&self) -> bool {
        self.dirty.load(Ordering::Acquire)
    }

    fn needs_save_worker(&self) -> bool {
        true
    }

    fn instance_id(&self) -> Uuid {
        self.file_store.instance_id()
    }

    fn persistence_metadata(&self) -> Option<PersistenceMetadata> {
        // Surface what we've observed; do NOT trigger a load here — the
        // backend may be queried before any DataStore call (e.g. when the TUI
        // renders its diagnostics panel on startup), and we don't want
        // persistence_metadata() to do I/O. ensure_loaded populates the
        // cache as a side effect of any DataStore call, which is enough.
        self.last_metadata.read().ok().and_then(|g| g.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::Board;
    use kanban_persistence_json::JsonFileStore;
    use tempfile::tempdir;

    fn make_store(path: &std::path::Path) -> JsonDataStore {
        JsonDataStore::new(Arc::new(JsonFileStore::new(path)))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_json_backend_exposes_metadata_after_flush() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("md.json");
        let jds = make_store(&path);
        // Trigger a write so flush has something to persist.
        jds.upsert_board(Board::new("B", None::<String>)).unwrap();
        jds.flush().await.unwrap();
        let meta = jds
            .persistence_metadata()
            .expect("flushed JSON backend must expose metadata");
        assert_eq!(
            meta.writer_version.as_deref(),
            Some(kanban_core::KANBAN_VERSION),
        );
        assert_eq!(
            meta.writer_commit.as_deref(),
            Some(kanban_core::KANBAN_COMMIT),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_json_backend_metadata_is_none_before_any_load_or_save() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("untouched.json");
        let jds = make_store(&path);
        // No DataStore call yet → ensure_loaded never ran → cache empty.
        assert!(jds.persistence_metadata().is_none());
    }

    /// Verifies that `ensure_loaded` no longer relies on `block_in_place`, so
    /// it works from a current-thread (single-threaded) Tokio runtime.
    #[tokio::test]
    async fn test_ensure_loaded_works_in_single_thread_runtime() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("single_thread.json");
        let jds = make_store(&path);
        // Must not panic — block_in_place panics on single-threaded runtimes.
        let boards = jds.list_boards().unwrap();
        assert!(boards.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_flush_restores_dirty_flag_on_io_failure() {
        use async_trait::async_trait;
        use kanban_persistence::{
            PersistenceError, PersistenceMetadata, PersistenceResult, PersistenceStore,
            StoreSnapshot,
        };

        struct FailingStore;

        #[async_trait]
        impl PersistenceStore for FailingStore {
            async fn save(&self, _: StoreSnapshot) -> PersistenceResult<PersistenceMetadata> {
                Err(PersistenceError::Io(std::io::Error::other(
                    "injected save failure",
                )))
            }
            async fn load(&self) -> PersistenceResult<(StoreSnapshot, PersistenceMetadata)> {
                Err(PersistenceError::Serialization("not implemented".into()))
            }
            async fn exists(&self) -> bool {
                false
            }
            fn path(&self) -> &std::path::Path {
                std::path::Path::new("")
            }
            fn instance_id(&self) -> uuid::Uuid {
                uuid::Uuid::nil()
            }
            fn load_sync(&self) -> PersistenceResult<Option<(StoreSnapshot, PersistenceMetadata)>> {
                Ok(None)
            }
        }

        let jds = JsonDataStore::new(Arc::new(FailingStore));
        jds.upsert_board(Board::new("B", None::<String>)).unwrap();
        assert!(jds.needs_flush(), "must be dirty before flush attempt");

        let result = jds.flush().await;
        assert!(result.is_err(), "flush should propagate the I/O failure");
        assert!(
            jds.needs_flush(),
            "dirty flag must be restored after a failed flush"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_construction_does_no_io() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        // File does not exist; construction must not panic or error.
        let _store = make_store(&path);
        assert!(!path.exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_list_boards_triggers_load_on_existing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.json");

        // Pre-create a file with one board.
        let (boards_json, _) = {
            let store = JsonFileStore::new(&path);
            let board = Board::new("Alpha", None::<String>);
            let snap = Snapshot {
                archived_boards: Vec::new(),
                boards: vec![board],
                ..Snapshot::new()
            };
            let data = snapshot_to_json_bytes(&snap).unwrap();
            let meta = PersistenceMetadata::new(Uuid::new_v4());
            store
                .save(StoreSnapshot {
                    data,
                    metadata: meta,
                })
                .await
                .unwrap();
            (snap.boards, ())
        };

        let jds = make_store(&path);
        // Inner must be None before any access.
        {
            let guard = jds.inner.read().unwrap();
            assert!(guard.is_none(), "inner should be None before first read");
        }

        let boards = jds.list_boards().unwrap();
        assert_eq!(boards.len(), boards_json.len());
        assert_eq!(boards[0].name, "Alpha");

        // Inner must now be Some.
        {
            let guard = jds.inner.read().unwrap();
            assert!(guard.is_some(), "inner should be Some after first read");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_needs_flush_false_when_clean() {
        let dir = tempdir().unwrap();
        let jds = make_store(&dir.path().join("t.json"));
        assert!(!jds.needs_flush());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_needs_flush_true_after_upsert() {
        let dir = tempdir().unwrap();
        let jds = make_store(&dir.path().join("t.json"));
        jds.upsert_board(Board::new("B", None::<String>)).unwrap();
        assert!(jds.needs_flush());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_flush_writes_to_disk() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("flush.json");
        let jds = make_store(&path);
        jds.upsert_board(Board::new("Flushed", None::<String>))
            .unwrap();
        jds.flush().await.unwrap();
        assert!(!jds.needs_flush(), "dirty flag cleared after flush");

        let jds2 = make_store(&path);
        let boards = jds2.list_boards().unwrap();
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].name, "Flushed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_reload_clears_cache() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("reload.json");

        // Write initial data via a separate store.
        let writer = make_store(&path);
        writer
            .upsert_board(Board::new("Initial", None::<String>))
            .unwrap();
        writer.flush().await.unwrap();

        // Open the same file in a second store and load it.
        let reader = make_store(&path);
        let boards = reader.list_boards().unwrap();
        assert_eq!(boards[0].name, "Initial");

        // Externally update the file by flushing a new board through the writer.
        writer
            .upsert_board(Board::new("Updated", None::<String>))
            .unwrap();
        writer.flush().await.unwrap();

        // Before reload, reader still sees stale data.
        let boards = reader.list_boards().unwrap();
        assert_eq!(boards.len(), 1, "stale before reload");

        reader.reload().await.unwrap();
        let boards = reader.list_boards().unwrap();
        assert_eq!(boards.len(), 2, "should see both boards after reload");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_needs_save_worker_returns_true() {
        let dir = tempdir().unwrap();
        let jds = make_store(&dir.path().join("t.json"));
        assert!(jds.needs_save_worker());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_apply_snapshot_sets_dirty_flag() {
        let dir = tempdir().unwrap();
        let jds = make_store(&dir.path().join("t.json"));
        jds.apply_snapshot(Snapshot::new()).unwrap();
        assert!(jds.needs_flush(), "apply_snapshot must mark backend dirty");
    }

    // ─── F2 (KAN-871): JSON file seam round-trips the reference archival model ──
    //
    // After F1 an archived card stays LIVE in `cards` behind a marker. These
    // pin that the V9 on-disk shape carries the archived entity under
    // `archived_cards` ONLY (never duplicated in `cards`), and that a
    // save->load cycle re-establishes the reference model: the card is live
    // (source of truth, reachable by id), hidden from the live list, and
    // retrievable as archived. Whole-entity fidelity is asserted through
    // `get_card` (the live entity IS the source of truth) so the tests stay
    // correct through the F3b `Archived<T>` collapse.

    /// Seed one live card and one archived card, flush, and inspect the raw
    /// on-disk JSON: the archived card's id lives under `archived_cards.entity`
    /// and MUST NOT also appear under `cards`.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_archived_card_not_duplicated_in_ondisk_cards() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("arch_dedup.json");
        let jds = make_store(&path);

        let mut board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let live = Card::new(&mut board, col_id, "Live", 0);
        let archived = Card::new(&mut board, col_id, "Archived", 1);
        let archived_id = archived.id;
        jds.upsert_card(live).unwrap();
        jds.upsert_card(archived.clone()).unwrap();
        jds.insert_archived_card(ArchivedCard::new(archived, board.id, col_id, 1))
            .unwrap();
        jds.flush().await.unwrap();

        let on_disk: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let cards = on_disk["data"]["cards"].as_array().unwrap();
        let archived_cards = on_disk["data"]["archived_cards"].as_array().unwrap();
        let id_str = archived_id.to_string();

        assert!(
            archived_cards
                .iter()
                .any(|a| a["entity"]["id"].as_str() == Some(id_str.as_str())),
            "archived card entity must be serialized under archived_cards"
        );
        assert!(
            cards
                .iter()
                .all(|c| c["id"].as_str() != Some(id_str.as_str())),
            "archived card id must NOT be duplicated under on-disk cards"
        );
        assert_eq!(
            cards.len(),
            1,
            "only the live card stays under on-disk cards"
        );
    }

    /// Archive a card, flush, drop the store, reload: the card is live
    /// (reachable by id), hidden from `list_all_cards`, and retrievable as
    /// archived. This is the reference model re-established through the file.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_json_save_load_preserves_reference_archival() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("ref_model.json");
        let jds = make_store(&path);

        let mut board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let card = Card::new(&mut board, col_id, "ToArchive", 0);
        let card_id = card.id;
        jds.upsert_card(card.clone()).unwrap();
        jds.insert_archived_card(ArchivedCard::new(card, board.id, col_id, 0))
            .unwrap();
        jds.flush().await.unwrap();

        let reader = make_store(&path);
        let live = reader
            .get_card(card_id)
            .unwrap()
            .expect("archived card stays live and reachable by id after reload");
        assert_eq!(live.title, "ToArchive");
        assert!(
            reader
                .list_all_cards()
                .unwrap()
                .iter()
                .all(|c| c.id != card_id),
            "archived card must be hidden from the live list after reload"
        );
        assert!(
            reader.get_archived_card(card_id).unwrap().is_some(),
            "archived card must be retrievable as archived after reload"
        );
    }

    /// Every `Card` field survives the file round-trip losslessly. Read the
    /// whole entity back through `get_card` (the live source of truth) so the
    /// assertion holds after the F3b marker collapse.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_json_roundtrip_archived_card_whole_entity_stable() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("whole_entity.json");
        let jds = make_store(&path);

        let mut board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let mut card = Card::new(&mut board, col_id, "Full", 3);
        card.description = Some("rich body".into());
        card.priority = kanban_domain::CardPriority::High;
        card.sprint_id = Some(Uuid::new_v4());
        let original = card.clone();
        jds.upsert_card(card.clone()).unwrap();
        jds.insert_archived_card(ArchivedCard::new(card, board.id, col_id, 3))
            .unwrap();
        jds.flush().await.unwrap();

        let reader = make_store(&path);
        let reloaded = reader
            .get_card(original.id)
            .unwrap()
            .expect("archived card is reachable as a live entity by id");
        assert_eq!(reloaded, original, "no Card field may be lost or altered");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_ensure_loaded_is_idempotent_under_concurrent_access() {
        use kanban_persistence::{PersistenceMetadata, PersistenceStore, StoreSnapshot};

        let dir = tempdir().unwrap();
        let path = dir.path().join("concurrent.json");

        // Pre-populate the file with one board.
        {
            let store = Arc::new(JsonFileStore::new(&path));
            let board = Board::new("ConcurrentBoard", None::<String>);
            let snap = kanban_domain::Snapshot {
                archived_boards: Vec::new(),
                boards: vec![board],
                ..kanban_domain::Snapshot::new()
            };
            let data = snapshot_to_json_bytes(&snap).unwrap();
            store
                .save(StoreSnapshot {
                    data,
                    metadata: PersistenceMetadata::new(uuid::Uuid::new_v4()),
                })
                .await
                .unwrap();
        }

        let jds = Arc::new(make_store(&path));
        let jds2 = Arc::clone(&jds);

        let t1 = tokio::task::spawn_blocking(move || jds.list_boards());
        let t2 = tokio::task::spawn_blocking(move || jds2.list_boards());

        let (r1, r2) = tokio::join!(t1, t2);
        let boards1 = r1.unwrap().unwrap();
        let boards2 = r2.unwrap().unwrap();

        assert_eq!(boards1.len(), 1);
        assert_eq!(boards2.len(), 1);
        assert_eq!(boards1[0].name, "ConcurrentBoard");
        assert_eq!(boards2[0].name, "ConcurrentBoard");
    }
}
