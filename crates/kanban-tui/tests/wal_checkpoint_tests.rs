use kanban_domain::{KanbanOperations, KanbanResult};
use kanban_service::{AppConfig, KanbanContext, StoreManager};
use kanban_tui::tui_context::TuiContext;
use tempfile::TempDir;

fn test_store_manager() -> StoreManager {
    let mut registry = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    registry.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    StoreManager::new(registry, backends)
}

async fn open_context(locator: &str, config: AppConfig) -> KanbanResult<KanbanContext> {
    let mut config = config;
    let sm = test_store_manager();
    sm.sync_backend_with_file(locator, &mut config);
    let backend = sm.make_backend(locator, &config).await?;
    KanbanContext::open(backend, config).await
}

fn assert_wal_empty(db_path: &std::path::Path) {
    let wal = db_path.with_extension("sqlite3-wal");
    let len = if wal.exists() {
        wal.metadata().unwrap().len()
    } else {
        0
    };
    assert_eq!(len, 0, "WAL should be empty at {}", wal.display());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_tui_execute_queues_flush_signal_on_json_path() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("test.json");
    let sm = test_store_manager();
    let backend = sm
        .make_backend(path.to_str().unwrap(), &AppConfig::default())
        .await
        .unwrap();
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let (mut tui_ctx, save_rx, _completion_rx) = TuiContext::new(ctx).unwrap();
    let mut save_rx = save_rx.unwrap();

    tui_ctx.create_board("TestBoard".to_string(), None).unwrap();

    save_rx
        .try_recv()
        .expect("flush signal should be queued after create_board on JSON path");
    assert_eq!(tui_ctx.list_boards().unwrap().len(), 1);
    assert_eq!(tui_ctx.list_boards().unwrap()[0].name, "TestBoard");
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_tui_execute_checkpoints_wal_on_sqlite_path() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let ctx = open_context(path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    let (mut tui_ctx, _, _) = TuiContext::new(ctx).unwrap();
    tui_ctx.create_board("B".to_string(), None).unwrap();
    tui_ctx.save().await.unwrap();
    assert_wal_empty(&path);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_tui_undo_checkpoints_wal_on_sqlite_path() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let ctx = open_context(path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    let (mut tui_ctx, _, _) = TuiContext::new(ctx).unwrap();
    tui_ctx.create_board("B".to_string(), None).unwrap();
    assert!(tui_ctx.undo().unwrap());
    tui_ctx.save().await.unwrap();
    assert_wal_empty(&path);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_tui_redo_checkpoints_wal_on_sqlite_path() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let ctx = open_context(path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    let (mut tui_ctx, _, _) = TuiContext::new(ctx).unwrap();
    tui_ctx.create_board("B".to_string(), None).unwrap();
    assert!(tui_ctx.undo().unwrap());
    assert!(tui_ctx.redo().unwrap());
    tui_ctx.save().await.unwrap();
    assert_wal_empty(&path);
}
