use kanban_backend::{KanbanBackend, KanbanBackendFactory};
use kanban_core::AppConfig;
use kanban_persistence_json::JsonBackendFactory;

#[test]
fn test_json_factory_reports_its_backend_name() {
    assert_eq!(JsonBackendFactory.name(), "json");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_json_factory_creates_backend_from_locator() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("board.json");

    let backend: std::sync::Arc<dyn KanbanBackend> = JsonBackendFactory
        .create(path.to_str().unwrap(), &AppConfig::default())
        .await
        .expect("factory creates a json backend");

    assert!(backend.list_boards().unwrap().is_empty());
}

/// Pins the documented lazy-load contract (`json_backend.rs:20-22`): the file is
/// not read or written until the first `DataStore`/`CommandStore` call.
#[tokio::test(flavor = "multi_thread")]
async fn test_json_factory_creates_backend_without_touching_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("untouched.json");

    let _backend = JsonBackendFactory
        .create(path.to_str().unwrap(), &AppConfig::default())
        .await
        .expect("factory creates a json backend");

    assert!(!path.exists(), "create must not touch disk");
}

#[test]
fn test_json_backend_factory_matches_locator_as_catch_all() {
    assert!(JsonBackendFactory.matches_locator("board.json", b"{\"boards\":[]}"));
    assert!(JsonBackendFactory.matches_locator("board.txt", b"[1,2,3]"));
    assert!(JsonBackendFactory.matches_locator("board.json", b"   {\"boards\":[]}"));
    assert!(JsonBackendFactory.matches_locator("/nonexistent/board.json", &[]));
}
