use kanban_domain::KanbanOperations;
use kanban_service::StoreManager;
use kanban_tui::App;
use std::fs;
use tempfile::tempdir;

fn test_store_manager() -> StoreManager {
    let mut registry = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    registry.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    StoreManager::new(registry, backends)
}

#[test]
fn test_export_single_board() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_export.json");

    let mut app = App::test_default();

    let board = app
        .ctx
        .create_board("Test Board".to_string(), None)
        .unwrap();
    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    app.ctx
        .create_card(
            board.id,
            column.id,
            "Test Task".to_string(),
            Default::default(),
        )
        .unwrap();
    app.board_list.inner_mut().set_selected_index(Some(0));
    app.input.set(file_path.to_str().unwrap().to_string());
    app.reload_model();
    app.prepare_frame();

    app.export_board_with_filename().unwrap();

    let content = fs::read_to_string(&file_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert!(parsed.get("boards").is_some());
    let boards = parsed["boards"].as_array().unwrap();
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0]["board"]["name"], "Test Board");
    assert_eq!(boards[0]["columns"].as_array().unwrap().len(), 1);
    assert_eq!(boards[0]["cards"].as_array().unwrap().len(), 1);
    assert_eq!(boards[0]["cards"][0]["title"], "Test Task");
}

#[test]
fn test_export_all_boards() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_export_all.json");

    let mut app = App::test_default();

    let board1 = app.ctx.create_board("Board 1".to_string(), None).unwrap();
    let column1 = app
        .ctx
        .create_column(board1.id, "Todo".to_string(), None)
        .unwrap();
    app.ctx
        .create_card(
            board1.id,
            column1.id,
            "Task 1".to_string(),
            Default::default(),
        )
        .unwrap();

    let board2 = app.ctx.create_board("Board 2".to_string(), None).unwrap();
    let column2 = app
        .ctx
        .create_column(board2.id, "Todo".to_string(), None)
        .unwrap();
    app.ctx
        .create_card(
            board2.id,
            column2.id,
            "Task 2".to_string(),
            Default::default(),
        )
        .unwrap();

    app.input.set(file_path.to_str().unwrap().to_string());
    app.reload_model();
    app.prepare_frame();

    app.export_all_boards_with_filename().unwrap();

    let content = fs::read_to_string(&file_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert!(parsed.get("boards").is_some());
    let boards = parsed["boards"].as_array().unwrap();
    assert_eq!(boards.len(), 2);
    assert_eq!(boards[0]["board"]["name"], "Board 1");
    assert_eq!(boards[1]["board"]["name"], "Board 2");
}

#[test]
fn test_export_empty_boards() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_empty.json");

    let mut app = App::test_default();
    app.persistence.save_file = Some(file_path.to_str().unwrap().to_string());
    app.reload_model();
    app.prepare_frame();

    app.auto_save().unwrap();

    let content = fs::read_to_string(&file_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert!(parsed.get("boards").is_some());
    let boards = parsed["boards"].as_array().unwrap();
    assert_eq!(boards.len(), 0);
}

#[test]
fn test_import_valid_format() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_import.json");

    let json = r#"{
        "boards": [{
            "board": {
                "id": "00000000-0000-0000-0000-000000000001",
                "name": "Imported Board",
                "description": null,
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-01T00:00:00Z"
            },
            "columns": [{
                "id": "00000000-0000-0000-0000-000000000002",
                "board_id": "00000000-0000-0000-0000-000000000001",
                "name": "Todo",
                "position": 0,
                "wip_limit": null,
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-01T00:00:00Z"
            }],
            "cards": [{
                "id": "00000000-0000-0000-0000-000000000003",
                "column_id": "00000000-0000-0000-0000-000000000002",
                "title": "Imported Task",
                "description": null,
                "priority": "Medium",
                "status": "Todo",
                "position": 0,
                "due_date": null,
                "points": null,
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-01T00:00:00Z"
            }],
            "archived_cards": [],
            "sprints": []
        }]
    }"#;

    fs::write(&file_path, json).unwrap();

    let mut app = App::test_default();
    app.import_board_from_file(file_path.to_str().unwrap())
        .unwrap();

    app.reload_model();
    app.prepare_frame();
    assert_eq!(app.model.boards().len(), 1);
    assert_eq!(app.model.boards()[0].name, "Imported Board");
    assert_eq!(app.model.columns().len(), 1);
    assert_eq!(app.model.all_cards().len(), 1);
    assert_eq!(app.model.all_cards()[0].title, "Imported Task");
}

#[test]
fn test_import_invalid_format_fails() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_invalid.json");

    let json = r#"{"invalid": "format"}"#;
    fs::write(&file_path, json).unwrap();

    let mut app = App::test_default();
    let result = app.import_board_from_file(file_path.to_str().unwrap());

    assert!(result.is_err());
}

#[tokio::test]
async fn test_auto_save() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_autosave.json");

    let (mut app, _rx) = App::new(Some(file_path.to_str().unwrap().to_string()))
        .await
        .unwrap();

    let board = app
        .ctx
        .create_board("Auto Save Board".to_string(), None)
        .unwrap();
    app.ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();

    app.reload_model();
    app.prepare_frame();
    app.auto_save().unwrap();

    let content = fs::read_to_string(&file_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(parsed["boards"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["boards"][0]["board"]["name"], "Auto Save Board");
}

#[tokio::test]
async fn test_failed_import_returns_error() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_bad.json");

    let json = r#"{"boards": [{"invalid": true}]}"#;
    fs::write(&file_path, json).unwrap();

    // An invalid JSON file causes App::new to fail before the TUI starts,
    // preventing any risk of overwriting the file with empty data.
    let result = App::new(Some(file_path.to_str().unwrap().to_string())).await;
    assert!(
        result.is_err(),
        "App::new should fail for a JSON file with invalid board data"
    );
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_async_load_initial_state_sqlite() {
    use kanban_domain::{Board, Column, DataStore};

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_load.db");

    // Create and populate a SQLite store
    let store = kanban_persistence_sqlite::SqliteStore::open(db_path.to_str().unwrap())
        .await
        .unwrap();

    let board = Board::new("SQLite Board", None::<String>);
    let column = Column::new(board.id, "Backlog", 0);

    let snapshot = kanban_domain::Snapshot {
        archived_boards: Vec::new(),
        boards: vec![board.clone()],
        columns: vec![column.clone()],
        cards: vec![],
        archived_cards: vec![],
        sprints: vec![],
        graph: Default::default(),
    };
    store.apply_snapshot(snapshot).unwrap();
    drop(store);

    let sm = test_store_manager();
    let (mut app, _rx) = App::new_with_store(sm, Some(db_path.to_str().unwrap().to_string()))
        .await
        .unwrap();

    app.load_initial_state().await;
    app.reload_model();
    app.prepare_frame();
    assert_eq!(app.model.boards().len(), 1);
    assert_eq!(app.model.boards()[0].name, "SQLite Board");
    assert_eq!(app.model.columns().len(), 1);
    assert_eq!(app.model.columns()[0].name, "Backlog");
}

#[test]
fn test_export_import_sprint_and_card_prefixes() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_prefixes.json");

    // Create board with both sprint_prefix and card_prefix
    let mut app = App::test_default();
    use kanban_domain::{BoardUpdate, FieldUpdate, SprintUpdate};
    let board = app
        .ctx
        .create_board("Prefix Board".to_string(), None)
        .unwrap();
    app.ctx
        .update_board(
            board.id,
            BoardUpdate {
                sprint_prefix: FieldUpdate::Set("sprint".to_string()),
                card_prefix: FieldUpdate::Set("task".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    let column = app
        .ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    app.ctx
        .create_card(
            board.id,
            column.id,
            "Test Card".to_string(),
            Default::default(),
        )
        .unwrap();

    // Create sprint with card_prefix override
    let sprint = app.ctx.create_sprint(board.id, None, None).unwrap();
    app.ctx
        .update_sprint(
            sprint.id,
            SprintUpdate {
                card_prefix: FieldUpdate::Set("hotfix".to_string()),
                ..Default::default()
            },
        )
        .unwrap();

    app.board_list.inner_mut().set_selected_index(Some(0));
    app.input.set(file_path.to_str().unwrap().to_string());
    app.reload_model();
    app.prepare_frame();

    // Export
    app.export_board_with_filename().unwrap();
    let content = fs::read_to_string(&file_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Verify prefixes in exported JSON
    assert_eq!(parsed["boards"][0]["board"]["sprint_prefix"], "sprint");
    assert_eq!(parsed["boards"][0]["board"]["card_prefix"], "task");
    assert_eq!(parsed["boards"][0]["sprints"][0]["card_prefix"], "hotfix");

    // Clear and reimport
    let mut app2 = App::test_default();
    app2.import_board_from_file(file_path.to_str().unwrap())
        .unwrap();

    // Verify prefixes preserved after import
    app2.reload_model();
    app2.prepare_frame();
    assert_eq!(app2.model.boards().len(), 1);
    assert_eq!(
        app2.model.boards()[0].sprint_prefix,
        Some("sprint".to_string())
    );
    assert_eq!(app2.model.boards()[0].card_prefix, Some("task".to_string()));
    assert_eq!(app2.model.sprints().len(), 1);
    assert_eq!(
        app2.model.sprints()[0].card_prefix,
        Some("hotfix".to_string())
    );
}

#[test]
fn test_backward_compat_old_export_format() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_old_format.json");

    // Create old format JSON with branch_prefix instead of sprint_prefix
    let old_json = r#"{
        "boards": [{
            "board": {
                "id": "00000000-0000-0000-0000-000000000001",
                "name": "Old Board",
                "description": null,
                "branch_prefix": "FEAT",
                "sprint_duration_days": null,
                "next_card_number": 1,
                "task_sort_field": "Default",
                "task_sort_order": "Ascending",
                "sprint_names": [],
                "sprint_name_used_count": 0,
                "next_sprint_number": 1,
                "active_sprint_id": null,
                "task_list_view": "Flat",
                "prefix_counters": {},
                "sprint_counters": {},
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-01T00:00:00Z"
            },
            "columns": [{
                "id": "00000000-0000-0000-0000-000000000002",
                "board_id": "00000000-0000-0000-0000-000000000001",
                "name": "Todo",
                "position": 0,
                "wip_limit": null,
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-01T00:00:00Z"
            }],
            "cards": [{
                "id": "00000000-0000-0000-0000-000000000003",
                "column_id": "00000000-0000-0000-0000-000000000002",
                "title": "Old Card",
                "description": null,
                "priority": "Medium",
                "status": "Todo",
                "position": 0,
                "due_date": null,
                "points": null,
                "card_number": 1,
                "sprint_id": null,
                "assigned_prefix": "task",
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-01T00:00:00Z",
                "completed_at": null,
                "sprint_logs": []
            }],
            "archived_cards": [],
            "sprints": []
        }]
    }"#;

    fs::write(&file_path, old_json).unwrap();

    // Import old format
    let mut app = App::test_default();
    app.import_board_from_file(file_path.to_str().unwrap())
        .unwrap();

    // Verify board imported and old branch_prefix is mapped to sprint_prefix
    app.reload_model();
    app.prepare_frame();
    assert_eq!(app.model.boards().len(), 1);
    assert_eq!(app.model.boards()[0].name, "Old Board");
    assert_eq!(
        app.model.boards()[0].sprint_prefix,
        Some("FEAT".to_string())
    );
    // card_prefix should be None since old format didn't have it
    assert_eq!(app.model.boards()[0].card_prefix, None);

    // Verify cards still work
    assert_eq!(app.model.all_cards().len(), 1);
    assert_eq!(app.model.all_cards()[0].title, "Old Card");
}

#[test]
fn test_import_column_missing_default_status_key_defaults_to_none() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_missing_default_status.json");

    // A column with no `default_status` key at all, as written by any build
    // predating that field. The export/import format carries no version
    // envelope and is never migrated, so this must stay importable forever.
    let json = r#"{
        "boards": [{
            "board": {
                "id": "00000000-0000-0000-0000-000000000001",
                "name": "Pre-Default-Status Board",
                "description": null,
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-01T00:00:00Z"
            },
            "columns": [{
                "id": "00000000-0000-0000-0000-000000000002",
                "board_id": "00000000-0000-0000-0000-000000000001",
                "name": "Doing",
                "position": 0,
                "wip_limit": null,
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-01T00:00:00Z"
            }],
            "cards": [],
            "archived_cards": [],
            "sprints": []
        }]
    }"#;

    fs::write(&file_path, json).unwrap();

    let mut app = App::test_default();
    app.import_board_from_file(file_path.to_str().unwrap())
        .unwrap();

    app.reload_model();
    app.prepare_frame();
    assert_eq!(app.model.boards().len(), 1);
    assert_eq!(app.model.columns().len(), 1);
    assert_eq!(app.model.columns()[0].default_status, None);
}
