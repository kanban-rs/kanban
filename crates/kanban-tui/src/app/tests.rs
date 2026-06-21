use super::types::default_store_manager;
use super::*;
use kanban_core::InputState;
use ratatui::layout::Rect;
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

#[test]
fn test_scroll_help_into_view_scrolls_deep_item() {
    let mut app = App::test_default();
    app.view.last_frame_area = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 50,
    };
    app.ui_state.help_list.update_item_count(50);
    app.ui_state.help_list.jump_to(49);
    app.scroll_help_into_view();
    assert!(
        app.ui_state.help_list.get_scroll_offset() > 0,
        "help list should have scrolled to bring item 49 into view"
    );
}

#[tokio::test]
async fn test_no_file_tui_startup_pushes_choose_storage_dialog_prefilled_with_boards_json() {
    let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
    let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();

    app.maybe_push_startup_file_dialog();

    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::ChooseStorageFile),
        "no-file startup must open the ChooseStorageFile dialog"
    );
    assert_eq!(
        app.input.as_str(),
        "boards.json",
        "dialog must be pre-filled with boards.json"
    );
}

#[tokio::test]
async fn test_no_file_tui_startup_dialog_cancel_stays_in_memory() {
    use crossterm::event::KeyCode;

    let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
    let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
    app.maybe_push_startup_file_dialog();

    app.handle_choose_storage_file_dialog(KeyCode::Esc);

    assert_eq!(
        app.mode,
        AppMode::Normal,
        "cancelling the dialog must return to Normal mode"
    );
    assert!(
        !app.has_data_file,
        "cancelling must leave the app in in-memory mode"
    );
    assert!(
        app.persistence.save_file.is_none(),
        "cancelling must not set a save file"
    );
}

// multi_thread required: adopt_storage_file uses block_in_place, which
// panics on the current_thread runtime.
#[tokio::test(flavor = "multi_thread")]
async fn test_no_file_tui_startup_dialog_confirm_creates_file_and_adopts_backend() {
    use crossterm::event::KeyCode;

    let dir = tempfile::TempDir::new().unwrap();
    let target = dir.path().join("myboard.json");
    let target_str = target.to_str().unwrap().to_string();

    let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
    let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
    app.maybe_push_startup_file_dialog();

    app.input.clear();
    app.input.set(target_str.clone());

    app.handle_choose_storage_file_dialog(KeyCode::Enter);

    assert!(
        app.has_data_file,
        "confirming must mark the app as having a data file"
    );
    assert_eq!(
        app.persistence.save_file.as_deref(),
        Some(target_str.as_str()),
        "persistence.save_file must point to the chosen path"
    );
    assert_eq!(
        app.mode,
        AppMode::Normal,
        "confirming must dismiss the dialog"
    );
}

// multi_thread required: adopt_storage_file uses block_in_place + the save
// worker is a spawned task that writes to disk asynchronously.
#[tokio::test(flavor = "multi_thread")]
async fn test_no_file_tui_startup_dialog_confirm_persists_in_memory_state_to_disk() {
    use crossterm::event::KeyCode;
    use kanban_domain::Board;

    let dir = tempfile::TempDir::new().unwrap();
    let target = dir.path().join("seeded.json");
    let target_str = target.to_str().unwrap().to_string();

    let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
    let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();

    // Seed in-memory state with a board so we can detect whether adopt
    // transferred it to the new on-disk backend.
    let mut snapshot = app.ctx.snapshot().unwrap();
    snapshot
        .boards
        .push(Board::new("BeforeAdopt", None::<String>));
    app.ctx.apply_snapshot(snapshot).unwrap();

    app.maybe_push_startup_file_dialog();
    app.input.clear();
    app.input.set(target_str.clone());
    app.handle_choose_storage_file_dialog(KeyCode::Enter);

    // Wait for the save worker to flush at least once.
    let completion_rx = app
        .persistence
        .save_completion_rx
        .as_mut()
        .expect("save completion channel must exist after adopt");
    tokio::time::timeout(std::time::Duration::from_secs(2), completion_rx.recv())
        .await
        .expect("save worker must signal completion within 2s")
        .expect("completion sender dropped before signal");

    assert!(
        target.exists(),
        "target file must exist on disk after adopt"
    );

    use kanban_persistence::PersistenceStore;
    let on_disk = kanban_persistence_json::JsonFileStore::new(&target_str);
    let (snap, _meta) = on_disk
        .load_sync()
        .unwrap()
        .expect("file must contain a snapshot");
    let parsed = kanban_persistence::snapshot_from_json_bytes(&snap.data).unwrap();
    assert!(
        parsed.boards.iter().any(|b| b.name == "BeforeAdopt"),
        "in-memory state must be transferred to the new on-disk backend"
    );
}

// multi_thread required for the same reason as the success test.
#[tokio::test(flavor = "multi_thread")]
async fn test_no_file_tui_startup_dialog_confirm_refuses_existing_path() {
    use crossterm::event::KeyCode;

    let dir = tempfile::TempDir::new().unwrap();
    let target = dir.path().join("already-here.json");
    std::fs::write(&target, b"{\"boards\":[]}").unwrap();
    let target_str = target.to_str().unwrap().to_string();

    let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
    let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
    app.maybe_push_startup_file_dialog();
    app.input.clear();
    app.input.set(target_str.clone());

    app.handle_choose_storage_file_dialog(KeyCode::Enter);

    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::ChooseStorageFile),
        "existing-file confirm must leave the dialog open"
    );
    assert_eq!(
        app.input.as_str(),
        target_str.as_str(),
        "input must be preserved so the user can pick a different name"
    );
    assert!(
        !app.has_data_file,
        "existing-file confirm must not flip has_data_file"
    );
    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("existing-file confirm must surface a banner");
    assert_eq!(banner.variant, crate::components::BannerVariant::Error);
    assert!(
        banner.message.contains("already exists"),
        "banner must explain that the file already exists, got: {}",
        banner.message
    );
    // The pre-existing file content must not have been touched.
    let on_disk = std::fs::read(&target).unwrap();
    assert_eq!(
        on_disk,
        b"{\"boards\":[]}".to_vec(),
        "the existing file must not have been overwritten"
    );
}

// multi_thread required for the same reason as the success test.
#[tokio::test(flavor = "multi_thread")]
async fn test_no_file_tui_startup_dialog_confirm_failure_keeps_dialog_open() {
    use crossterm::event::KeyCode;

    // SQLite path inside a non-existent parent directory — SqliteBackend::open
    // cannot create the file because the parent doesn't exist, so make_backend
    // returns Err.
    let dir = tempfile::TempDir::new().unwrap();
    let bad_path = dir
        .path()
        .join("nonexistent_subdir")
        .join("foo.sqlite")
        .to_str()
        .unwrap()
        .to_string();

    let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
    let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
    app.maybe_push_startup_file_dialog();

    app.input.clear();
    app.input.set(bad_path.clone());

    app.handle_choose_storage_file_dialog(KeyCode::Enter);

    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::ChooseStorageFile),
        "confirm failure must leave the dialog open so the user can retry"
    );
    assert_eq!(
        app.input.as_str(),
        bad_path.as_str(),
        "input must be preserved on failure so the user can edit the path"
    );
    assert!(
        !app.has_data_file,
        "confirm failure must not flip has_data_file"
    );
    assert!(
        app.persistence.save_file.is_none(),
        "confirm failure must not set a save file"
    );
    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("confirm failure must surface a banner");
    assert_eq!(
        banner.variant,
        crate::components::BannerVariant::Error,
        "banner must be an error variant"
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

#[tokio::test]
async fn test_choose_storage_dialog_default_backend_is_json() {
    let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
    let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
    app.maybe_push_startup_file_dialog();

    assert_eq!(
        app.choose_storage_backend,
        StorageBackendChoice::Json,
        "default storage backend selection must be JSON"
    );
    assert_eq!(
        app.input.as_str(),
        "boards.json",
        "default filename must end in .json to match the default backend"
    );
}

#[tokio::test]
async fn test_choose_storage_dialog_tab_toggles_backend_and_swaps_extension() {
    use crossterm::event::KeyCode;

    let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
    let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
    app.maybe_push_startup_file_dialog();

    app.handle_choose_storage_file_dialog(KeyCode::Tab);
    assert_eq!(
        app.choose_storage_backend,
        StorageBackendChoice::Sqlite,
        "Tab must toggle the backend selection from JSON to SQLite"
    );
    assert_eq!(
        app.input.as_str(),
        "boards.sqlite",
        "Tab must swap the filename extension to match the new backend"
    );

    app.handle_choose_storage_file_dialog(KeyCode::Tab);
    assert_eq!(
        app.choose_storage_backend,
        StorageBackendChoice::Json,
        "Tab must toggle the backend selection back to JSON"
    );
    assert_eq!(
        app.input.as_str(),
        "boards.json",
        "Tab must swap the filename extension back to .json"
    );
}

#[tokio::test]
async fn test_choose_storage_dialog_tab_appends_extension_for_filename_without_one() {
    use crossterm::event::KeyCode;

    let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
    let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
    app.maybe_push_startup_file_dialog();
    app.input.clear();
    app.input.set("myboard".to_string());

    app.handle_choose_storage_file_dialog(KeyCode::Tab);

    assert_eq!(
        app.input.as_str(),
        "myboard.sqlite",
        "Tab must append the extension when the filename has no known extension"
    );
}

#[test]
fn test_swap_known_extension_table() {
    let cases = [
        // Primary swap
        ("boards.json", ".sqlite", "boards.sqlite"),
        ("boards.sqlite", ".json", "boards.json"),
        // Alternative SQLite extensions are recognised
        ("boards.sqlite3", ".json", "boards.json"),
        ("boards.db", ".json", "boards.json"),
        // No known extension → append
        ("boards", ".sqlite", "boards.sqlite"),
        // Empty input → returns just the new extension. The dialog
        // pre-fills "boards.json" so this is unreachable in practice;
        // documented for the helper's stand-alone behaviour.
        ("", ".json", ".json"),
        // Multi-dot stems are preserved
        ("foo.tar.json", ".sqlite", "foo.tar.sqlite"),
        // Known list is lowercase-only — uppercase extensions are
        // not recognised and the new extension is appended.
        ("FOO.JSON", ".sqlite", "FOO.JSON.sqlite"),
    ];

    for (input, ext, expected) in cases {
        assert_eq!(
            swap_known_extension(input, ext),
            expected,
            "swap_known_extension({:?}, {:?})",
            input,
            ext
        );
    }
}

mod active_card_index_regression {
    use crate::test_helpers::{load_with_card_order, setup_reload_resort_fixture};
    use crate::App;
    use kanban_domain::{CardPriority, CardUpdate, KanbanOperations};

    #[test]
    fn test_get_current_priority_selection_index_after_reload_resort_returns_originally_selected_card_priority(
    ) {
        let mut app = App::test_default();
        let fx = setup_reload_resort_fixture(&mut app);

        app.ctx
            .update_card(
                fx.a_id,
                CardUpdate {
                    priority: Some(CardPriority::Critical),
                    ..Default::default()
                },
            )
            .unwrap();
        app.ctx
            .update_card(
                fx.p_id,
                CardUpdate {
                    priority: Some(CardPriority::Low),
                    ..Default::default()
                },
            )
            .unwrap();
        load_with_card_order(&mut app, &[fx.a_id, fx.p_id, fx.b_id, fx.c_id, fx.d_id]);

        let idx = app.get_current_priority_selection_index();

        assert_eq!(
                idx, 3,
                "must return Critical's index (3) — A's priority — not Low's index (0) which is P's priority at A's stale index"
            );
    }

    #[test]
    fn test_get_current_sprint_selection_index_after_reload_resort_returns_originally_selected_card_sprint(
    ) {
        use crate::components::sprint_assign_list::{build_entries, sprint_id_of};

        let mut app = App::test_default();
        let fx = setup_reload_resort_fixture(&mut app);

        let sprint_a = app.ctx.create_sprint(fx.board_id, None, None).unwrap();
        let sprint_p = app.ctx.create_sprint(fx.board_id, None, None).unwrap();
        app.ctx.assign_card_to_sprint(fx.a_id, sprint_a.id).unwrap();
        app.ctx.assign_card_to_sprint(fx.p_id, sprint_p.id).unwrap();
        load_with_card_order(&mut app, &[fx.a_id, fx.p_id, fx.b_id, fx.c_id, fx.d_id]);

        let sprints = app.model.sprints().to_vec();
        let entries = build_entries(&sprints, fx.board_id, chrono::Utc::now());
        let expected_idx = entries
            .iter()
            .position(|e| sprint_id_of(e) == Some(sprint_a.id))
            .expect("sprint_a appears in entries");

        let idx = app.get_current_sprint_selection_index();

        assert_eq!(
            idx, expected_idx,
            "must return A's sprint index, not P's sprint index at A's stale slot"
        );
    }
}

mod active_card_helpers {
    use crate::App;
    use kanban_domain::{CreateCardOptions, KanbanOperations, Snapshot};

    fn app_with_card() -> (App, uuid::Uuid) {
        let mut app = App::test_default();
        let board = app.ctx.create_board("B".into(), None).unwrap();
        let column = app
            .ctx
            .create_column(board.id, "Todo".into(), None)
            .unwrap();
        let card = app
            .ctx
            .create_card(
                board.id,
                column.id,
                "C".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        let snap = Snapshot {
            boards: app.ctx.data_store().list_boards().unwrap(),
            columns: app.ctx.data_store().list_all_columns().unwrap(),
            cards: app.ctx.data_store().list_all_cards().unwrap(),
            archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
            sprints: app.ctx.data_store().list_all_sprints().unwrap(),
            graph: app.ctx.data_store().get_graph().unwrap(),
        };
        app.model.load_from_snapshot(snap);
        (app, card.id)
    }

    #[test]
    fn test_activate_card_with_known_id_sets_active_card_id_and_returns_true() {
        let (mut app, card_id) = app_with_card();

        let succeeded = app.activate_card(card_id);

        assert!(succeeded, "must report success when the card exists");
        assert_eq!(app.selection.active_card_id, Some(card_id));
    }

    #[test]
    fn test_activate_card_with_unknown_id_preserves_active_card_id_and_returns_false() {
        let (mut app, card_id) = app_with_card();
        app.selection.active_card_id = Some(card_id);

        let succeeded = app.activate_card(uuid::Uuid::new_v4());

        assert!(!succeeded, "must report failure when the card is absent");
        assert_eq!(
                app.selection.active_card_id,
                Some(card_id),
                "activate_card must not touch active_card_id on miss — sites that need clear-on-miss must use set_active_card_or_clear"
            );
    }

    #[test]
    fn test_set_active_card_or_clear_with_known_id_sets_active_card_id() {
        let (mut app, card_id) = app_with_card();

        app.set_active_card_or_clear(card_id);

        assert_eq!(app.selection.active_card_id, Some(card_id));
    }

    #[test]
    fn test_set_active_card_or_clear_with_unknown_id_clears_active_card_id() {
        let (mut app, card_id) = app_with_card();
        app.selection.active_card_id = Some(card_id);

        app.set_active_card_or_clear(uuid::Uuid::new_v4());

        assert_eq!(
                app.selection.active_card_id, None,
                "set_active_card_or_clear must clear the previous active card when the new id is absent — prevents downstream handlers from acting on a stale active card"
            );
    }
}
