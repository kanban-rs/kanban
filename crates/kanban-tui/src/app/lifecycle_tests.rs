use super::types::default_store_manager;
use super::*;
use kanban_core::InputState;
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
    use kanban_service::json_backend::JsonDataStore;
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
        animation: AnimationState::default(),
        filter: FilterState::default(),
        dialog_input: DialogInputState::default(),
        focus: FocusState::default(),
        persistence: PersistenceState::new(None, save_completion_rx),
        multi_select: MultiSelectState::default(),
        ui_state: UiState::default(),
        sprint_view: SprintViewState::default(),
        view: ViewState::default(),
        model: model::Model::default(),
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

    let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
    let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
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
