use kanban_core::AppConfig;
use kanban_domain::KanbanResult;
use kanban_persistence::StoreRegistry;
use kanban_service::{KanbanContext, StoreManager};

fn manager() -> StoreManager {
    let mut registry = StoreRegistry::new();
    registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    StoreManager::new(registry, kanban_backend::KanbanBackendRegistry::new())
}

async fn open_context(locator: &str, config: AppConfig) -> KanbanResult<KanbanContext> {
    let mut config = config;
    let mut stores = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    stores.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    stores.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    let sm = kanban_service::StoreManager::new(stores, backends);
    sm.sync_backend_with_file(locator, &mut config);
    let backend = sm.make_backend(locator, &config).await?;
    KanbanContext::open(backend, config).await
}

fn now() -> &'static str {
    "2024-01-01T00:00:00Z"
}

fn write_json(dir: &std::path::Path, name: &str, data: serde_json::Value) -> String {
    let path = dir.join(name);
    std::fs::write(&path, serde_json::to_string_pretty(&data).unwrap()).unwrap();
    path.to_str().unwrap().to_string()
}

fn create_test_json(dir: &std::path::Path, name: &str) -> String {
    write_json(
        dir,
        name,
        serde_json::json!({
            "boards": [],
            "columns": [],
            "cards": [],
            "archived_cards": [],
            "sprints": [],
            "graph": { "cards": { "edges": [] } }
        }),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_store_json_to_json_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let from = create_test_json(dir.path(), "source.json");
    let to = dir.path().join("target.json");
    let to_str = to.to_str().unwrap();

    manager()
        .migrate_store("json", &from, "json", to_str)
        .await
        .unwrap();
    assert!(to.exists());
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_store_json_to_sqlite() {
    let dir = tempfile::tempdir().unwrap();
    let from = create_test_json(dir.path(), "source.json");
    let to = dir.path().join("target.sqlite");
    let to_str = to.to_str().unwrap();

    manager()
        .migrate_store("json", &from, "sqlite", to_str)
        .await
        .unwrap();
    assert!(to.exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_store_fails_if_target_exists() {
    let dir = tempfile::tempdir().unwrap();
    let from = create_test_json(dir.path(), "source.json");
    let to = create_test_json(dir.path(), "target.json");

    let err = manager()
        .migrate_store("json", &from, "json", &to)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("already exists"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_store_fails_if_source_missing() {
    let dir = tempfile::tempdir().unwrap();
    let from = dir.path().join("nonexistent.json");
    let to = dir.path().join("target.json");

    let err = manager()
        .migrate_store("json", from.to_str().unwrap(), "json", to.to_str().unwrap())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not found"));
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_store_repairs_dangling_sprint_id() {
    let dir = tempfile::tempdir().unwrap();
    let board_id = uuid::Uuid::new_v4().to_string();
    let col_id = uuid::Uuid::new_v4().to_string();
    let card_id = uuid::Uuid::new_v4().to_string();
    let ghost_sprint_id = uuid::Uuid::new_v4().to_string();

    let from = write_json(
        dir.path(),
        "source.json",
        serde_json::json!({
            "boards": [{ "id": board_id, "name": "B",
                "task_sort_field": "Default", "task_sort_order": "Ascending",
                "sprint_name_used_count": 0, "next_sprint_number": 1,
                "task_list_view": "Flat", "prefix_counters": {}, "sprint_counters": {},
                "created_at": now(), "updated_at": now() }],
            "columns": [{ "id": col_id, "board_id": board_id, "name": "TODO",
                "position": 0, "created_at": now(), "updated_at": now() }],
            "sprints": [],
            "cards": [{ "id": card_id, "column_id": col_id, "title": "Orphaned",
                "priority": "Medium", "status": "Todo", "position": 0, "card_number": 1,
                "sprint_id": ghost_sprint_id,
                "sprint_logs": [], "created_at": now(), "updated_at": now() }],
            "archived_cards": [],
            "graph": { "cards": { "edges": [] } }
        }),
    );
    let to = dir.path().join("out.sqlite");

    manager()
        .migrate_store("json", &from, "sqlite", to.to_str().unwrap())
        .await
        .unwrap();

    let ctx = open_context(to.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    let cards = ctx.cards().unwrap();
    assert_eq!(cards.len(), 1, "card should be present");
    assert!(
        cards[0].sprint_id.is_none(),
        "dangling sprint_id should be nulled out"
    );
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_store_repairs_orphaned_column_id() {
    let dir = tempfile::tempdir().unwrap();
    let board_id = uuid::Uuid::new_v4().to_string();
    let valid_col_id = uuid::Uuid::new_v4().to_string();
    let ghost_col_id = uuid::Uuid::new_v4().to_string();
    let card_id = uuid::Uuid::new_v4().to_string();

    let from = write_json(
        dir.path(),
        "source.json",
        serde_json::json!({
            "boards": [{ "id": board_id, "name": "B",
                "task_sort_field": "Default", "task_sort_order": "Ascending",
                "sprint_name_used_count": 0, "next_sprint_number": 1,
                "task_list_view": "Flat", "prefix_counters": {}, "sprint_counters": {},
                "created_at": now(), "updated_at": now() }],
            "columns": [{ "id": valid_col_id, "board_id": board_id, "name": "TODO",
                "position": 0, "created_at": now(), "updated_at": now() }],
            "sprints": [],
            "cards": [{ "id": card_id, "column_id": ghost_col_id, "title": "Orphaned",
                "priority": "Medium", "status": "Todo", "position": 0, "card_number": 1,
                "sprint_logs": [], "created_at": now(), "updated_at": now() }],
            "archived_cards": [],
            "graph": { "cards": { "edges": [] } }
        }),
    );
    let to = dir.path().join("out.sqlite");

    manager()
        .migrate_store("json", &from, "sqlite", to.to_str().unwrap())
        .await
        .unwrap();

    let ctx = open_context(to.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    let cards = ctx.cards().unwrap();
    assert_eq!(cards.len(), 1, "card should be present");
    let expected_col_id = uuid::Uuid::parse_str(&valid_col_id).unwrap();
    assert_eq!(
        cards[0].column_id, expected_col_id,
        "orphaned card should be moved to the first valid column"
    );
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_migrate_store_cleans_up_destination_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let ghost_col_id = uuid::Uuid::new_v4().to_string();
    let card_id = uuid::Uuid::new_v4().to_string();

    // Write JSON that is valid JSON but would produce a non-repairable save error
    // by having a card whose column_id references nothing and there are NO valid columns
    // (so the repair fallback has nothing to fall back to — save will fail)
    let from = write_json(
        dir.path(),
        "source.json",
        serde_json::json!({
            "boards": [],
            "columns": [],
            "sprints": [],
            "cards": [{ "id": card_id, "column_id": ghost_col_id, "title": "T",
                "priority": "Medium", "status": "Todo", "position": 0, "card_number": 1,
                "sprint_logs": [], "created_at": now(), "updated_at": now() }],
            "archived_cards": [],
            "graph": { "cards": { "edges": [] } }
        }),
    );
    let to = dir.path().join("out.sqlite");

    let result = manager()
        .migrate_store("json", &from, "sqlite", to.to_str().unwrap())
        .await;

    assert!(result.is_err(), "migration should fail");
    assert!(
        !to.exists(),
        "destination file should be cleaned up after failure"
    );
}
