use kanban_domain::DataStore;
use kanban_persistence::StoreRegistry;
use kanban_service::StoreManager;

fn manager() -> StoreManager {
    let mut registry = StoreRegistry::new();
    registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    StoreManager::new(registry, kanban_backend::KanbanBackendRegistry::new())
}

fn create_test_json(dir: &std::path::Path, name: &str, boards: &[&str]) -> String {
    let path = dir.join(name);
    let board_objects: Vec<serde_json::Value> = boards
        .iter()
        .map(|name| {
            serde_json::json!({
                "id": uuid::Uuid::new_v4().to_string(),
                "name": name,
                "column_ids": [],
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z",
                "task_sort_field": "Default",
                "task_sort_order": "Ascending"
            })
        })
        .collect();
    let data = serde_json::json!({
        "boards": board_objects,
        "columns": [],
        "cards": [],
        "archived_cards": [],
        "sprints": [],
        "graph": { "cards": { "edges": [] } }
    });
    std::fs::write(&path, serde_json::to_string_pretty(&data).unwrap()).unwrap();
    path.to_str().unwrap().to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_validate_store_readable_valid_json_returns_ok() {
    let dir = tempfile::tempdir().unwrap();
    let path = create_test_json(dir.path(), "board.json", &["Board1"]);

    manager()
        .validate_store_readable("json", &path)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_validate_store_readable_nonexistent_file_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nonexistent.json");

    let err = manager()
        .validate_store_readable("json", path.to_str().unwrap())
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("does not exist") || err.to_string().contains("not found"),
        "got: {}",
        err
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_validate_store_readable_invalid_json_content_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.json");
    std::fs::write(&path, "hello world").unwrap();

    let err = manager()
        .validate_store_readable("json", path.to_str().unwrap())
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("serialization") || msg.contains("parse") || msg.contains("invalid"),
        "expected a parse/serialization error, got: {}",
        msg
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_validate_store_readable_empty_file_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.json");
    std::fs::write(&path, "").unwrap();

    let err = manager()
        .validate_store_readable("json", path.to_str().unwrap())
        .await
        .unwrap_err();
    assert!(!err.to_string().is_empty(), "expected error, got: {}", err);
}

#[test]
fn test_storage_location_with_dotdot_fails_validation() {
    let config = kanban_core::AppConfig {
        storage_location: Some("../../foo".to_string()),
        ..Default::default()
    };
    let result = kanban_service::config::validate(&config);
    assert!(
        result.is_err(),
        "expected validation error for '..' in path"
    );
    let err = result.unwrap_err().to_string();
    assert!(err.contains(".."), "error should mention '..': {}", err);
}

#[test]
fn test_storage_location_with_dotdot_in_filename_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let location = dir.path().join("my..file.json");
    let config = kanban_core::AppConfig {
        storage_location: Some(location.to_string_lossy().into_owned()),
        ..Default::default()
    };
    let result = kanban_service::config::validate(&config);
    assert!(
        result.is_ok(),
        "expected no error for '..' in filename (not a path component): {:?}",
        result
    );
}

#[test]
fn test_storage_location_with_nested_dotdot_fails_validation() {
    let config = kanban_core::AppConfig {
        storage_location: Some("data/../../../etc".to_string()),
        ..Default::default()
    };
    let result = kanban_service::config::validate(&config);
    assert!(
        result.is_err(),
        "expected validation error for '..' in path"
    );
    let err = result.unwrap_err().to_string();
    assert!(err.contains(".."), "error should mention '..': {}", err);
}

async fn create_test_sqlite(dir: &std::path::Path, name: &str, boards: &[&str]) -> String {
    use kanban_persistence::PersistenceStore;
    use kanban_persistence_sqlite::SqliteStore;

    let path = dir.join(name);
    let path_str = path.to_str().unwrap().to_string();
    let store = SqliteStore::open(&path_str).await.unwrap();

    for name in boards {
        store
            .upsert_board(kanban_domain::Board::new(name.to_string(), None::<String>))
            .unwrap();
    }
    // Checkpoint (WAL -> base file, truncating the WAL) and close the pool so
    // a later on-disk corruption of the base file is actually observed by
    // the next open, instead of being masked by a live WAL or a pool
    // connection still holding valid cached pages.
    store.checkpoint().await.unwrap();
    store.close().await;

    path_str
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_validate_store_readable_valid_sqlite_returns_ok() {
    let dir = tempfile::tempdir().unwrap();
    let path = create_test_sqlite(dir.path(), "board.sqlite", &["Board1"]).await;

    manager()
        .validate_store_readable("sqlite", &path)
        .await
        .unwrap();
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_validate_store_readable_corrupt_sqlite_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = create_test_sqlite(dir.path(), "board.sqlite", &["Board1"]).await;

    // Overwrite the whole file (including the 16-byte "SQLite format 3\0"
    // magic) with garbage.
    let len = std::fs::metadata(&path).unwrap().len() as usize;
    std::fs::write(&path, vec![0xFFu8; len.max(200)]).unwrap();

    let err = manager()
        .validate_store_readable("sqlite", &path)
        .await
        .unwrap_err();
    assert!(!err.to_string().is_empty(), "expected error, got: {}", err);
}
