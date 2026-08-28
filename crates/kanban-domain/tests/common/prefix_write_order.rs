use kanban_backend_memory::InMemoryStore;
use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, DataStore, DependencyGraph, KanbanResult,
    Prefix, Snapshot, Sprint,
};
use std::sync::Mutex;
use uuid::Uuid;

/// Records any card written while the namespace its prefix names has no row.
pub struct PrefixWriteOrderStore {
    inner: InMemoryStore,
    unbacked_at_write: Mutex<Vec<(u32, String)>>,
    prefix_upsert_names: Mutex<Vec<String>>,
    swallow_prefix_writes: bool,
}

impl PrefixWriteOrderStore {
    pub fn new() -> Self {
        Self {
            inner: InMemoryStore::default(),
            unbacked_at_write: Mutex::new(Vec::new()),
            prefix_upsert_names: Mutex::new(Vec::new()),
            swallow_prefix_writes: false,
        }
    }

    pub fn with_prefix_writes_swallowed() -> Self {
        Self {
            inner: InMemoryStore::default(),
            unbacked_at_write: Mutex::new(Vec::new()),
            prefix_upsert_names: Mutex::new(Vec::new()),
            swallow_prefix_writes: true,
        }
    }

    pub fn unbacked_at_write(&self) -> Vec<(u32, String)> {
        self.unbacked_at_write.lock().unwrap().clone()
    }

    /// The exact `Prefix::name` string passed to each `upsert_prefix` call,
    /// in call order, before any storage-layer normalisation runs.
    pub fn prefix_upsert_names(&self) -> Vec<String> {
        self.prefix_upsert_names.lock().unwrap().clone()
    }
}

impl Default for PrefixWriteOrderStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DataStore for PrefixWriteOrderStore {
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

    fn get_prefix(&self, name: &str) -> KanbanResult<Option<Prefix>> {
        self.inner.get_prefix(name)
    }
    fn list_prefixes(&self) -> KanbanResult<Vec<Prefix>> {
        self.inner.list_prefixes()
    }
    fn upsert_prefix(&self, prefix: Prefix) -> KanbanResult<()> {
        self.prefix_upsert_names
            .lock()
            .unwrap()
            .push(prefix.name.clone());
        if self.swallow_prefix_writes {
            return Ok(());
        }
        self.inner.upsert_prefix(prefix)
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
        exclude: &[Uuid],
    ) -> KanbanResult<usize> {
        self.inner
            .count_cards_in_column_excluding(column_id, exclude)
    }
    fn upsert_card(&self, card: Card) -> KanbanResult<()> {
        if !card.prefix.is_empty() {
            let normalized = Prefix::normalize(&card.prefix);
            if self.inner.get_prefix(&normalized)?.is_none() {
                self.unbacked_at_write
                    .lock()
                    .unwrap()
                    .push((card.card_number, card.prefix.clone()));
            }
        }
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
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        self.inner.clear_sprint_from_cards(sprint_id, timestamp)
    }

    fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        self.inner.get_archived_card(card_id)
    }
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.inner.list_archived_cards()
    }
    fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
        self.inner.insert_archived_card(ac)
    }
    fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
        self.inner.delete_archived_card(card_id)
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

    fn snapshot(&self) -> KanbanResult<Snapshot> {
        self.inner.snapshot()
    }
    fn apply_snapshot(&self, snapshot: Snapshot) -> KanbanResult<()> {
        self.inner.apply_snapshot(snapshot)
    }
}
