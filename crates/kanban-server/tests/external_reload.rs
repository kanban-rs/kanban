mod common;

use common::{json_of, send};
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

#[tokio::test(flavor = "multi_thread")]
async fn test_own_write_does_not_trigger_external_reload_path() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("s.json");

    // "Server" side: the context the watcher will keep fresh.
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(&path))));
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let state = AppState::new(ctx);

    // Start watching for external changes — this spawns a background task that
    // will reload + broadcast when it detects file changes.
    watch_for_external_changes(state.clone(), path.to_str().unwrap())
        .await
        .unwrap();

    // Subscribe to the broadcast channel BEFORE making any writes, so we capture
    // exactly the events from our test write.
    let mut event_rx = state.event_tx.subscribe();

    // Make a board creation via the HTTP route. This internally calls
    // persist_and_broadcast, which broadcasts event #1 immediately.
    let req = serde_json::json!({"name": "Test Board", "card_prefix": "TB"});
    let resp = send(&state, "POST", "/v1/boards", Some(&req)).await;
    assert_eq!(resp.status(), axum::http::StatusCode::CREATED);
    let _board = json_of(resp).await;

    // Wait long enough for the file to be flushed (500ms debounce) and the
    // watcher to detect the change and fire its own reload+broadcast.
    // With the bug, the watcher doesn't know this was its own write, so it
    // sends event #2. With the fix, it recognizes instance_id and doesn't.
    tokio::time::sleep(Duration::from_millis(800)).await;

    // Count how many events arrived. Use try_recv() to drain all pending events.
    let mut event_count = 0;
    loop {
        match event_rx.try_recv() {
            Ok(_) => event_count += 1,
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {
                panic!("Broadcast channel lagged; increase capacity")
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
        }
    }

    // Assert we received exactly 1 event (from persist_and_broadcast in the route).
    // If the watcher also fired its own reload+broadcast (the bug), we'd see 2.
    assert_eq!(
        event_count, 1,
        "expected exactly 1 event from persist_and_broadcast; if > 1, the watcher is firing for own writes (bug)"
    );
}
