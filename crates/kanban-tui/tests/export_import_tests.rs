use kanban_tui::App;
use kanban_domain::{Board, Column, Card};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_export_single_board() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_export.json");

    let mut app = App::new(None);

    let board = Board::new("Test Board".to_string(), None);
    let column = Column::new(board.id, "Todo".to_string(), 0);
    let card = Card::new(column.id, "Test Task".to_string(), 0);

    app.boards.push(board.clone());
    app.columns.push(column.clone());
    app.cards.push(card.clone());
    app.board_selection.set(Some(0));
    app.input.set(file_path.to_str().unwrap().to_string());

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

    let mut app = App::new(None);

    let board1 = Board::new("Board 1".to_string(), None);
    let column1 = Column::new(board1.id, "Todo".to_string(), 0);
    let card1 = Card::new(column1.id, "Task 1".to_string(), 0);

    let board2 = Board::new("Board 2".to_string(), None);
    let column2 = Column::new(board2.id, "Todo".to_string(), 0);
    let card2 = Card::new(column2.id, "Task 2".to_string(), 0);

    app.boards.push(board1);
    app.boards.push(board2);
    app.columns.push(column1);
    app.columns.push(column2);
    app.cards.push(card1);
    app.cards.push(card2);
    app.input.set(file_path.to_str().unwrap().to_string());

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

    let mut app = App::new(None);
    app.save_file = Some(file_path.to_str().unwrap().to_string());

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
            }]
        }]
    }"#;

    fs::write(&file_path, json).unwrap();

    let mut app = App::new(None);
    app.import_board_from_file(file_path.to_str().unwrap()).unwrap();

    assert_eq!(app.boards.len(), 1);
    assert_eq!(app.boards[0].name, "Imported Board");
    assert_eq!(app.columns.len(), 1);
    assert_eq!(app.cards.len(), 1);
    assert_eq!(app.cards[0].title, "Imported Task");
}

#[test]
fn test_import_invalid_format_fails() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_invalid.json");

    let json = r#"{"invalid": "format"}"#;
    fs::write(&file_path, json).unwrap();

    let mut app = App::new(None);
    let result = app.import_board_from_file(file_path.to_str().unwrap());

    assert!(result.is_err());
}

#[test]
fn test_auto_save() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_autosave.json");

    let mut app = App::new(Some(file_path.to_str().unwrap().to_string()));

    let board = Board::new("Auto Save Board".to_string(), None);
    let column = Column::new(board.id, "Todo".to_string(), 0);
    app.boards.push(board);
    app.columns.push(column);

    app.auto_save().unwrap();

    let content = fs::read_to_string(&file_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(parsed["boards"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["boards"][0]["board"]["name"], "Auto Save Board");
}

#[test]
fn test_failed_import_clears_save_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_bad.json");

    let json = r#"{"boards": [{"invalid": true}]}"#;
    fs::write(&file_path, json).unwrap();

    let app = App::new(Some(file_path.to_str().unwrap().to_string()));

    assert!(app.save_file.is_none());
}
