#![allow(dead_code)]

use kanban_backend::{KanbanBackend, RemoteWrites, TransactionFn};
use kanban_domain::KanbanError;
use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, CommandBatch, CommandStore, DataStore,
    DependencyGraph, KanbanResult, Snapshot, Sprint,
};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::app::ExportDialogState;
use kanban_tui::App;
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// One recorded store read: which method was called, and the id(s) it was
/// called with. Empty for collection-shaped reads such as `list_boards`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOp {
    pub method: &'static str,
    pub ids: Vec<Uuid>,
}

pub type ReadOpLog = Arc<Mutex<Vec<ReadOp>>>;

pub type WrappedBackend = (Arc<dyn KanbanBackend>, Arc<AtomicUsize>, ReadOpLog);

/// Asserts `log` holds exactly `expected`, in order. A log that is a strict
/// superset or subset of `expected` fails.
pub fn assert_ops(log: &Mutex<Vec<ReadOp>>, expected: &[ReadOp]) {
    let actual = log.lock().unwrap().clone();
    assert_eq!(
        actual.as_slice(),
        expected,
        "read op log mismatch\n  expected: {expected:?}\n  actual:   {actual:?}"
    );
}

/// A `KanbanBackend` decorator that counts every DataStore/CommandStore READ
/// method invoked, delegating all reads and writes verbatim to `inner`.
/// Writes are never counted. Only required trait methods are overridden;
/// default trait methods route through the instrumented required ones.
pub struct CountingBackend {
    inner: Arc<dyn KanbanBackend>,
    reads: Arc<AtomicUsize>,
    ops: ReadOpLog,
    failing: Arc<Mutex<HashSet<&'static str>>>,
}

impl CountingBackend {
    pub fn wrap(inner: Arc<dyn KanbanBackend>) -> WrappedBackend {
        let reads = Arc::new(AtomicUsize::new(0));
        let ops: ReadOpLog = Arc::new(Mutex::new(Vec::new()));
        let backend: Arc<dyn KanbanBackend> = Arc::new(Self {
            inner,
            reads: reads.clone(),
            ops: ops.clone(),
            failing: Arc::new(Mutex::new(HashSet::new())),
        });
        (backend, reads, ops)
    }

    /// Wraps `inner` so `method` returns an error whenever it is called.
    /// The call still appears in the op log.
    pub fn wrap_failing(
        inner: Arc<dyn KanbanBackend>,
        method: &'static str,
    ) -> Arc<dyn KanbanBackend> {
        let mut failing = HashSet::new();
        failing.insert(method);
        Arc::new(Self {
            inner,
            reads: Arc::new(AtomicUsize::new(0)),
            ops: Arc::new(Mutex::new(Vec::new())),
            failing: Arc::new(Mutex::new(failing)),
        })
    }

    fn record(&self, method: &'static str, ids: Vec<Uuid>) {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.ops.lock().unwrap().push(ReadOp { method, ids });
    }

    fn fault(&self, method: &'static str) -> KanbanResult<()> {
        if self.failing.lock().unwrap().contains(method) {
            return Err(KanbanError::Database(format!("injected fault: {method}")));
        }
        Ok(())
    }
}

impl DataStore for CountingBackend {
    fn get_prefix(&self, name: &str) -> KanbanResult<Option<kanban_domain::Prefix>> {
        self.record("get_prefix", vec![]);
        self.inner.get_prefix(name)
    }
    fn list_prefixes(&self) -> KanbanResult<Vec<kanban_domain::Prefix>> {
        self.record("list_prefixes", vec![]);
        self.inner.list_prefixes()
    }
    fn upsert_prefix(&self, prefix: kanban_domain::Prefix) -> KanbanResult<()> {
        self.inner.upsert_prefix(prefix)
    }
    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.record("get_board", vec![id]);
        self.inner.get_board(id)
    }
    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.record("list_boards", vec![]);
        self.fault("list_boards")?;
        self.inner.list_boards()
    }
    fn upsert_board(&self, board: Board) -> KanbanResult<()> {
        self.inner.upsert_board(board)
    }
    fn delete_board(&self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_board(id)
    }
    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.record("get_column", vec![id]);
        self.inner.get_column(id)
    }
    fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.record("list_columns_by_board", vec![board_id]);
        self.inner.list_columns_by_board(board_id)
    }
    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        self.record("list_all_columns", vec![]);
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
        self.record("get_card", vec![id]);
        self.inner.get_card(id)
    }
    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        self.record("list_all_cards", vec![]);
        self.inner.list_all_cards()
    }
    fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.record("list_cards_by_column", vec![column_id]);
        self.inner.list_cards_by_column(column_id)
    }
    fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.record("list_cards_by_sprint", vec![sprint_id]);
        self.inner.list_cards_by_sprint(sprint_id)
    }
    fn list_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<Vec<Card>> {
        self.record("list_cards_by_columns", column_ids.to_vec());
        self.inner.list_cards_by_columns(column_ids)
    }
    fn list_cards_by_column_filtered(
        &self,
        column_id: Uuid,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<Vec<Card>> {
        self.record("list_cards_by_column_filtered", vec![column_id]);
        self.inner
            .list_cards_by_column_filtered(column_id, archived)
    }
    fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
        self.record("count_cards_in_column", vec![column_id]);
        self.inner.count_cards_in_column(column_id)
    }
    fn count_cards_in_column_filtered(
        &self,
        column_id: Uuid,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<usize> {
        self.record("count_cards_in_column_filtered", vec![column_id]);
        self.inner
            .count_cards_in_column_filtered(column_id, archived)
    }
    fn count_cards_in_column_excluding(
        &self,
        column_id: Uuid,
        exclude_ids: &[Uuid],
    ) -> KanbanResult<usize> {
        self.record("count_cards_in_column_excluding", vec![column_id]);
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
    fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        self.record("get_archived_card", vec![card_id]);
        self.inner.get_archived_card(card_id)
    }
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.record("list_archived_cards", vec![]);
        self.inner.list_archived_cards()
    }
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        self.record("list_archived_cards_by_board", vec![board_id]);
        self.inner.list_archived_cards_by_board(board_id)
    }
    fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
        self.inner.insert_archived_card(ac)
    }
    fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
        self.inner.delete_archived_card(card_id)
    }
    fn clear_sprint_from_archived_cards(
        &self,
        sprint_id: Uuid,
        cleared_at: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        self.inner
            .clear_sprint_from_archived_cards(sprint_id, cleared_at)
    }
    fn get_archived_board(&self, board_id: Uuid) -> KanbanResult<Option<ArchivedBoard>> {
        self.record("get_archived_board", vec![board_id]);
        self.inner.get_archived_board(board_id)
    }
    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        self.record("list_archived_boards", vec![]);
        self.inner.list_archived_boards()
    }
    fn insert_archived_board(&self, ab: ArchivedBoard) -> KanbanResult<()> {
        self.inner.insert_archived_board(ab)
    }
    fn delete_archived_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.inner.delete_archived_board(board_id)
    }
    fn unarchive_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.inner.unarchive_board(board_id)
    }
    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.record("get_sprint", vec![id]);
        self.inner.get_sprint(id)
    }
    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.record("list_sprints_by_board", vec![board_id]);
        self.inner.list_sprints_by_board(board_id)
    }
    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        self.record("list_all_sprints", vec![]);
        self.inner.list_all_sprints()
    }
    fn upsert_sprint(&self, sprint: Sprint) -> KanbanResult<()> {
        self.inner.upsert_sprint(sprint)
    }
    fn delete_sprint(&self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_sprint(id)
    }
    fn delete_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.inner.delete_sprints_by_board(board_id)
    }
    fn get_graph(&self) -> KanbanResult<DependencyGraph> {
        self.record("get_graph", vec![]);
        self.inner.get_graph()
    }
    fn set_graph(&self, graph: DependencyGraph) -> KanbanResult<()> {
        self.inner.set_graph(graph)
    }
    fn modify_graph(&self, f: kanban_domain::GraphMutFn) -> KanbanResult<()> {
        self.record("modify_graph", vec![]);
        self.inner.modify_graph(f)
    }
    fn snapshot(&self) -> KanbanResult<Snapshot> {
        self.record("snapshot", vec![]);
        self.inner.snapshot()
    }
    fn apply_snapshot(&self, snapshot: Snapshot) -> KanbanResult<()> {
        self.inner.apply_snapshot(snapshot)
    }
}

impl CommandStore for CountingBackend {
    fn append_batch(&self, batch: &CommandBatch) -> KanbanResult<u64> {
        self.inner.append_batch(batch)
    }
    fn batch_count(&self) -> KanbanResult<u64> {
        self.record("batch_count", vec![]);
        self.inner.batch_count()
    }
    fn load_batches(&self, offset: u64, limit: u64) -> KanbanResult<Vec<CommandBatch>> {
        self.record("load_batches", vec![]);
        self.inner.load_batches(offset, limit)
    }
}

impl KanbanBackend for CountingBackend {
    fn as_data_store(&self) -> &dyn DataStore {
        self
    }

    fn remote_writes(&self) -> Option<&dyn RemoteWrites> {
        self.inner.remote_writes()
    }

    fn with_transaction(&self, f: TransactionFn<'_>) -> KanbanResult<()> {
        self.inner.with_transaction(f)
    }
}

/// A `KanbanBackend` decorator that counts only `DataStore::snapshot` calls
/// (what `App::reload_model` issues), delegating everything else verbatim to
/// `inner`. Unlike `CountingBackend`, this does not count the incidental
/// reads a command's own validation/execution performs, so it isolates "how
/// many whole-model reloads happened" from "how many store reads happened".
pub struct SnapshotCountingBackend {
    inner: Arc<dyn KanbanBackend>,
    snapshot_reads: Arc<AtomicUsize>,
}

impl SnapshotCountingBackend {
    pub fn wrap(inner: Arc<dyn KanbanBackend>) -> (Arc<dyn KanbanBackend>, Arc<AtomicUsize>) {
        let snapshot_reads = Arc::new(AtomicUsize::new(0));
        let backend: Arc<dyn KanbanBackend> = Arc::new(Self {
            inner,
            snapshot_reads: snapshot_reads.clone(),
        });
        (backend, snapshot_reads)
    }
}

impl DataStore for SnapshotCountingBackend {
    fn get_prefix(&self, name: &str) -> KanbanResult<Option<kanban_domain::Prefix>> {
        self.inner.get_prefix(name)
    }
    fn list_prefixes(&self) -> KanbanResult<Vec<kanban_domain::Prefix>> {
        self.inner.list_prefixes()
    }
    fn upsert_prefix(&self, prefix: kanban_domain::Prefix) -> KanbanResult<()> {
        self.inner.upsert_prefix(prefix)
    }

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
    fn list_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<Vec<Card>> {
        self.inner.list_cards_by_columns(column_ids)
    }
    fn list_cards_by_column_filtered(
        &self,
        column_id: Uuid,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<Vec<Card>> {
        self.inner
            .list_cards_by_column_filtered(column_id, archived)
    }
    fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
        self.inner.count_cards_in_column(column_id)
    }
    fn count_cards_in_column_filtered(
        &self,
        column_id: Uuid,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<usize> {
        self.inner
            .count_cards_in_column_filtered(column_id, archived)
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
    fn get_archived_card(&self, id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        self.inner.get_archived_card(id)
    }
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.inner.list_archived_cards()
    }
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        self.inner.list_archived_cards_by_board(board_id)
    }
    fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
        self.inner.insert_archived_card(ac)
    }
    fn delete_archived_card(&self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_archived_card(id)
    }
    fn clear_sprint_from_archived_cards(
        &self,
        sprint_id: Uuid,
        cleared_at: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        self.inner
            .clear_sprint_from_archived_cards(sprint_id, cleared_at)
    }
    fn get_archived_board(&self, id: Uuid) -> KanbanResult<Option<ArchivedBoard>> {
        self.inner.get_archived_board(id)
    }
    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        self.inner.list_archived_boards()
    }
    fn insert_archived_board(&self, ab: ArchivedBoard) -> KanbanResult<()> {
        self.inner.insert_archived_board(ab)
    }
    fn delete_archived_board(&self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_archived_board(id)
    }
    fn unarchive_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.inner.unarchive_board(board_id)
    }
    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.inner.get_sprint(id)
    }
    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.inner.list_sprints_by_board(board_id)
    }
    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        self.inner.list_all_sprints()
    }
    fn upsert_sprint(&self, sprint: Sprint) -> KanbanResult<()> {
        self.inner.upsert_sprint(sprint)
    }
    fn delete_sprint(&self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_sprint(id)
    }
    fn delete_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.inner.delete_sprints_by_board(board_id)
    }
    fn get_graph(&self) -> KanbanResult<DependencyGraph> {
        self.inner.get_graph()
    }
    fn set_graph(&self, graph: DependencyGraph) -> KanbanResult<()> {
        self.inner.set_graph(graph)
    }
    fn modify_graph(&self, f: kanban_domain::GraphMutFn) -> KanbanResult<()> {
        self.inner.modify_graph(f)
    }
    fn snapshot(&self) -> KanbanResult<Snapshot> {
        self.snapshot_reads.fetch_add(1, Ordering::SeqCst);
        self.inner.snapshot()
    }
    fn apply_snapshot(&self, snapshot: Snapshot) -> KanbanResult<()> {
        self.inner.apply_snapshot(snapshot)
    }
}

impl CommandStore for SnapshotCountingBackend {
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

impl KanbanBackend for SnapshotCountingBackend {
    fn as_data_store(&self) -> &dyn DataStore {
        self
    }

    fn remote_writes(&self) -> Option<&dyn RemoteWrites> {
        self.inner.remote_writes()
    }

    fn with_transaction(&self, f: TransactionFn<'_>) -> KanbanResult<()> {
        self.inner.with_transaction(f)
    }
}

/// A `KanbanBackend` decorator whose `snapshot()` always fails, delegating
/// every other `DataStore`/`CommandStore` method verbatim to `inner`. Used to
/// simulate a transient read failure (SQLite busy, I/O error) on the
/// destination backend right after a storage-location swap, while still
/// allowing direct entity reads against `inner` to prove the destination's
/// data survived.
pub struct FailingSnapshotBackend {
    inner: Arc<dyn KanbanBackend>,
}

impl FailingSnapshotBackend {
    pub fn wrap(inner: Arc<dyn KanbanBackend>) -> Arc<dyn KanbanBackend> {
        Arc::new(Self { inner })
    }
}

impl DataStore for FailingSnapshotBackend {
    fn get_prefix(&self, name: &str) -> KanbanResult<Option<kanban_domain::Prefix>> {
        self.inner.get_prefix(name)
    }
    fn list_prefixes(&self) -> KanbanResult<Vec<kanban_domain::Prefix>> {
        self.inner.list_prefixes()
    }
    fn upsert_prefix(&self, prefix: kanban_domain::Prefix) -> KanbanResult<()> {
        self.inner.upsert_prefix(prefix)
    }
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
    fn list_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<Vec<Card>> {
        self.inner.list_cards_by_columns(column_ids)
    }
    fn list_cards_by_column_filtered(
        &self,
        column_id: Uuid,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<Vec<Card>> {
        self.inner
            .list_cards_by_column_filtered(column_id, archived)
    }
    fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
        self.inner.count_cards_in_column(column_id)
    }
    fn count_cards_in_column_filtered(
        &self,
        column_id: Uuid,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<usize> {
        self.inner
            .count_cards_in_column_filtered(column_id, archived)
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
    fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        self.inner.get_archived_card(card_id)
    }
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.inner.list_archived_cards()
    }
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        self.inner.list_archived_cards_by_board(board_id)
    }
    fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
        self.inner.insert_archived_card(ac)
    }
    fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
        self.inner.delete_archived_card(card_id)
    }
    fn clear_sprint_from_archived_cards(
        &self,
        sprint_id: Uuid,
        cleared_at: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        self.inner
            .clear_sprint_from_archived_cards(sprint_id, cleared_at)
    }
    fn get_archived_board(&self, board_id: Uuid) -> KanbanResult<Option<ArchivedBoard>> {
        self.inner.get_archived_board(board_id)
    }
    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        self.inner.list_archived_boards()
    }
    fn insert_archived_board(&self, ab: ArchivedBoard) -> KanbanResult<()> {
        self.inner.insert_archived_board(ab)
    }
    fn delete_archived_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.inner.delete_archived_board(board_id)
    }
    fn unarchive_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.inner.unarchive_board(board_id)
    }
    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.inner.get_sprint(id)
    }
    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.inner.list_sprints_by_board(board_id)
    }
    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        self.inner.list_all_sprints()
    }
    fn upsert_sprint(&self, sprint: Sprint) -> KanbanResult<()> {
        self.inner.upsert_sprint(sprint)
    }
    fn delete_sprint(&self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_sprint(id)
    }
    fn delete_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<()> {
        self.inner.delete_sprints_by_board(board_id)
    }
    fn get_graph(&self) -> KanbanResult<DependencyGraph> {
        self.inner.get_graph()
    }
    fn set_graph(&self, graph: DependencyGraph) -> KanbanResult<()> {
        self.inner.set_graph(graph)
    }
    fn modify_graph(&self, f: kanban_domain::GraphMutFn) -> KanbanResult<()> {
        self.inner.modify_graph(f)
    }
    fn snapshot(&self) -> KanbanResult<Snapshot> {
        Err(kanban_domain::KanbanError::Database(
            "simulated transient read failure".to_string(),
        ))
    }
    fn apply_snapshot(&self, snapshot: Snapshot) -> KanbanResult<()> {
        self.inner.apply_snapshot(snapshot)
    }
}

impl CommandStore for FailingSnapshotBackend {
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

impl KanbanBackend for FailingSnapshotBackend {
    fn as_data_store(&self) -> &dyn DataStore {
        self
    }

    fn remote_writes(&self) -> Option<&dyn RemoteWrites> {
        self.inner.remote_writes()
    }

    fn with_transaction(&self, f: TransactionFn<'_>) -> KanbanResult<()> {
        self.inner.with_transaction(f)
    }
}

/// A `KanbanBackendFactory` that always hands back the same pre-built
/// backend for one fixed locator string, regardless of `config`. Lets a test
/// inject an arbitrary `KanbanBackend` (e.g. `FailingSnapshotBackend`) into
/// `App::handle_migration_complete`'s real `store_manager.make_backend` call,
/// which otherwise only ever constructs real json/sqlite backends.
pub struct FixedLocatorBackendFactory {
    pub locator: String,
    pub backend: Arc<dyn KanbanBackend>,
}

#[async_trait::async_trait]
impl kanban_backend::KanbanBackendFactory for FixedLocatorBackendFactory {
    fn name(&self) -> &str {
        "fixed-locator-test-double"
    }

    fn matches_locator(&self, locator: &str, _header: &[u8]) -> bool {
        locator == self.locator
    }

    async fn create(
        &self,
        _locator: &str,
        _config: &kanban_core::AppConfig,
    ) -> KanbanResult<Arc<dyn KanbanBackend>> {
        Ok(self.backend.clone())
    }
}

/// Builds a `StoreManager` whose only registered backend factory is
/// `FixedLocatorBackendFactory` for `locator`, returning `backend` whenever
/// `make_backend` is called with that exact locator.
pub fn store_manager_with_fixed_backend(
    locator: String,
    backend: Arc<dyn KanbanBackend>,
) -> kanban_service::StoreManager {
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    backends.register(Box::new(FixedLocatorBackendFactory { locator, backend }));
    kanban_service::StoreManager::new(kanban_persistence::StoreRegistry::new(), backends)
}

pub fn render_widget_to_string<F>(width: u16, height: u16, draw_fn: F) -> String
where
    F: FnOnce(&mut ratatui::Frame),
{
    use ratatui::{backend::TestBackend, Terminal};
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(draw_fn).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let mut result = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            result.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        result.push('\n');
    }
    result
}

pub fn render_to_string(app: &App) -> String {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render_settings_view(app, frame, frame.area());
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let mut result = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            result.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        result.push('\n');
    }
    result
}

pub fn render_to_string_with_colors(app: &App) -> Vec<(String, Option<ratatui::style::Color>)> {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render_settings_view(app, frame, frame.area());
        })
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    let mut result = Vec::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = buffer.cell((x, y)).unwrap();
            result.push((cell.symbol().to_string(), cell.style().fg));
        }
    }
    result
}

pub fn setup_settings_app() -> App {
    let mut app = App::test_default();
    app.focus.active = Focus::Boards;
    app.handle_open_settings();
    app
}

pub fn setup_app_with_export_dialog(board_count: usize) -> App {
    use kanban_domain::KanbanOperations;
    let mut app = App::test_default();
    app.focus.active = Focus::Boards;
    app.push_mode(AppMode::Settings);
    let board_ids: Vec<uuid::Uuid> = (0..board_count)
        .map(|i| {
            app.ctx
                .create_board(format!("Board{}", i + 1), None)
                .unwrap()
                .id
        })
        .collect();
    app.export_dialog = Some(ExportDialogState::new(board_ids));
    app.push_mode(AppMode::Dialog(DialogMode::ExportBoards));
    app
}

pub async fn create_test_json_file(dir: &std::path::Path, name: &str, boards: &[&str]) -> String {
    use kanban_persistence::{PersistenceMetadata, PersistenceStore, StoreSnapshot};

    let path = dir.join(name);
    let path_str = path.to_str().unwrap().to_string();
    let store = kanban_persistence_json::JsonFileStore::new(&path_str);

    let domain_boards: Vec<kanban_domain::Board> = boards
        .iter()
        .map(|n| kanban_domain::Board::new(n.to_string(), None::<String>))
        .collect();
    let snapshot = kanban_domain::Snapshot {
        archived_boards: Vec::new(),
        boards: domain_boards,
        columns: vec![],
        cards: vec![],
        archived_cards: vec![],
        sprints: vec![],
        graph: Default::default(),
        prefixes: Vec::new(),
    };

    let store_snapshot = StoreSnapshot {
        data: serde_json::to_vec(&snapshot).unwrap(),
        metadata: PersistenceMetadata::new(store.instance_id()),
    };
    store.save(store_snapshot).await.unwrap();

    path_str
}

pub async fn create_test_sqlite_file(dir: &std::path::Path, name: &str, boards: &[&str]) -> String {
    use kanban_domain::DataStore;

    let path = dir.join(name);
    let path_str = path.to_str().unwrap().to_string();
    let store = kanban_persistence_sqlite::SqliteStore::open(&path_str)
        .await
        .unwrap();

    let domain_boards: Vec<kanban_domain::Board> = boards
        .iter()
        .map(|n| kanban_domain::Board::new(n.to_string(), None::<String>))
        .collect();
    let snapshot = kanban_domain::Snapshot {
        archived_boards: Vec::new(),
        boards: domain_boards,
        columns: vec![],
        cards: vec![],
        archived_cards: vec![],
        sprints: vec![],
        graph: Default::default(),
        prefixes: Vec::new(),
    };
    store.apply_snapshot(snapshot).unwrap();

    path_str
}

pub async fn setup_app_with_json_file(dir: &std::path::Path) -> App {
    let path = create_test_json_file(dir, "source.json", &["OriginalBoard"]).await;
    let (mut app, _rx) = App::new(Some(path)).await.unwrap();
    app.load_initial_state().await;
    app
}

pub async fn setup_app_with_json_file_and_save_worker(dir: &std::path::Path) -> App {
    let path = create_test_json_file(dir, "source.json", &["OriginalBoard"]).await;
    let (mut app, save_rx) = App::new(Some(path)).await.unwrap();
    app.load_initial_state().await;
    app.spawn_save_worker(
        save_rx.expect("App::new should hand back a save receiver"),
        None,
    );
    app
}
