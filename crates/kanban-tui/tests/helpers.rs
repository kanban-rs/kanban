#![allow(dead_code)]

use kanban_backend::{KanbanBackend, RemoteWrites, TransactionFn};
use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, CommandBatch, CommandStore, DataStore,
    DependencyGraph, KanbanResult, Snapshot, Sprint,
};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::app::ExportDialogState;
use kanban_tui::App;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use uuid::Uuid;

/// A `KanbanBackend` decorator that counts every DataStore/CommandStore READ
/// method invoked, delegating all reads and writes verbatim to `inner`.
/// Writes are never counted. Only required trait methods are overridden;
/// default trait methods route through the instrumented required ones.
pub struct CountingBackend {
    inner: Arc<dyn KanbanBackend>,
    reads: Arc<AtomicUsize>,
}

impl CountingBackend {
    pub fn wrap(inner: Arc<dyn KanbanBackend>) -> (Arc<dyn KanbanBackend>, Arc<AtomicUsize>) {
        let reads = Arc::new(AtomicUsize::new(0));
        let backend: Arc<dyn KanbanBackend> = Arc::new(Self {
            inner,
            reads: reads.clone(),
        });
        (backend, reads)
    }

    fn record(&self) {
        self.reads.fetch_add(1, Ordering::SeqCst);
    }
}

impl DataStore for CountingBackend {
    fn get_prefix(&self, name: &str) -> KanbanResult<Option<kanban_domain::Prefix>> {
        self.record();
        self.inner.get_prefix(name)
    }
    fn list_prefixes(&self) -> KanbanResult<Vec<kanban_domain::Prefix>> {
        self.record();
        self.inner.list_prefixes()
    }
    fn upsert_prefix(&self, prefix: kanban_domain::Prefix) -> KanbanResult<()> {
        self.inner.upsert_prefix(prefix)
    }
    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.record();
        self.inner.get_board(id)
    }
    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.record();
        self.inner.list_boards()
    }
    fn upsert_board(&self, board: Board) -> KanbanResult<()> {
        self.inner.upsert_board(board)
    }
    fn delete_board(&self, id: Uuid) -> KanbanResult<()> {
        self.inner.delete_board(id)
    }
    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.record();
        self.inner.get_column(id)
    }
    fn list_columns_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.record();
        self.inner.list_columns_by_board(board_id)
    }
    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        self.record();
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
        self.record();
        self.inner.get_card(id)
    }
    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        self.record();
        self.inner.list_all_cards()
    }
    fn list_cards_by_column(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.record();
        self.inner.list_cards_by_column(column_id)
    }
    fn list_cards_by_sprint(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        self.record();
        self.inner.list_cards_by_sprint(sprint_id)
    }
    fn list_cards_by_columns(&self, column_ids: &[Uuid]) -> KanbanResult<Vec<Card>> {
        self.record();
        self.inner.list_cards_by_columns(column_ids)
    }
    fn list_cards_by_column_filtered(
        &self,
        column_id: Uuid,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<Vec<Card>> {
        self.record();
        self.inner
            .list_cards_by_column_filtered(column_id, archived)
    }
    fn count_cards_in_column(&self, column_id: Uuid) -> KanbanResult<usize> {
        self.record();
        self.inner.count_cards_in_column(column_id)
    }
    fn count_cards_in_column_filtered(
        &self,
        column_id: Uuid,
        archived: kanban_domain::ArchivedFilter,
    ) -> KanbanResult<usize> {
        self.record();
        self.inner
            .count_cards_in_column_filtered(column_id, archived)
    }
    fn count_cards_in_column_excluding(
        &self,
        column_id: Uuid,
        exclude_ids: &[Uuid],
    ) -> KanbanResult<usize> {
        self.record();
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
        self.record();
        self.inner.get_archived_card(card_id)
    }
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.record();
        self.inner.list_archived_cards()
    }
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        self.record();
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
        self.record();
        self.inner.get_archived_board(board_id)
    }
    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        self.record();
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
        self.record();
        self.inner.get_sprint(id)
    }
    fn list_sprints_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.record();
        self.inner.list_sprints_by_board(board_id)
    }
    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        self.record();
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
        self.record();
        self.inner.get_graph()
    }
    fn set_graph(&self, graph: DependencyGraph) -> KanbanResult<()> {
        self.inner.set_graph(graph)
    }
    fn modify_graph(&self, f: kanban_domain::GraphMutFn) -> KanbanResult<()> {
        self.record();
        self.inner.modify_graph(f)
    }
    fn snapshot(&self) -> KanbanResult<Snapshot> {
        self.record();
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
        self.record();
        self.inner.batch_count()
    }
    fn load_batches(&self, offset: u64, limit: u64) -> KanbanResult<Vec<CommandBatch>> {
        self.record();
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
