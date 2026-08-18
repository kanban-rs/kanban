mod helpers;

use kanban_tui::App;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_import_board_from_file_refreshes_the_whole_model_without_a_further_reload() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("import.json");

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

    assert_eq!(app.model.boards().len(), 1);
    assert_eq!(app.model.boards()[0].name, "Imported Board");
    assert_eq!(app.model.columns().len(), 1);
    assert_eq!(app.model.all_cards().len(), 1);
    assert_eq!(app.model.all_cards()[0].title, "Imported Task");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_storage_location_change_refreshes_the_model_without_a_further_reload() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = helpers::setup_app_with_json_file(dir.path()).await;
    assert_eq!(app.model.boards()[0].name, "OriginalBoard");

    let sqlite_path =
        helpers::create_test_sqlite_file(dir.path(), "other.db", &["SqliteBoard"]).await;

    let old_config = app.app_config.clone();
    let old_storage_location = app.app_config.effective_storage_location();
    app.app_config.storage_location = Some(sqlite_path.clone());

    app.apply_storage_location_change(old_config, &old_storage_location);
    app.await_migration().await;

    assert_eq!(app.model.boards().len(), 1);
    assert_eq!(app.model.boards()[0].name, "SqliteBoard");
    assert_eq!(
        app.persistence.save_file.as_deref(),
        Some(sqlite_path.as_str())
    );
}
