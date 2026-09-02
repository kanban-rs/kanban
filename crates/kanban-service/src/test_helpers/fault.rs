use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kanban_domain::command_batch::CommandBatch;
use kanban_domain::command_store::CommandStore;
use kanban_domain::data_store::{DataStore, GraphMutFn};
use kanban_domain::{
    ArchivedBoard, ArchivedCard, ArchivedFilter, Board, Card, Column, DependencyGraph, KanbanError,
    KanbanResult, Prefix, Snapshot, Sprint,
};
use uuid::Uuid;

use super::BackendFactory;
use crate::KanbanBackend;

/// One intercepted read: the `DataStore` method name and the ids it was called
/// with (empty for whole-collection reads, the scope key for scoped reads).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOp {
    pub method: &'static str,
    pub ids: Vec<Uuid>,
}

/// The read methods this wrapper intercepts: these are the ones `fail()` can
/// fault AND the ones that appear in `ops()`. A `fail()` call naming anything
/// outside this list panics rather than silently doing nothing - a test that
/// thinks it injected a fault and did not is a test that asserts nothing.
/// A later card that needs to fault a read not listed here must WIDEN this
/// const; it cannot fault it by name alone.
pub const FAULTABLE_READS: &[&str] = &[
    "list_boards",
    "get_board",
    "list_all_columns",
    "list_columns_by_board",
    "get_column",
    "list_all_cards",
    "list_cards_by_column",
    "get_card",
    "list_all_sprints",
    "list_sprints_by_board",
    "get_sprint",
    "get_graph",
    "list_archived_cards",
    "list_archived_cards_by_board",
    "list_archived_boards",
    "list_prefixes",
];

/// Wraps a real backend and (a) makes named `DataStore` reads return an error
/// on demand, and (b) records every intercepted read in order, so a test can
/// prove a read *failure* is distinguished from a read that legitimately found
/// nothing, AND that a read it expected NOT to happen did not happen - on every
/// backend, not just in-memory.
///
/// Every method delegates to `inner`. Only the reads named in
/// `FAULTABLE_READS` are intercepted; writes are never faulted and never
/// recorded, because a half-applied write would leave the wrapped backend in a
/// state the test did not ask for.
pub struct FaultInjectingBackend {
    inner: Arc<dyn KanbanBackend>,
    failing: Mutex<HashSet<&'static str>>,
    ops: Mutex<Vec<ReadOp>>,
}

impl FaultInjectingBackend {
    pub fn new(inner: Arc<dyn KanbanBackend>) -> Self {
        Self {
            inner,
            failing: Mutex::new(HashSet::new()),
            ops: Mutex::new(Vec::new()),
        }
    }

    /// The wrapped backend, for assertions about real state.
    pub fn inner(&self) -> &Arc<dyn KanbanBackend> {
        &self.inner
    }

    /// Make `method` return an error until cleared. `method` must be one of the
    /// names in [`FAULTABLE_READS`]; anything else panics.
    pub fn fail(&self, method: &'static str) {
        assert!(
            FAULTABLE_READS.contains(&method),
            "{method} is not a faultable read; widen FAULTABLE_READS to fault it"
        );
        self.failing.lock().unwrap().insert(method);
    }

    /// Make `reload` return an error until cleared. Separate from [`fail`](Self::fail)
    /// because `FAULTABLE_READS` scopes `DataStore` reads, not lifecycle calls
    /// like `KanbanBackend::reload`.
    pub fn fail_reload(&self) {
        self.failing.lock().unwrap().insert("reload");
    }

    pub fn clear_faults(&self) {
        self.failing.lock().unwrap().clear();
    }

    /// Every intercepted read since construction or the last `clear_ops`, in
    /// call order. Returns a clone so the caller holds no lock while asserting.
    ///
    /// A read is recorded whether it succeeded OR was faulted: the log answers
    /// "was this method called", not "did it return data". A test asserting a
    /// read did not happen wants the call to be absent, not merely failed.
    pub fn ops(&self) -> Vec<ReadOp> {
        self.ops.lock().unwrap().clone()
    }

    /// Reset the log. Tests that resolve once to warm the `Model` and then
    /// assert on a SECOND resolve call this in between; without it every
    /// assertion has to skip a prefix, which is how an off-by-one in an op
    /// assertion hides.
    pub fn clear_ops(&self) {
        self.ops.lock().unwrap().clear();
    }

    pub fn op_count(&self, method: &str) -> usize {
        self.ops
            .lock()
            .unwrap()
            .iter()
            .filter(|o| o.method == method)
            .count()
    }

    /// Records first, then faults, so a faulted read still appears in the log.
    fn check(&self, method: &'static str, ids: Vec<Uuid>) -> KanbanResult<()> {
        self.ops.lock().unwrap().push(ReadOp { method, ids });
        if self.failing.lock().unwrap().contains(method) {
            return Err(KanbanError::Database(format!("injected fault: {method}")));
        }
        Ok(())
    }
}

/// Path -> every wrapper produced for that path, in construction order.
///
/// A durable backend is expected to hand back a *fresh* store on each open of
/// the same path, sharing on-disk state; the reload assertions in the contract
/// suite depend on it. So this records one entry per call rather than one per
/// path, and the wrapper the context is currently using is the LAST element.
pub type FaultHandles = Arc<Mutex<HashMap<PathBuf, Vec<Arc<FaultInjectingBackend>>>>>;

/// Wrap a `BackendFactory` so every backend it produces is fault-injectable.
/// The handles are keyed by path so a test can reach the wrapper the context is
/// using and flip a fault on it mid-test.
///
/// This calls `inner` on EVERY invocation and never reuses a previously built
/// backend. Caching by path would turn "reopen and re-read from disk" into
/// "hand back the same in-memory instance", silently defeating the reload
/// assertions this helper exists to serve.
pub fn faultable(inner: BackendFactory) -> (BackendFactory, FaultHandles) {
    let handles: FaultHandles = Arc::new(Mutex::new(HashMap::new()));
    let for_factory = Arc::clone(&handles);
    let factory: BackendFactory = Box::new(move |path: &std::path::Path| {
        let wrapper = Arc::new(FaultInjectingBackend::new(inner(path)));
        for_factory
            .lock()
            .unwrap()
            .entry(path.to_path_buf())
            .or_default()
            .push(Arc::clone(&wrapper));
        wrapper as Arc<dyn KanbanBackend>
    });
    (factory, handles)
}

impl DataStore for FaultInjectingBackend {
    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.check("get_board", vec![id])?;
        self.inner.get_board(id)
    }
    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.check("list_boards", vec![])?;
        self.inner.list_boards()
    }
    fn upsert_board(&self, board: Board) -> KanbanResult<()> {
        self.inner.upsert_board(board)
    }
    fn delete_board(&self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_board(id)
    }

    fn get_prefix(&self, name: &str) -> KanbanResult<Option<Prefix>> {
        self.inner.get_prefix(name)
    }
    fn list_prefixes(&self) -> KanbanResult<Vec<Prefix>> {
        self.check("list_prefixes", vec![])?;
        self.inner.list_prefixes()
    }
    fn upsert_prefix(&self, prefix: Prefix) -> KanbanResult<()> {
        self.inner.upsert_prefix(prefix)
    }

    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.check("get_column", vec![id])?;
        self.inner.get_column(id)
    }
    fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.check("list_columns_by_board", vec![board_id])?;
        self.inner.list_columns_by_board(board_id)
    }
    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        self.check("list_all_columns", vec![])?;
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
        self.check("get_card", vec![id])?;
        self.inner.get_card(id)
    }
    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        self.check("list_all_cards", vec![])?;
        self.inner.list_all_cards()
    }
    fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.check("list_cards_by_column", vec![column_id])?;
        self.inner.list_cards_by_column(column_id)
    }
    fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.inner.list_cards_by_sprint(sprint_id)
    }
    fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
        self.inner.count_cards_in_column(column_id)
    }
    fn list_cards_by_prefix_and_number(
        &self,
        prefix: &str,
        card_number: u32,
    ) -> KanbanResult<Vec<Card>> {
        self.inner
            .list_cards_by_prefix_and_number(prefix, card_number)
    }
    fn list_cards_by_number(&self, card_number: u32) -> KanbanResult<Vec<Card>> {
        self.inner.list_cards_by_number(card_number)
    }
    fn get_card_by_board_and_number(
        &self,
        board_id: Uuid,
        card_number: u32,
    ) -> KanbanResult<Option<Card>> {
        self.inner
            .get_card_by_board_and_number(board_id, card_number)
    }
    fn get_card_by_sprint_and_number(
        &self,
        sprint_id: Uuid,
        card_number: u32,
    ) -> KanbanResult<Option<Card>> {
        self.inner
            .get_card_by_sprint_and_number(sprint_id, card_number)
    }
    fn count_cards_in_column_excluding(
        &self,
        column_id: Uuid,
        exclude: &[Uuid],
    ) -> KanbanResult<usize> {
        self.inner
            .count_cards_in_column_excluding(column_id, exclude)
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
    fn list_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<Vec<Card>> {
        self.inner.list_cards_by_columns(column_ids)
    }
    fn list_cards_by_column_filtered(
        &self,
        column_id: Uuid,
        archived: ArchivedFilter,
    ) -> KanbanResult<Vec<Card>> {
        self.inner
            .list_cards_by_column_filtered(column_id, archived)
    }
    fn count_cards_in_column_filtered(
        &self,
        column_id: Uuid,
        archived: ArchivedFilter,
    ) -> KanbanResult<usize> {
        self.inner
            .count_cards_in_column_filtered(column_id, archived)
    }
    fn clear_sprint_from_cards(
        &self,
        sprint_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        self.inner.clear_sprint_from_cards(sprint_id, timestamp)
    }

    fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        self.inner.get_archived_card(card_id)
    }
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.check("list_archived_cards", vec![])?;
        self.inner.list_archived_cards()
    }
    fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
        self.inner.insert_archived_card(ac)
    }
    fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
        self.inner.delete_archived_card(card_id)
    }
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        self.check("list_archived_cards_by_board", vec![board_id])?;
        self.inner.list_archived_cards_by_board(board_id)
    }
    fn clear_sprint_from_archived_cards(
        &self,
        sprint_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        self.inner
            .clear_sprint_from_archived_cards(sprint_id, timestamp)
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
        self.check("get_sprint", vec![id])?;
        self.inner.get_sprint(id)
    }
    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.check("list_sprints_by_board", vec![board_id])?;
        self.inner.list_sprints_by_board(board_id)
    }
    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        self.check("list_all_sprints", vec![])?;
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
        self.check("get_graph", vec![])?;
        self.inner.get_graph()
    }
    fn set_graph(&self, graph: DependencyGraph) -> KanbanResult<()> {
        self.inner.set_graph(graph)
    }
    fn modify_graph(&self, f: GraphMutFn) -> KanbanResult<()> {
        self.inner.modify_graph(f)
    }

    fn snapshot(&self) -> KanbanResult<Snapshot> {
        self.inner.snapshot()
    }
    fn apply_snapshot(&self, snapshot: Snapshot) -> KanbanResult<()> {
        self.inner.apply_snapshot(snapshot)
    }
}

impl CommandStore for FaultInjectingBackend {
    fn append_batch(&self, batch: &CommandBatch) -> KanbanResult<u64> {
        self.inner.append_batch(batch)
    }
    fn batch_count(&self) -> KanbanResult<u64> {
        self.inner.batch_count()
    }
    fn load_batches(&self, from: u64, to: u64) -> KanbanResult<Vec<CommandBatch>> {
        self.inner.load_batches(from, to)
    }
    fn load_all_batches(&self) -> KanbanResult<(Vec<CommandBatch>, u64)> {
        self.inner.load_all_batches()
    }
}

#[async_trait]
impl KanbanBackend for FaultInjectingBackend {
    fn as_data_store(&self) -> &dyn DataStore {
        self
    }
    async fn flush(&self) -> KanbanResult<()> {
        self.inner.flush().await
    }
    async fn reload(&self) -> KanbanResult<()> {
        if self.failing.lock().unwrap().contains("reload") {
            return Err(KanbanError::Database("injected fault: reload".into()));
        }
        self.inner.reload().await
    }
    fn needs_flush(&self) -> bool {
        self.inner.needs_flush()
    }
    fn needs_save_worker(&self) -> bool {
        self.inner.needs_save_worker()
    }
    fn instance_id(&self) -> Uuid {
        self.inner.instance_id()
    }
    fn local_persistence(&self) -> Option<&dyn kanban_backend::LocalPersistence> {
        self.inner.local_persistence()
    }
    fn health_checker(&self) -> Option<Box<dyn kanban_core::HealthChecker>> {
        self.inner.health_checker()
    }
    fn remote_writes(&self) -> Option<&dyn kanban_backend::RemoteWrites> {
        self.inner.remote_writes()
    }
    fn with_transaction(&self, f: kanban_backend::TransactionFn<'_>) -> KanbanResult<()> {
        self.inner.with_transaction(f)
    }
}
