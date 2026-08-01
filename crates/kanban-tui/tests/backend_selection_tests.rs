fn test_store_manager() -> kanban_service::StoreManager {
    let mut registry = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    registry.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    kanban_service::StoreManager::new(registry, backends)
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_new_with_store_sqlite_path_yields_no_save_worker() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("board.sqlite");

    // Pre-create the SQLite file so content-sniffing works.
    kanban_persistence_sqlite::SqliteStore::open(path.to_str().unwrap())
        .await
        .unwrap();

    let sm = test_store_manager();
    let (app, save_rx) =
        kanban_tui::App::new_with_store(sm, Some(path.to_str().unwrap().to_string()))
            .await
            .unwrap();

    assert!(
        !app.ctx.backend().needs_save_worker(),
        "SQLite backend must not require a background save worker"
    );
    assert!(
        save_rx.is_none(),
        "no save channel should be created for a write-through backend"
    );
}

#[tokio::test]
async fn test_new_with_store_json_path_yields_save_worker() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("board.json");

    let sm = test_store_manager();
    let (app, save_rx) =
        kanban_tui::App::new_with_store(sm, Some(path.to_str().unwrap().to_string()))
            .await
            .unwrap();

    assert!(
        app.ctx.backend().needs_save_worker(),
        "JSON backend must require a background save worker"
    );
    assert!(
        save_rx.is_some(),
        "a save channel should be created for a JSON backend"
    );
}

// cwd is process-global; the only test in this file that mutates it must
// serialize access. A static lock keeps the file robust if more cwd-dependent
// tests are added later.
static CWD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[tokio::test]
// Holding the std Mutex across the await is safe here: this test runs on the
// single-threaded current_thread runtime, no other task can need the lock,
// and we need cwd to remain set for the full duration of the call.
#[allow(clippy::await_holding_lock)]
async fn test_new_with_store_no_file_uses_in_memory_backend_and_has_no_save_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let _guard = CWD_LOCK.lock().unwrap();
    let original_cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();

    let sm = test_store_manager();
    let (app, _save_rx) = kanban_tui::App::new_with_store(sm, None).await.unwrap();

    assert!(
        app.persistence.save_file.is_none(),
        "no-file mode must not associate a save path"
    );
    assert!(
        !app.has_data_file,
        "no-file mode must set has_data_file = false"
    );
    assert!(
        !dir.path().join("kanban.json").exists(),
        "must not create kanban.json in cwd when no file is given"
    );

    std::env::set_current_dir(original_cwd).unwrap();
}
