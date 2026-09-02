use super::types::default_store_manager;
use super::*;
use kanban_core::InputState;
use kanban_domain::Model;
use kanban_view::board_list::BoardList;
use std::sync::{Arc, Mutex};

/// The save worker must NOT send a completion signal when `backend.flush()`
/// returns `ConflictDetected`. Sending it on conflict decrements
/// `pending_saves` to 0, causing the Layer-2 TUI guard to lower its shield
/// while data is still dirty — leaving the board in an inconsistent state.
#[tokio::test(flavor = "multi_thread")]
async fn test_save_worker_does_not_send_completion_on_conflict() {
    use async_trait::async_trait;
    use kanban_domain::DataStore as _;
    use kanban_persistence::{
        PersistenceError, PersistenceMetadata, PersistenceResult, PersistenceStore, StoreSnapshot,
    };
    use kanban_persistence_json::JsonDataStore;
    use std::path::Path;

    struct ConflictingStore;

    #[async_trait]
    impl PersistenceStore for ConflictingStore {
        async fn save(&self, _: StoreSnapshot) -> PersistenceResult<PersistenceMetadata> {
            Err(PersistenceError::ConflictDetected {
                path: "conflict.json".into(),
                source: None,
            })
        }
        async fn load(&self) -> PersistenceResult<(StoreSnapshot, PersistenceMetadata)> {
            Err(PersistenceError::Serialization("noop".into()))
        }
        async fn exists(&self) -> bool {
            false
        }
        fn path(&self) -> &Path {
            Path::new("conflict.json")
        }
        fn instance_id(&self) -> uuid::Uuid {
            uuid::Uuid::nil()
        }
        fn load_sync(&self) -> PersistenceResult<Option<(StoreSnapshot, PersistenceMetadata)>> {
            Ok(None)
        }
    }

    let backend = Arc::new(JsonDataStore::new(Arc::new(ConflictingStore)));
    backend
        .upsert_board(kanban_domain::Board::new("B", None::<String>))
        .unwrap();

    let inner = kanban_service::KanbanContext::open_deferred(
        Arc::clone(&backend) as Arc<dyn kanban_service::backend::KanbanBackend>,
        kanban_core::AppConfig::default(),
    );

    let (ctx, save_rx, save_completion_rx) =
        crate::tui_context::TuiContext::new(inner).expect("TuiContext::new failed");
    let save_rx = save_rx.expect("JsonDataStore must need a save worker");

    let mut app = App {
        store_manager: Arc::new(default_store_manager()),
        should_quit: false,
        quit_with_pending: false,
        quit_with_migration: false,
        mode: AppMode::Normal,
        mode_stack: Vec::new(),
        input: InputState::new(),
        ctx,
        app_config: kanban_core::AppConfig::default(),
        selection: SelectionHub::default(),
        board_list: BoardList::new(),
        animation: AnimationState::default(),
        filter: FilterState::default(),
        dialog_input: DialogInputState::default(),
        focus: FocusState::default(),
        persistence: PersistenceState::new(None, save_completion_rx),
        multi_select: MultiSelectState::default(),
        ui_state: UiState::default(),
        sprint_view: SprintViewState::default(),
        view: ViewState::default(),
        model: Model::default(),
        controller: kanban_view::Controller::default(),
        relationship: RelationshipState::default(),
        save_error: None,
        pending_key: None,
        has_data_file: true,
        cli_file_provided: false,
        cli_file_override: false,
        config_storage_backend: "json".into(),
        config_storage_location: "conflict.json".into(),
        original_storage_backend: None,
        original_storage_location: None,
        export_dialog: None,
        migration_state: MigrationState::Idle,
        export_result_rx: None,
        needs_redraw: false,
        error_log: Arc::new(Mutex::new(crate::error_log::ErrorLogState::default())),
        auto_open_seen_count: 0,
        choose_storage_backend: StorageBackendChoice::default(),
    };

    app.spawn_save_worker(save_rx, None);

    // Queue a flush signal (simulate a mutation that needs saving).
    app.ctx.save_coordinator.queue_flush();

    // Allow the save worker time to process the flush signal.
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // Completion must NOT have been sent — flush returned ConflictDetected.
    let result = app
        .persistence
        .save_completion_rx
        .as_mut()
        .unwrap()
        .try_recv();
    assert!(
        result.is_err(),
        "save worker must not send completion signal when flush returns ConflictDetected"
    );
}

// multi_thread required: adopt_storage_file uses block_in_place.
#[tokio::test(flavor = "multi_thread")]
async fn test_adopt_storage_file_leaves_context_ready_for_mutations() {
    use crossterm::event::KeyCode;
    use kanban_domain::commands::{BoardCommand, Command, CreateBoard};

    let dir = tempfile::TempDir::new().unwrap();
    let target = dir.path().join("after-adopt.json");

    let sm = default_store_manager();
    let (mut app, _save_rx) = App::new_with_store_and_config(sm, None, Default::default())
        .await
        .unwrap();
    app.maybe_push_startup_file_dialog();
    app.input.clear();
    app.input.set(target.to_str().unwrap().to_string());
    app.handle_choose_storage_file_dialog(KeyCode::Enter);

    // Mirrors the user-level "press n to create a board" path: the
    // context must accept a command after the backend has been swapped.
    let cmd = Command::Board(BoardCommand::Create(CreateBoard {
        id: uuid::Uuid::new_v4(),
        name: "AfterAdopt".into(),
        card_prefix: None,
        position: 0,
    }));
    app.ctx
        .execute_command(cmd)
        .expect("execute_command must succeed after adopt_storage_file");
}

macro_rules! delegate_data_store {
    () => {
        fn get_board(
            &self,
            id: uuid::Uuid,
        ) -> kanban_domain::KanbanResult<Option<kanban_domain::Board>> {
            self.inner.as_data_store().get_board(id)
        }
        fn upsert_board(&self, board: kanban_domain::Board) -> kanban_domain::KanbanResult<()> {
            self.inner.as_data_store().upsert_board(board)
        }
        fn delete_board(&self, id: uuid::Uuid) -> kanban_domain::KanbanResult<()> {
            self.inner.as_data_store().delete_board(id)
        }
        fn get_prefix(
            &self,
            name: &str,
        ) -> kanban_domain::KanbanResult<Option<kanban_domain::Prefix>> {
            self.inner.as_data_store().get_prefix(name)
        }
        fn list_prefixes(&self) -> kanban_domain::KanbanResult<Vec<kanban_domain::Prefix>> {
            self.inner.as_data_store().list_prefixes()
        }
        fn upsert_prefix(&self, prefix: kanban_domain::Prefix) -> kanban_domain::KanbanResult<()> {
            self.inner.as_data_store().upsert_prefix(prefix)
        }
        fn get_column(
            &self,
            id: uuid::Uuid,
        ) -> kanban_domain::KanbanResult<Option<kanban_domain::Column>> {
            self.inner.as_data_store().get_column(id)
        }
        fn list_columns_by_board(
            &self,
            board_id: uuid::Uuid,
        ) -> kanban_domain::KanbanResult<Vec<kanban_domain::Column>> {
            self.inner.as_data_store().list_columns_by_board(board_id)
        }
        fn list_all_columns(&self) -> kanban_domain::KanbanResult<Vec<kanban_domain::Column>> {
            self.inner.as_data_store().list_all_columns()
        }
        fn upsert_column(&self, column: kanban_domain::Column) -> kanban_domain::KanbanResult<()> {
            self.inner.as_data_store().upsert_column(column)
        }
        fn delete_column(&self, id: uuid::Uuid) -> kanban_domain::KanbanResult<()> {
            self.inner.as_data_store().delete_column(id)
        }
        fn delete_columns_by_board(&self, board_id: uuid::Uuid) -> kanban_domain::KanbanResult<()> {
            self.inner.as_data_store().delete_columns_by_board(board_id)
        }
        fn get_card(
            &self,
            id: uuid::Uuid,
        ) -> kanban_domain::KanbanResult<Option<kanban_domain::Card>> {
            self.inner.as_data_store().get_card(id)
        }
        fn list_all_cards(&self) -> kanban_domain::KanbanResult<Vec<kanban_domain::Card>> {
            self.inner.as_data_store().list_all_cards()
        }
        fn list_cards_by_column(
            &self,
            column_id: uuid::Uuid,
        ) -> kanban_domain::KanbanResult<Vec<kanban_domain::Card>> {
            self.inner.as_data_store().list_cards_by_column(column_id)
        }
        fn list_cards_by_sprint(
            &self,
            sprint_id: uuid::Uuid,
        ) -> kanban_domain::KanbanResult<Vec<kanban_domain::Card>> {
            self.inner.as_data_store().list_cards_by_sprint(sprint_id)
        }
        fn count_cards_in_column(
            &self,
            column_id: uuid::Uuid,
        ) -> kanban_domain::KanbanResult<usize> {
            self.inner.as_data_store().count_cards_in_column(column_id)
        }
        fn count_cards_in_column_excluding(
            &self,
            column_id: uuid::Uuid,
            exclude: &[uuid::Uuid],
        ) -> kanban_domain::KanbanResult<usize> {
            self.inner
                .as_data_store()
                .count_cards_in_column_excluding(column_id, exclude)
        }
        fn upsert_card(&self, card: kanban_domain::Card) -> kanban_domain::KanbanResult<()> {
            self.inner.as_data_store().upsert_card(card)
        }
        fn delete_card(&self, id: uuid::Uuid) -> kanban_domain::KanbanResult<()> {
            self.inner.as_data_store().delete_card(id)
        }
        fn delete_cards_by_columns(
            &self,
            column_ids: &[uuid::Uuid],
        ) -> kanban_domain::KanbanResult<()> {
            self.inner
                .as_data_store()
                .delete_cards_by_columns(column_ids)
        }
        fn clear_sprint_from_cards(
            &self,
            sprint_id: uuid::Uuid,
            timestamp: chrono::DateTime<chrono::Utc>,
        ) -> kanban_domain::KanbanResult<()> {
            self.inner
                .as_data_store()
                .clear_sprint_from_cards(sprint_id, timestamp)
        }
        fn get_archived_card(
            &self,
            card_id: uuid::Uuid,
        ) -> kanban_domain::KanbanResult<Option<kanban_domain::ArchivedCard>> {
            self.inner.as_data_store().get_archived_card(card_id)
        }
        fn list_archived_cards(
            &self,
        ) -> kanban_domain::KanbanResult<Vec<kanban_domain::ArchivedCard>> {
            self.inner.as_data_store().list_archived_cards()
        }
        fn insert_archived_card(
            &self,
            ac: kanban_domain::ArchivedCard,
        ) -> kanban_domain::KanbanResult<()> {
            self.inner.as_data_store().insert_archived_card(ac)
        }
        fn delete_archived_card(&self, card_id: uuid::Uuid) -> kanban_domain::KanbanResult<()> {
            self.inner.as_data_store().delete_archived_card(card_id)
        }
        fn list_archived_boards(
            &self,
        ) -> kanban_domain::KanbanResult<Vec<kanban_domain::ArchivedBoard>> {
            self.inner.as_data_store().list_archived_boards()
        }
        fn get_sprint(
            &self,
            id: uuid::Uuid,
        ) -> kanban_domain::KanbanResult<Option<kanban_domain::Sprint>> {
            self.inner.as_data_store().get_sprint(id)
        }
        fn list_sprints_by_board(
            &self,
            board_id: uuid::Uuid,
        ) -> kanban_domain::KanbanResult<Vec<kanban_domain::Sprint>> {
            self.inner.as_data_store().list_sprints_by_board(board_id)
        }
        fn list_all_sprints(&self) -> kanban_domain::KanbanResult<Vec<kanban_domain::Sprint>> {
            self.inner.as_data_store().list_all_sprints()
        }
        fn upsert_sprint(&self, sprint: kanban_domain::Sprint) -> kanban_domain::KanbanResult<()> {
            self.inner.as_data_store().upsert_sprint(sprint)
        }
        fn delete_sprint(&self, id: uuid::Uuid) -> kanban_domain::KanbanResult<()> {
            self.inner.as_data_store().delete_sprint(id)
        }
        fn delete_sprints_by_board(&self, board_id: uuid::Uuid) -> kanban_domain::KanbanResult<()> {
            self.inner.as_data_store().delete_sprints_by_board(board_id)
        }
        fn get_graph(&self) -> kanban_domain::KanbanResult<kanban_domain::DependencyGraph> {
            self.inner.as_data_store().get_graph()
        }
        fn set_graph(
            &self,
            graph: kanban_domain::DependencyGraph,
        ) -> kanban_domain::KanbanResult<()> {
            self.inner.as_data_store().set_graph(graph)
        }
    };
}

/// Delegates to `inner` except `DataStore::snapshot`, which always errors.
struct HostileSourceBackend {
    inner: std::sync::Arc<dyn kanban_backend::KanbanBackend>,
}

impl kanban_domain::DataStore for HostileSourceBackend {
    delegate_data_store!();

    fn list_boards(&self) -> kanban_domain::KanbanResult<Vec<kanban_domain::Board>> {
        self.inner.as_data_store().list_boards()
    }
    fn snapshot(&self) -> kanban_domain::KanbanResult<kanban_domain::Snapshot> {
        Err(kanban_domain::KanbanError::Database(
            "HostileSourceBackend: snapshot must not be called".into(),
        ))
    }
    fn apply_snapshot(&self, snapshot: kanban_domain::Snapshot) -> kanban_domain::KanbanResult<()> {
        self.inner.as_data_store().apply_snapshot(snapshot)
    }
}

impl kanban_domain::command_store::CommandStore for HostileSourceBackend {
    fn append_batch(
        &self,
        batch: &kanban_domain::command_batch::CommandBatch,
    ) -> kanban_domain::KanbanResult<u64> {
        self.inner.append_batch(batch)
    }
    fn batch_count(&self) -> kanban_domain::KanbanResult<u64> {
        self.inner.batch_count()
    }
    fn load_batches(
        &self,
        from: u64,
        to: u64,
    ) -> kanban_domain::KanbanResult<Vec<kanban_domain::command_batch::CommandBatch>> {
        self.inner.load_batches(from, to)
    }
}

#[async_trait::async_trait]
impl kanban_backend::KanbanBackend for HostileSourceBackend {
    fn as_data_store(&self) -> &dyn kanban_domain::DataStore {
        self
    }
    async fn flush(&self) -> kanban_domain::KanbanResult<()> {
        self.inner.flush().await
    }
    async fn reload(&self) -> kanban_domain::KanbanResult<()> {
        self.inner.reload().await
    }
    fn mark_dirty(&self) {
        self.inner.mark_dirty()
    }
    fn needs_flush(&self) -> bool {
        self.inner.needs_flush()
    }
    fn needs_save_worker(&self) -> bool {
        self.inner.needs_save_worker()
    }
    fn instance_id(&self) -> uuid::Uuid {
        self.inner.instance_id()
    }
    fn with_transaction(
        &self,
        f: kanban_backend::TransactionFn<'_>,
    ) -> kanban_domain::KanbanResult<()> {
        self.inner.with_transaction(f)
    }
}

/// Delegates to `inner` except `DataStore::apply_snapshot`, which always errors.
struct HostileTargetBackend {
    inner: std::sync::Arc<dyn kanban_backend::KanbanBackend>,
}

impl kanban_domain::DataStore for HostileTargetBackend {
    delegate_data_store!();

    fn list_boards(&self) -> kanban_domain::KanbanResult<Vec<kanban_domain::Board>> {
        self.inner.as_data_store().list_boards()
    }
    fn snapshot(&self) -> kanban_domain::KanbanResult<kanban_domain::Snapshot> {
        self.inner.as_data_store().snapshot()
    }
    fn apply_snapshot(
        &self,
        _snapshot: kanban_domain::Snapshot,
    ) -> kanban_domain::KanbanResult<()> {
        Err(kanban_domain::KanbanError::Database(
            "HostileTargetBackend: apply_snapshot must not be called".into(),
        ))
    }
}

impl kanban_domain::command_store::CommandStore for HostileTargetBackend {
    fn append_batch(
        &self,
        batch: &kanban_domain::command_batch::CommandBatch,
    ) -> kanban_domain::KanbanResult<u64> {
        self.inner.append_batch(batch)
    }
    fn batch_count(&self) -> kanban_domain::KanbanResult<u64> {
        self.inner.batch_count()
    }
    fn load_batches(
        &self,
        from: u64,
        to: u64,
    ) -> kanban_domain::KanbanResult<Vec<kanban_domain::command_batch::CommandBatch>> {
        self.inner.load_batches(from, to)
    }
}

#[async_trait::async_trait]
impl kanban_backend::KanbanBackend for HostileTargetBackend {
    fn as_data_store(&self) -> &dyn kanban_domain::DataStore {
        self
    }
    async fn flush(&self) -> kanban_domain::KanbanResult<()> {
        self.inner.flush().await
    }
    async fn reload(&self) -> kanban_domain::KanbanResult<()> {
        self.inner.reload().await
    }
    fn mark_dirty(&self) {
        self.inner.mark_dirty()
    }
    fn needs_flush(&self) -> bool {
        self.inner.needs_flush()
    }
    fn needs_save_worker(&self) -> bool {
        self.inner.needs_save_worker()
    }
    fn instance_id(&self) -> uuid::Uuid {
        self.inner.instance_id()
    }
    fn with_transaction(
        &self,
        f: kanban_backend::TransactionFn<'_>,
    ) -> kanban_domain::KanbanResult<()> {
        self.inner.with_transaction(f)
    }
}

struct HostileTargetBackendFactory;

#[async_trait::async_trait]
impl kanban_backend::KanbanBackendFactory for HostileTargetBackendFactory {
    fn name(&self) -> &str {
        "hostile-target"
    }

    fn matches_locator(&self, _locator: &str, _header: &[u8]) -> bool {
        true
    }

    async fn create(
        &self,
        locator: &str,
        config: &kanban_core::AppConfig,
    ) -> kanban_domain::KanbanResult<std::sync::Arc<dyn kanban_backend::KanbanBackend>> {
        let inner = kanban_persistence_json::JsonBackendFactory
            .create(locator, config)
            .await?;
        Ok(std::sync::Arc::new(HostileTargetBackend { inner }))
    }
}

fn hostile_target_store_manager() -> kanban_service::StoreManager {
    let mut registry = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(HostileTargetBackendFactory));
    kanban_service::StoreManager::new(registry, backends)
}

struct UnreadableTargetBackendFactory;

#[async_trait::async_trait]
impl kanban_backend::KanbanBackendFactory for UnreadableTargetBackendFactory {
    fn name(&self) -> &str {
        "unreadable-target"
    }

    fn matches_locator(&self, _locator: &str, _header: &[u8]) -> bool {
        true
    }

    async fn create(
        &self,
        locator: &str,
        config: &kanban_core::AppConfig,
    ) -> kanban_domain::KanbanResult<std::sync::Arc<dyn kanban_backend::KanbanBackend>> {
        let inner = kanban_persistence_json::JsonBackendFactory
            .create(locator, config)
            .await?;
        Ok(std::sync::Arc::new(UnreadableTargetBackend { inner }))
    }
}

struct UnreadableTargetBackend {
    inner: std::sync::Arc<dyn kanban_backend::KanbanBackend>,
}

impl kanban_domain::DataStore for UnreadableTargetBackend {
    delegate_data_store!();

    fn snapshot(&self) -> kanban_domain::KanbanResult<kanban_domain::Snapshot> {
        self.inner.as_data_store().snapshot()
    }
    fn apply_snapshot(&self, snapshot: kanban_domain::Snapshot) -> kanban_domain::KanbanResult<()> {
        self.inner.as_data_store().apply_snapshot(snapshot)
    }
    fn list_boards(&self) -> kanban_domain::KanbanResult<Vec<kanban_domain::Board>> {
        Err(kanban_domain::KanbanError::Database(
            "UnreadableTargetBackend: list_boards must not be called".into(),
        ))
    }
}

impl kanban_domain::command_store::CommandStore for UnreadableTargetBackend {
    fn append_batch(
        &self,
        batch: &kanban_domain::command_batch::CommandBatch,
    ) -> kanban_domain::KanbanResult<u64> {
        self.inner.append_batch(batch)
    }
    fn batch_count(&self) -> kanban_domain::KanbanResult<u64> {
        self.inner.batch_count()
    }
    fn load_batches(
        &self,
        from: u64,
        to: u64,
    ) -> kanban_domain::KanbanResult<Vec<kanban_domain::command_batch::CommandBatch>> {
        self.inner.load_batches(from, to)
    }
}

#[async_trait::async_trait]
impl kanban_backend::KanbanBackend for UnreadableTargetBackend {
    fn as_data_store(&self) -> &dyn kanban_domain::DataStore {
        self
    }
    async fn flush(&self) -> kanban_domain::KanbanResult<()> {
        self.inner.flush().await
    }
    async fn reload(&self) -> kanban_domain::KanbanResult<()> {
        self.inner.reload().await
    }
    fn mark_dirty(&self) {
        self.inner.mark_dirty()
    }
    fn needs_flush(&self) -> bool {
        self.inner.needs_flush()
    }
    fn needs_save_worker(&self) -> bool {
        self.inner.needs_save_worker()
    }
    fn instance_id(&self) -> uuid::Uuid {
        self.inner.instance_id()
    }
    fn with_transaction(
        &self,
        f: kanban_backend::TransactionFn<'_>,
    ) -> kanban_domain::KanbanResult<()> {
        self.inner.with_transaction(f)
    }
}

fn unreadable_target_store_manager() -> kanban_service::StoreManager {
    let mut registry = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(UnreadableTargetBackendFactory));
    kanban_service::StoreManager::new(registry, backends)
}

fn app_with_backend(
    backend: std::sync::Arc<dyn kanban_backend::KanbanBackend>,
) -> (App, Option<tokio::sync::mpsc::Receiver<()>>) {
    let inner =
        kanban_service::KanbanContext::open_deferred(backend, kanban_core::AppConfig::default());
    let (ctx, save_rx, save_completion_rx) =
        crate::tui_context::TuiContext::new(inner).expect("TuiContext::new failed");

    let app = App {
        store_manager: Arc::new(default_store_manager()),
        should_quit: false,
        quit_with_pending: false,
        quit_with_migration: false,
        mode: AppMode::Normal,
        mode_stack: Vec::new(),
        input: InputState::new(),
        ctx,
        app_config: kanban_core::AppConfig::default(),
        selection: SelectionHub::default(),
        board_list: BoardList::new(),
        animation: AnimationState::default(),
        filter: FilterState::default(),
        dialog_input: DialogInputState::default(),
        focus: FocusState::default(),
        persistence: PersistenceState::new(None, save_completion_rx),
        multi_select: MultiSelectState::default(),
        ui_state: UiState::default(),
        sprint_view: SprintViewState::default(),
        view: ViewState::default(),
        model: Model::default(),
        controller: kanban_view::Controller::default(),
        relationship: RelationshipState::default(),
        save_error: None,
        pending_key: None,
        has_data_file: false,
        cli_file_provided: false,
        cli_file_override: false,
        config_storage_backend: "json".into(),
        config_storage_location: "in-memory".into(),
        original_storage_backend: None,
        original_storage_location: None,
        export_dialog: None,
        migration_state: MigrationState::Idle,
        export_result_rx: None,
        needs_redraw: false,
        error_log: Arc::new(Mutex::new(crate::error_log::ErrorLogState::default())),
        auto_open_seen_count: 0,
        choose_storage_backend: StorageBackendChoice::default(),
    };
    (app, save_rx)
}

struct WholeWorkspaceIds {
    board_a: uuid::Uuid,
    board_b: uuid::Uuid,
    board_c: uuid::Uuid,
    col_a1: uuid::Uuid,
    col_a2: uuid::Uuid,
    col_b1: uuid::Uuid,
    col_c1: uuid::Uuid,
    card1: uuid::Uuid,
    card2: uuid::Uuid,
    card3: uuid::Uuid,
    card4: uuid::Uuid,
    card5: uuid::Uuid,
    sprint: uuid::Uuid,
}

fn seed_whole_workspace(app: &mut App) -> WholeWorkspaceIds {
    use kanban_domain::{ColumnUpdate, FieldUpdate, GraphOperations, KanbanOperations, Severity};

    let board_a = app
        .ctx
        .create_board("Alpha".into(), Some("alph".into()))
        .unwrap();
    let board_b = app
        .ctx
        .create_board("Beta".into(), Some("beta".into()))
        .unwrap();
    let board_c = app
        .ctx
        .create_board("Gamma".into(), Some("gama".into()))
        .unwrap();

    let col_a1 = app
        .ctx
        .create_column(board_a.id, "Todo".into(), Some(0))
        .unwrap();
    let col_a2 = app
        .ctx
        .create_column(board_a.id, "Done".into(), Some(1))
        .unwrap();
    let col_b1 = app
        .ctx
        .create_column(board_b.id, "Backlog".into(), Some(0))
        .unwrap();
    let col_c1 = app
        .ctx
        .create_column(board_c.id, "Archive Backlog".into(), Some(0))
        .unwrap();

    let col_a1 = app
        .ctx
        .update_column(
            col_a1.id,
            ColumnUpdate {
                wip_limit: FieldUpdate::Set(3),
                ..Default::default()
            },
        )
        .unwrap();
    let col_a2 = app
        .ctx
        .update_column(
            col_a2.id,
            ColumnUpdate {
                wip_limit: FieldUpdate::Set(7),
                ..Default::default()
            },
        )
        .unwrap();
    let col_b1 = app
        .ctx
        .update_column(
            col_b1.id,
            ColumnUpdate {
                wip_limit: FieldUpdate::Set(2),
                ..Default::default()
            },
        )
        .unwrap();
    let col_c1 = app
        .ctx
        .update_column(
            col_c1.id,
            ColumnUpdate {
                wip_limit: FieldUpdate::Set(4),
                ..Default::default()
            },
        )
        .unwrap();

    let sprint = app.ctx.create_sprint(board_a.id, None, None).unwrap();

    let card1 = app
        .ctx
        .create_card(board_a.id, col_a1.id, "First".into(), Default::default())
        .unwrap();
    let card2 = app
        .ctx
        .create_card(board_a.id, col_a1.id, "Second".into(), Default::default())
        .unwrap();
    let card3 = app
        .ctx
        .create_card(board_a.id, col_a2.id, "Third".into(), Default::default())
        .unwrap();
    let card4 = app
        .ctx
        .create_card(board_b.id, col_b1.id, "Fourth".into(), Default::default())
        .unwrap();
    let card5 = app
        .ctx
        .create_card(board_c.id, col_c1.id, "Fifth".into(), Default::default())
        .unwrap();

    app.ctx.assign_card_to_sprint(card1.id, sprint.id).unwrap();
    app.ctx.block(card1.id, card2.id, Severity::High).unwrap();
    app.ctx.archive_card(card3.id).unwrap();
    app.ctx.block(card2.id, card3.id, Severity::Low).unwrap();
    app.ctx.archive_board(board_c.id).unwrap();

    WholeWorkspaceIds {
        board_a: board_a.id,
        board_b: board_b.id,
        board_c: board_c.id,
        col_a1: col_a1.id,
        col_a2: col_a2.id,
        col_b1: col_b1.id,
        col_c1: col_c1.id,
        card1: card1.id,
        card2: card2.id,
        card3: card3.id,
        card4: card4.id,
        card5: card5.id,
        sprint: sprint.id,
    }
}

fn assert_whole_workspace(snapshot: &kanban_domain::Snapshot, ids: &WholeWorkspaceIds) {
    use kanban_domain::Severity;

    let board = |id: uuid::Uuid| snapshot.boards.iter().find(|b| b.id == id).unwrap();
    assert_eq!(board(ids.board_a).name, "Alpha");
    assert_eq!(board(ids.board_b).name, "Beta");
    assert_eq!(board(ids.board_c).name, "Gamma");

    let column = |id: uuid::Uuid| snapshot.columns.iter().find(|c| c.id == id).unwrap();
    let col_a1 = column(ids.col_a1);
    assert_eq!(col_a1.name, "Todo");
    assert_eq!(col_a1.position, 0);
    assert_eq!(col_a1.board_id, ids.board_a);
    assert_eq!(col_a1.wip_limit, Some(3));
    let col_a2 = column(ids.col_a2);
    assert_eq!(col_a2.name, "Done");
    assert_eq!(col_a2.board_id, ids.board_a);
    assert_eq!(col_a2.wip_limit, Some(7));
    let col_b1 = column(ids.col_b1);
    assert_eq!(col_b1.name, "Backlog");
    assert_eq!(col_b1.board_id, ids.board_b);
    assert_eq!(col_b1.wip_limit, Some(2));
    let col_c1 = column(ids.col_c1);
    assert_eq!(col_c1.name, "Archive Backlog");
    assert_eq!(col_c1.board_id, ids.board_c);
    assert_eq!(col_c1.wip_limit, Some(4));

    let card = |id: uuid::Uuid| snapshot.cards.iter().find(|c| c.id == id).unwrap();
    let card1 = card(ids.card1);
    assert_eq!(card1.title, "First");
    assert_eq!(card1.column_id, ids.col_a1);
    assert_eq!(card1.position, 0);
    assert_eq!(card1.prefix, "alph");
    assert_eq!(card1.card_number, 1);
    assert_eq!(card1.sprint_id, Some(ids.sprint));

    let card2 = card(ids.card2);
    assert_eq!(card2.title, "Second");
    assert_eq!(card2.column_id, ids.col_a1);
    assert_eq!(card2.position, 1);
    assert_eq!(card2.prefix, "alph");
    assert_eq!(card2.card_number, 2);

    let card3 = card(ids.card3);
    assert_eq!(card3.title, "Third");
    assert_eq!(card3.column_id, ids.col_a2);
    assert_eq!(card3.position, 0);
    assert_eq!(card3.prefix, "alph");
    assert_eq!(card3.card_number, 3);

    let card4 = card(ids.card4);
    assert_eq!(card4.title, "Fourth");
    assert_eq!(card4.column_id, ids.col_b1);
    assert_eq!(card4.prefix, "beta");
    assert_eq!(card4.card_number, 1);

    let card5 = card(ids.card5);
    assert_eq!(card5.title, "Fifth");
    assert_eq!(card5.column_id, ids.col_c1);
    assert_eq!(card5.prefix, "gama");
    assert_eq!(card5.card_number, 1);

    assert_eq!(
        snapshot.archived_cards.len(),
        1,
        "exactly one archived card marker"
    );
    assert_eq!(
        snapshot.archived_cards[0].entity_id, ids.card3,
        "the archived marker must point at card3"
    );

    assert_eq!(
        snapshot.archived_boards.len(),
        1,
        "exactly one archived board marker"
    );
    assert_eq!(
        snapshot.archived_boards[0].entity_id, ids.board_c,
        "the archived marker must point at board_c"
    );

    assert_eq!(snapshot.sprints.len(), 1, "sprint must survive");
    assert_eq!(snapshot.sprints[0].id, ids.sprint);
    assert_eq!(snapshot.sprints[0].board_id, ids.board_a);

    let blocks = snapshot.graph.blocks_edges();
    assert_eq!(blocks.len(), 2, "both dependency edges must survive");
    let edge_1_2 = blocks
        .iter()
        .find(|e| e.base.source == ids.card1 && e.base.target == ids.card2)
        .expect("card1 -> card2 edge must survive");
    assert_eq!(edge_1_2.severity, Severity::High);
    let edge_2_3 = blocks
        .iter()
        .find(|e| e.base.source == ids.card2 && e.base.target == ids.card3)
        .expect("card2 -> card3 (archived endpoint) edge must survive");
    assert_eq!(edge_2_3.severity, Severity::Low);

    let prefix = |name: &str| {
        snapshot
            .prefixes
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("prefix row \"{name}\" must survive"))
    };
    let alph = prefix("alph");
    assert_eq!(alph.card_counter, 3);
    let beta = prefix("beta");
    assert_eq!(beta.card_counter, 1);
    let gama = prefix("gama");
    assert_eq!(gama.card_counter, 1);
    let sprint_prefix = prefix("sprint");
    assert_eq!(sprint_prefix.sprint_counter, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_adopt_storage_file_writes_the_whole_workspace_to_disk_json() {
    use crossterm::event::KeyCode;

    let dir = tempfile::TempDir::new().unwrap();
    let target = dir.path().join("whole-workspace.json");

    let sm = default_store_manager();
    let (mut app, _save_rx) = App::new_with_store_and_config(sm, None, Default::default())
        .await
        .unwrap();

    let ids = seed_whole_workspace(&mut app);

    app.maybe_push_startup_file_dialog();
    app.input.clear();
    app.input.set(target.to_str().unwrap().to_string());
    app.handle_choose_storage_file_dialog(KeyCode::Enter);

    // Let the save worker flush the queued write to disk.
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    let backend = std::sync::Arc::new(kanban_persistence_json::JsonDataStore::new(
        std::sync::Arc::new(kanban_persistence_json::JsonFileStore::new(
            target.to_str().unwrap(),
        )),
    ));
    use kanban_domain::DataStore as _;
    let snapshot = backend.snapshot().unwrap();

    assert_whole_workspace(&snapshot, &ids);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_adopt_storage_file_writes_the_whole_workspace_to_disk_sqlite() {
    use crossterm::event::KeyCode;
    use kanban_backend::KanbanBackendFactory as _;

    let dir = tempfile::TempDir::new().unwrap();
    let target = dir.path().join("whole-workspace.sqlite3");

    let sm = default_store_manager();
    let (mut app, _save_rx) = App::new_with_store_and_config(sm, None, Default::default())
        .await
        .unwrap();

    let ids = seed_whole_workspace(&mut app);

    app.maybe_push_startup_file_dialog();
    app.input.clear();
    app.input.set(target.to_str().unwrap().to_string());
    app.handle_choose_storage_file_dialog(KeyCode::Enter);

    // Let the save worker flush the queued write to disk.
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    let backend = kanban_persistence_sqlite::SqliteBackendFactory
        .create(target.to_str().unwrap(), &kanban_core::AppConfig::default())
        .await
        .unwrap();
    let snapshot = backend.as_data_store().snapshot().unwrap();

    assert_whole_workspace(&snapshot, &ids);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_adopt_storage_file_writes_an_empty_workspace_to_disk() {
    use crossterm::event::KeyCode;

    let dir = tempfile::TempDir::new().unwrap();
    let target = dir.path().join("empty-workspace.json");

    let sm = default_store_manager();
    let (mut app, _save_rx) = App::new_with_store_and_config(sm, None, Default::default())
        .await
        .unwrap();

    app.maybe_push_startup_file_dialog();
    app.input.clear();
    app.input.set(target.to_str().unwrap().to_string());
    app.handle_choose_storage_file_dialog(KeyCode::Enter);

    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    assert!(
        target.exists(),
        "adopt_storage_file must write the empty workspace to disk"
    );
    let backend = std::sync::Arc::new(kanban_persistence_json::JsonDataStore::new(
        std::sync::Arc::new(kanban_persistence_json::JsonFileStore::new(
            target.to_str().unwrap(),
        )),
    ));
    use kanban_domain::DataStore as _;
    let snapshot = backend.snapshot().unwrap();
    assert!(snapshot.boards.is_empty(), "empty workspace has no boards");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_adopt_storage_file_does_not_call_the_whole_store_trait_methods() {
    use crossterm::event::KeyCode;
    use kanban_domain::KanbanOperations;

    let dir = tempfile::TempDir::new().unwrap();
    let target = dir.path().join("hostile.json");

    let real_source = std::sync::Arc::new(kanban_persistence_json::JsonDataStore::new(
        std::sync::Arc::new(kanban_persistence_json::JsonFileStore::new(
            dir.path().join("source.json").to_str().unwrap(),
        )),
    ));
    let hostile_source: std::sync::Arc<dyn kanban_backend::KanbanBackend> =
        std::sync::Arc::new(HostileSourceBackend { inner: real_source });

    let (mut app, _save_rx) = app_with_backend(hostile_source);
    app.store_manager = std::sync::Arc::new(hostile_target_store_manager());
    app.ctx.create_board("Seed".into(), None).unwrap();

    app.maybe_push_startup_file_dialog();
    app.input.clear();
    app.input.set(target.to_str().unwrap().to_string());
    app.handle_choose_storage_file_dialog(KeyCode::Enter);

    assert_eq!(
        app.mode,
        AppMode::Normal,
        "adopt must succeed and dismiss the dialog even with hostile snapshot/apply_snapshot"
    );
    assert!(
        app.has_data_file,
        "adopt must succeed without calling snapshot/apply_snapshot"
    );

    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    assert!(
        target.exists(),
        "the workspace must have been written to disk via per-entity transfer"
    );
    let readback = std::sync::Arc::new(kanban_persistence_json::JsonDataStore::new(
        std::sync::Arc::new(kanban_persistence_json::JsonFileStore::new(
            target.to_str().unwrap(),
        )),
    ));
    use kanban_domain::DataStore as _;
    let snapshot = readback.snapshot().unwrap();
    assert_eq!(
        snapshot.boards.len(),
        1,
        "seeded board must have transferred"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_adopt_storage_file_aborts_without_swapping_when_the_target_is_unreadable() {
    use crossterm::event::KeyCode;

    let dir = tempfile::TempDir::new().unwrap();
    let target = dir.path().join("unreadable.json");

    let sm = unreadable_target_store_manager();
    let (mut app, _save_rx) = App::new_with_store_and_config(sm, None, Default::default())
        .await
        .unwrap();
    let original_instance_id = app.ctx.backend().instance_id();

    app.maybe_push_startup_file_dialog();
    app.input.clear();
    app.input.set(target.to_str().unwrap().to_string());
    app.handle_choose_storage_file_dialog(KeyCode::Enter);

    assert!(
        !app.has_data_file,
        "adopt must fail when the target backend cannot be read back"
    );
    assert!(
        app.persistence.save_file.is_none(),
        "save_file must be unchanged when adopt fails"
    );
    assert_eq!(
        app.ctx.backend().instance_id(),
        original_instance_id,
        "the backend must not have been swapped when the read-back probe fails"
    );
    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("a failed adopt must surface an error banner");
    assert_eq!(banner.variant, crate::components::BannerVariant::Error);
    assert!(
        banner
            .message
            .contains("UnreadableTargetBackend: list_boards"),
        "banner must name the read-back probe as the failure, got: {}",
        banner.message
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_adopt_storage_file_still_refuses_an_existing_path() {
    use crossterm::event::KeyCode;

    let dir = tempfile::TempDir::new().unwrap();
    let target = dir.path().join("already-here.json");
    std::fs::write(&target, "{}").unwrap();

    let sm = default_store_manager();
    let (mut app, _save_rx) = App::new_with_store_and_config(sm, None, Default::default())
        .await
        .unwrap();
    let original_instance_id = app.ctx.backend().instance_id();

    app.maybe_push_startup_file_dialog();
    app.input.clear();
    app.input.set(target.to_str().unwrap().to_string());
    app.handle_choose_storage_file_dialog(KeyCode::Enter);

    assert!(
        !app.has_data_file,
        "adopt must refuse a path that already exists"
    );
    assert_eq!(
        app.ctx.backend().instance_id(),
        original_instance_id,
        "the backend must not have been swapped when the existing-path guard fires"
    );
    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("refusing an existing path must surface an error banner");
    assert_eq!(banner.variant, crate::components::BannerVariant::Error);
    assert!(
        banner.message.contains("already exists"),
        "banner must explain that the file already exists, got: {}",
        banner.message
    );
}
