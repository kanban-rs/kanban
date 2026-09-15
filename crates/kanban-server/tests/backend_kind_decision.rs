use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn test_json_content_at_db_extension_is_not_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("board.db");

    {
        let backend: Arc<dyn KanbanBackend> =
            Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(&path))));
        let mut ctx = KanbanContext::open(backend, AppConfig::default())
            .await
            .unwrap();
        ctx.create_board("Seed".to_string(), None).unwrap();
        ctx.save().await.unwrap();
    }
    assert!(path.exists());

    assert!(!kanban_server::stores::registered_store_manager().is_sqlite(path.to_str().unwrap()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_content_at_db_extension_is_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("board.db");

    {
        let _backend = kanban_persistence_sqlite::SqliteBackend::open(path.to_str().unwrap())
            .await
            .unwrap();
    }
    assert!(path.exists());

    assert!(kanban_server::stores::registered_store_manager().is_sqlite(path.to_str().unwrap()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_plain_json_locator_is_not_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("board.json");

    {
        let backend: Arc<dyn KanbanBackend> =
            Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(&path))));
        let mut ctx = KanbanContext::open(backend, AppConfig::default())
            .await
            .unwrap();
        ctx.create_board("Seed".to_string(), None).unwrap();
        ctx.save().await.unwrap();
    }
    assert!(path.exists());

    assert!(!kanban_server::stores::registered_store_manager().is_sqlite(path.to_str().unwrap()));
}
