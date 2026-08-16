use kanban_domain::{Board, Card, Column, Snapshot};
use kanban_tui::App;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_import_failure_prevents_empty_state_save() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("kanban.json");

    // Create a real board and then save it in V2 format
    let board = Board::new("Test Board", None::<String>);
    let column = Column::new(board.id, "Todo", 0);

    // Create snapshot with one board
    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: vec![board.clone()],
        columns: vec![column.clone()],
        cards: vec![],
        archived_cards: vec![],
        sprints: vec![],
        graph: kanban_domain::DependencyGraph::new(),
        prefixes: Vec::new(),
    };

    // Manually create V2 format JSON
    let snapshot_json = serde_json::to_value(&snapshot).unwrap();
    let v2_content = serde_json::json!({
        "version": 2,
        "metadata": {
            "instance_id": "00000000-0000-0000-0000-000000000001",
            "saved_at": chrono::Utc::now().to_rfc3339()
        },
        "data": snapshot_json
    });

    fs::write(
        &file_path,
        serde_json::to_string_pretty(&v2_content).unwrap(),
    )
    .unwrap();

    // Create app with the V2 format file - should handle it gracefully now
    let (mut app, _rx) = App::new(Some(file_path.to_str().unwrap().to_string()))
        .await
        .unwrap();
    app.load_initial_state().await;

    // App should load the board from V2 format
    app.reload_model();
    app.prepare_frame();
    assert_eq!(
        app.model.boards().len(),
        1,
        "V2 format should be imported successfully"
    );
    assert_eq!(app.model.boards()[0].name, "Test Board");
    assert!(
        app.persistence.save_file.is_some(),
        "save_file should still be enabled after successful V2 import"
    );
}

#[tokio::test]
async fn test_import_failure_disables_save_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("kanban.json");

    // Create an invalid JSON file
    fs::write(&file_path, "{ invalid json }").unwrap();

    // An invalid JSON file causes App::new to fail before the TUI starts,
    // preventing any risk of overwriting the file with empty data.
    let result = App::new(Some(file_path.to_str().unwrap().to_string())).await;
    assert!(
        result.is_err(),
        "App::new should fail for an invalid JSON file"
    );
}

#[tokio::test]
async fn test_v2_format_is_imported_correctly() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("kanban.json");

    // Create a real board
    let board = Board::new("My Project", None::<String>);
    let column = Column::new(board.id, "Todo", 0);
    let mut board_mut = board.clone();
    let card = Card::new(&mut board_mut, column.id, "Important Task", 0);

    // Create snapshot with board, column, and card
    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: vec![board],
        columns: vec![column],
        cards: vec![card],
        archived_cards: vec![],
        sprints: vec![],
        graph: kanban_domain::DependencyGraph::new(),
        prefixes: Vec::new(),
    };

    // Manually create V2 format JSON
    let snapshot_json = serde_json::to_value(&snapshot).unwrap();
    let v2_content = serde_json::json!({
        "version": 2,
        "metadata": {
            "instance_id": "00000000-0000-0000-0000-000000000002",
            "saved_at": chrono::Utc::now().to_rfc3339()
        },
        "data": snapshot_json
    });

    fs::write(
        &file_path,
        serde_json::to_string_pretty(&v2_content).unwrap(),
    )
    .unwrap();

    // Create app with V2 format file
    let (mut app, _rx) = App::new(Some(file_path.to_str().unwrap().to_string()))
        .await
        .unwrap();
    app.load_initial_state().await;

    // Should successfully import the board with its column and card
    app.reload_model();
    app.prepare_frame();
    assert_eq!(app.model.boards().len(), 1);
    assert_eq!(app.model.boards()[0].name, "My Project");
    assert_eq!(app.model.columns().len(), 1);
    assert_eq!(app.model.all_cards().len(), 1);
    assert_eq!(app.model.all_cards()[0].title, "Important Task");
    assert!(
        app.persistence.save_file.is_some(),
        "save_file should remain enabled after successful V2 import"
    );
}
