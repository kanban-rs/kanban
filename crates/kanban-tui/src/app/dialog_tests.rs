use super::*;

#[tokio::test]
async fn test_no_file_tui_startup_pushes_choose_storage_dialog_prefilled_with_boards_json() {
    let sm = super::types::default_store_manager();
    let _cfg = super::test_support::isolated_config();
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

    let sm = super::types::default_store_manager();
    let _cfg = super::test_support::isolated_config();
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

    let sm = super::types::default_store_manager();
    let _cfg = super::test_support::isolated_config();
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

    let sm = super::types::default_store_manager();
    let _cfg = super::test_support::isolated_config();
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

    let sm = super::types::default_store_manager();
    let _cfg = super::test_support::isolated_config();
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

    let sm = super::types::default_store_manager();
    let _cfg = super::test_support::isolated_config();
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

#[tokio::test]
async fn test_choose_storage_dialog_default_backend_is_json() {
    let sm = super::types::default_store_manager();
    let _cfg = super::test_support::isolated_config();
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

    let sm = super::types::default_store_manager();
    let _cfg = super::test_support::isolated_config();
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

    let sm = super::types::default_store_manager();
    let _cfg = super::test_support::isolated_config();
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
