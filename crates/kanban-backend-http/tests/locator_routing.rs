use kanban_backend::KanbanBackendRegistry;
use kanban_backend_http::HttpBackendFactory;
use kanban_persistence_json::JsonBackendFactory;
use kanban_persistence_sqlite::SqliteBackendFactory;

fn registry() -> KanbanBackendRegistry {
    let mut registry = KanbanBackendRegistry::new();
    registry.register(Box::new(SqliteBackendFactory));
    registry.register(Box::new(JsonBackendFactory));
    registry.register(Box::new(HttpBackendFactory));
    registry
}

#[test]
fn test_the_registry_selects_the_http_backend_for_a_url() {
    let registry = registry();
    let factory = registry.for_locator("http://127.0.0.1:3000").unwrap();
    assert_eq!(factory.name(), "http");
}

#[test]
fn test_the_registry_selects_the_http_backend_for_a_url_ending_in_db() {
    let registry = registry();
    let factory = registry
        .for_locator("http://127.0.0.1:3000/board.db")
        .unwrap();
    assert_eq!(factory.name(), "http");
}

#[test]
fn test_the_registry_still_selects_json_and_sqlite_for_local_paths() {
    let registry = registry();
    let dir = tempfile::tempdir().unwrap();

    let json_path = dir.path().join("board.json");
    std::fs::write(&json_path, b"{\"boards\":[]}").unwrap();
    let json_factory = registry.for_locator(json_path.to_str().unwrap()).unwrap();
    assert_eq!(json_factory.name(), "json");

    let sqlite_path = dir.path().join("nonexistent.sqlite");
    let sqlite_factory = registry.for_locator(sqlite_path.to_str().unwrap()).unwrap();
    assert_eq!(sqlite_factory.name(), "sqlite");
}
