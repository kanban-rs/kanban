use kanban_persistence_json::JsonFileStore;
use kanban_server::state::AppState;
use kanban_server::watch::watch_for_external_changes;
use kanban_service::json_backend::JsonDataStore;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread")]
async fn test_get_boards_reflects_board_created_by_external_writer() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("s.json");

    // "Server" side: the context the watcher will keep fresh.
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(&path))));
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let state = AppState::new(ctx);
    watch_for_external_changes(state.clone(), path.to_str().unwrap())
        .await
        .unwrap();

    assert_eq!(state.ctx.lock().await.list_boards().unwrap().len(), 0);

    // "External process" (e.g. the TUI): a SEPARATE context, same file.
    {
        let backend: Arc<dyn KanbanBackend> =
            Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(&path))));
        let mut external_ctx = KanbanContext::open(backend, AppConfig::default())
            .await
            .unwrap();
        external_ctx
            .create_board("External Board".to_string(), Some("EXT".to_string()))
            .unwrap();
        external_ctx.save().await.unwrap(); // flush to disk — the write the watcher must see
    }

    // The watcher reacts asynchronously — poll with a bounded timeout
    // rather than asserting immediately after the external write.
    let mut boards_len = 0;
    for _ in 0..50 {
        boards_len = state.ctx.lock().await.list_boards().unwrap().len();
        if boards_len == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(
        boards_len, 1,
        "server must reflect the externally-created board within 5s"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_watch_for_external_changes_is_noop_for_sqlite_locator() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("s.sqlite");
    let backend: Arc<dyn KanbanBackend> = Arc::new(
        kanban_service::sqlite_backend::SqliteBackend::open(path.to_str().unwrap())
            .await
            .unwrap(),
    );
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let state = AppState::new(ctx);

    // Must return Ok and must NOT attempt to watch a path notify can't
    // meaningfully treat the same way — confirms the is_sqlite() guard.
    watch_for_external_changes(state, path.to_str().unwrap())
        .await
        .unwrap();
}
