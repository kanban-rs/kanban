use kanban_core::AppConfig;
use kanban_persistence::StoreRegistry;
use kanban_service::StoreManager;

fn manager() -> StoreManager {
    let mut registry = StoreRegistry::new();
    registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    StoreManager::new(registry, kanban_backend::KanbanBackendRegistry::new())
}

#[test]
fn test_make_store_json_backend() {
    let store = manager()
        .make_store("json", "/tmp/test_board.json")
        .unwrap();
    assert!(store.path().to_str().unwrap().ends_with(".json"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_make_store_json_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test_board");
    let store = manager()
        .make_store("json", path.to_str().unwrap())
        .unwrap();

    let data = serde_json::json!({
        "boards": [],
        "columns": [],
        "cards": [],
        "archived_cards": [],
        "sprints": [],
        "graph": { "cards": { "edges": [] } }
    });
    let snapshot = kanban_persistence::StoreSnapshot {
        data: serde_json::to_vec(&data).unwrap(),
        metadata: kanban_persistence::PersistenceMetadata::new(store.instance_id()),
    };
    store.save(snapshot).await.unwrap();

    let (loaded, _meta) = store.load().await.unwrap();
    let loaded_data: serde_json::Value = serde_json::from_slice(&loaded.data).unwrap();
    assert!(loaded_data["boards"].is_array());
}

#[test]
fn test_make_store_unknown_backend_returns_error() {
    let result = manager().make_store("txt", "/tmp/test_board.txt");
    match result {
        Ok(_) => panic!("Expected error for unknown backend"),
        Err(err) => {
            let msg = err.to_string();
            assert!(
                msg.contains("No backend registered for"),
                "Expected no backend error, got: {msg}"
            );
        }
    }
}

#[test]
fn test_make_store_with_config_explicit_path_wins() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("explicit.json");
    let path_str = path.to_str().unwrap().to_string();
    let config = AppConfig::default();
    let store = manager()
        .make_store_with_config(Some(&path_str), &config)
        .unwrap();
    assert!(store.path().to_str().unwrap().ends_with(".json"));
}

#[test]
fn test_make_store_with_config_none_uses_json_default() {
    let config = AppConfig::default();
    let store = manager().make_store_with_config(None, &config).unwrap();
    assert!(store.path().to_str().unwrap().ends_with("boards.json"));
}
