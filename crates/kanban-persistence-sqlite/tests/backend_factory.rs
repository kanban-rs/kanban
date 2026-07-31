use kanban_backend::{KanbanBackend, KanbanBackendFactory};
use kanban_core::AppConfig;
use kanban_persistence_sqlite::SqliteBackendFactory;

#[test]
fn test_sqlite_factory_reports_its_backend_name() {
    assert_eq!(SqliteBackendFactory.name(), "sqlite");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_factory_creates_backend_from_locator() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("factory.sqlite3");

    let backend: std::sync::Arc<dyn KanbanBackend> = SqliteBackendFactory
        .create(path.to_str().unwrap(), &AppConfig::default())
        .await
        .expect("factory creates a sqlite backend");

    assert!(backend.list_boards().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_factory_propagates_open_error_for_unusable_locator() {
    let dir = tempfile::tempdir().unwrap();
    let unusable = dir.path().join("no-such-dir").join("x.sqlite3");

    let err = SqliteBackendFactory
        .create(unusable.to_str().unwrap(), &AppConfig::default())
        .await
        .err()
        .expect("opening under a missing directory must fail");

    assert!(!err.to_string().is_empty());
}
