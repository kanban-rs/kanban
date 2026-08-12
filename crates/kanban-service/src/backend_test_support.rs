use kanban_backend::{KanbanBackend, RemoteWrites, TransactionFn};
use kanban_backend_memory::InMemoryStore;
use kanban_domain::{
    Board, BoardUpdate, Card, CardUpdate, Column, ColumnUpdate, CommandBatch, CommandStore,
    DataStore, KanbanResult, NewBoard, NewCard, NewColumn, Snapshot,
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
    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<kanban_domain::Sprint>> {
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

    fn with_transaction(&self, f: TransactionFn<'_>) -> KanbanResult<()> {
        self.inner.with_transaction(f)
    }
}
