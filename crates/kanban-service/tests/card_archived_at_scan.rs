use async_trait::async_trait;
use kanban_backend::{KanbanBackend, TransactionFn};
use kanban_backend_memory::InMemoryStore;
use kanban_domain::command_store::CommandStore;
use kanban_domain::data_store::DataStore;
use kanban_domain::{
    ArchivedCard, Board, Card, Column, CommandBatch, DependencyGraph, KanbanOperations,
    KanbanResult, Snapshot, Sprint,
};
use kanban_service::{AppConfig, KanbanContext};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Default)]
struct CountingBackend {
    inner: InMemoryStore,
    list_archived_cards_calls: AtomicUsize,
}

impl CountingBackend {
    fn list_archived_cards_call_count(&self) -> usize {
        self.list_archived_cards_calls.load(Ordering::SeqCst)
    }
}

impl DataStore for CountingBackend {
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
    fn count_cards_in_column_filtered(
        &self,
        column_id: Uuid,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<usize> {
        self.inner
            .count_cards_in_column_filtered(column_id, archived)
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
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        self.inner.clear_sprint_from_cards(sprint_id, timestamp)
    }
    fn get_archived_card(&self, card_id: Uuid) -> KanbanResult<Option<ArchivedCard>> {
        self.inner.get_archived_card(card_id)
    }
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.list_archived_cards_calls
            .fetch_add(1, Ordering::SeqCst);
        self.inner.list_archived_cards()
    }
    fn insert_archived_card(&self, ac: ArchivedCard) -> KanbanResult<()> {
        self.inner.insert_archived_card(ac)
    }
    fn delete_archived_card(&self, card_id: Uuid) -> KanbanResult<()> {
        self.inner.delete_archived_card(card_id)
    }
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
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

impl CommandStore for CountingBackend {
    fn append_batch(&self, batch: &CommandBatch) -> KanbanResult<u64> {
        self.inner.append_batch(batch)
    }
    fn batch_count(&self) -> KanbanResult<u64> {
        self.inner.batch_count()
    }
    fn load_batches(&self, from: u64, to: u64) -> KanbanResult<Vec<CommandBatch>> {
        self.inner.load_batches(from, to)
    }
}

#[async_trait]
impl KanbanBackend for CountingBackend {
    fn as_data_store(&self) -> &dyn DataStore {
        self
    }
    fn with_transaction(&self, f: TransactionFn<'_>) -> KanbanResult<()> {
        self.inner.with_transaction(f)
    }
}

fn counting_context() -> (Arc<CountingBackend>, KanbanContext) {
    let backend = Arc::new(CountingBackend::default());
    let ctx = KanbanContext::open_deferred(backend.clone(), AppConfig::default());
    (backend, ctx)
}

#[test]
fn test_card_get_by_id_zero_list_archived_cards_calls() {
    let (backend, mut ctx) = counting_context();
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let card = ctx
        .create_card_from_spec(
            None,
            kanban_domain::NewCard {
                column_id: col.id,
                title: "Card".into(),
                description: None,
                priority: kanban_domain::CardPriority::Medium,
                due_date: None,
                points: None,
                sprint_id: None,
            },
        )
        .unwrap();
    ctx.archive_card(card.id).unwrap();

    backend.list_archived_cards_calls.store(0, Ordering::SeqCst);

    let archived_at = ctx.card_archived_at(card.id).unwrap();

    assert!(archived_at.is_some());
    assert_eq!(
        backend.list_archived_cards_call_count(),
        0,
        "card_archived_at should perform a by-id lookup, not a full scan"
    );
}

fn make_card(ctx: &mut KanbanContext, col_id: Uuid, title: &str) -> Card {
    ctx.create_card_from_spec(
        None,
        kanban_domain::NewCard {
            column_id: col_id,
            title: title.to_string(),
            description: None,
            priority: kanban_domain::CardPriority::Medium,
            due_date: None,
            points: None,
            sprint_id: None,
        },
    )
    .unwrap()
}

#[test]
fn test_card_get_by_id_still_stamps_archived_at_for_archived_card() {
    let (_backend, mut ctx) = counting_context();
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let card = make_card(&mut ctx, col.id, "Card");
    ctx.archive_card(card.id).unwrap();

    let archived_at = ctx.card_archived_at(card.id).unwrap();

    assert!(archived_at.is_some());
}

#[test]
fn test_card_get_by_id_leaves_live_card_archived_at_none() {
    let (_backend, mut ctx) = counting_context();
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let card = make_card(&mut ctx, col.id, "Card");

    let archived_at = ctx.card_archived_at(card.id).unwrap();

    assert_eq!(archived_at, None);
}

#[test]
fn test_filter_cards_still_uses_archived_card_index() {
    let (backend, mut ctx) = counting_context();
    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let card_a = make_card(&mut ctx, col.id, "A");
    let card_b = make_card(&mut ctx, col.id, "B");
    ctx.archive_card(card_a.id).unwrap();
    ctx.archive_card(card_b.id).unwrap();

    backend.list_archived_cards_calls.store(0, Ordering::SeqCst);

    let cards = ctx
        .list_cards(kanban_domain::CardListFilter {
            board_id: Some(board.id),
            archived: kanban_domain::ArchivedFilter::Include,
            ..Default::default()
        })
        .unwrap();

    assert_eq!(cards.len(), 2);
    assert_eq!(
        backend.list_archived_cards_call_count(),
        2,
        "list_cards's collection-shaped path keeps building the archived index the same way it did before this change"
    );
}
