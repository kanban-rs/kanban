use assert_cmd::{cargo_bin_cmd, Command};
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use tempfile::tempdir;

fn kanban() -> Command {
    cargo_bin_cmd!("kanban")
}

fn kanban_no_config(dir: &std::path::Path) -> Command {
    let mut cmd = kanban();
    cmd.current_dir(dir)
        .env_remove("KANBAN_FILE")
        .env_remove("XDG_CONFIG_HOME")
        .env("HOME", dir);
    cmd
}

fn parse_json_output(output: &str) -> Value {
    serde_json::from_str(output).expect("Failed to parse JSON output")
}

fn extract_id(json: &Value) -> String {
    json["data"]["id"].as_str().unwrap().to_string()
}

mod board_tests {
    use super::*;

    #[test]
    fn test_board_create() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "Test Board",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["name"], "Test Board");
    }

    #[test]
    fn test_board_update_with_prefix() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "Test Board",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let board_id = extract_id(&create_json);

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "update",
                &board_id,
                "--card-prefix",
                "PROJ",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["card_prefix"], "PROJ");
    }

    #[test]
    fn test_board_list_empty() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        let output = kanban()
            .args([file.to_str().unwrap(), "board", "list"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["total"], 0);
    }

    #[test]
    fn test_board_list_with_data() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "Board 1",
            ])
            .assert()
            .success();

        kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "Board 2",
            ])
            .assert()
            .success();

        let output = kanban()
            .args([file.to_str().unwrap(), "board", "list"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["total"], 2);
    }

    #[test]
    fn test_board_get() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "Test Board",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let board_id = extract_id(&create_json);

        let output = kanban()
            .args([file.to_str().unwrap(), "board", "get", &board_id])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["name"], "Test Board");
    }

    #[test]
    fn test_board_update() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "Original",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let board_id = extract_id(&create_json);

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "update",
                &board_id,
                "--name",
                "Updated",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["name"], "Updated");
    }

    #[test]
    fn test_board_delete() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "To Delete",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let board_id = extract_id(&create_json);

        kanban()
            .args([file.to_str().unwrap(), "board", "delete", &board_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("\"deleted\""));

        let list_output = kanban()
            .args([file.to_str().unwrap(), "board", "list"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&list_output));
        assert_eq!(json["data"]["total"], 0);
    }
}

mod column_tests {
    use super::*;

    fn setup_board(file: &std::path::Path) -> String {
        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "Test Board",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        extract_id(&json)
    }

    #[test]
    fn test_column_create() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let board_id = setup_board(&file);

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "column",
                "create",
                "--board-id",
                &board_id,
                "--name",
                "TODO",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["name"], "TODO");
        assert_eq!(json["data"]["board_id"], board_id);
    }

    #[test]
    fn test_column_list() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let board_id = setup_board(&file);

        kanban()
            .args([
                file.to_str().unwrap(),
                "column",
                "create",
                "--board-id",
                &board_id,
                "--name",
                "TODO",
            ])
            .assert()
            .success();

        kanban()
            .args([
                file.to_str().unwrap(),
                "column",
                "create",
                "--board-id",
                &board_id,
                "--name",
                "DONE",
            ])
            .assert()
            .success();

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "column",
                "list",
                "--board-id",
                &board_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["total"], 2);
    }

    #[test]
    fn test_column_reorder() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let board_id = setup_board(&file);

        let col1_output = kanban()
            .args([
                file.to_str().unwrap(),
                "column",
                "create",
                "--board-id",
                &board_id,
                "--name",
                "First",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let col1_json = parse_json_output(&String::from_utf8_lossy(&col1_output));
        let col1_id = extract_id(&col1_json);

        kanban()
            .args([
                file.to_str().unwrap(),
                "column",
                "create",
                "--board-id",
                &board_id,
                "--name",
                "Second",
            ])
            .assert()
            .success();

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "column",
                "reorder",
                &col1_id,
                "--position",
                "1",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["position"], 1);
    }

    #[test]
    fn test_column_delete() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let board_id = setup_board(&file);

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "column",
                "create",
                "--board-id",
                &board_id,
                "--name",
                "To Delete",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let column_id = extract_id(&create_json);

        kanban()
            .args([file.to_str().unwrap(), "column", "delete", &column_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("\"deleted\""));
    }
}

mod card_tests {
    use super::*;

    fn setup_board_and_column(file: &std::path::Path) -> (String, String) {
        let board_output = kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "Test Board",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let board_json = parse_json_output(&String::from_utf8_lossy(&board_output));
        let board_id = extract_id(&board_json);

        let column_output = kanban()
            .args([
                file.to_str().unwrap(),
                "column",
                "create",
                "--board-id",
                &board_id,
                "--name",
                "TODO",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let column_json = parse_json_output(&String::from_utf8_lossy(&column_output));
        let column_id = extract_id(&column_json);

        (board_id, column_id)
    }

    #[test]
    fn test_card_create() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column(&file);

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Test Task",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["title"], "Test Task");
        assert_eq!(json["data"]["column_id"], column_id);
    }

    #[test]
    fn test_card_create_with_options() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column(&file);

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Test Task",
                "--description",
                "A test description",
                "--priority",
                "high",
                "--points",
                "5",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["description"], "A test description");
        assert_eq!(json["data"]["priority"], "High");
        assert_eq!(json["data"]["points"], 5);
    }

    #[test]
    fn test_card_list() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column(&file);

        kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Task 1",
            ])
            .assert()
            .success();

        kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Task 2",
            ])
            .assert()
            .success();

        let output = kanban()
            .args([file.to_str().unwrap(), "card", "list"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["total"], 2);
        assert!(json["data"]["total_pages"].is_number());
    }

    #[test]
    fn test_card_list_summary_omits_description() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column(&file);

        kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Task With Desc",
                "--description",
                "Hello description",
            ])
            .assert()
            .success();

        let output = kanban()
            .args([file.to_str().unwrap(), "card", "list"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(!json["data"]["items"][0]
            .as_object()
            .unwrap()
            .contains_key("description"));
    }

    #[test]
    fn test_card_list_pagination() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column(&file);

        for i in 1..=3 {
            kanban()
                .args([
                    file.to_str().unwrap(),
                    "card",
                    "create",
                    "--board-id",
                    &board_id,
                    "--column-id",
                    &column_id,
                    "--title",
                    &format!("Task {i}"),
                ])
                .assert()
                .success();
        }

        let page1_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "list",
                "--page-size",
                "2",
                "--page",
                "1",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let page1_json = parse_json_output(&String::from_utf8_lossy(&page1_output));
        assert_eq!(page1_json["data"]["total"], 3);
        assert_eq!(page1_json["data"]["total_pages"], 2);
        assert_eq!(page1_json["data"]["items"].as_array().unwrap().len(), 2);

        let page2_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "list",
                "--page-size",
                "2",
                "--page",
                "2",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let page2_json = parse_json_output(&String::from_utf8_lossy(&page2_output));
        assert_eq!(page2_json["data"]["items"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_card_list_out_of_bounds_page() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column(&file);

        kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Only Card",
            ])
            .assert()
            .success();

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "list",
                "--page-size",
                "5",
                "--page",
                "99",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert_eq!(json["data"]["items"].as_array().unwrap().len(), 0);
        assert_eq!(json["data"]["total"], 1);
    }

    #[test]
    fn test_card_update() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column(&file);

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Original",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let card_id = extract_id(&create_json);

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "update",
                &card_id,
                "--title",
                "Updated",
                "--priority",
                "critical",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["title"], "Updated");
        assert_eq!(json["data"]["priority"], "Critical");
    }

    #[test]
    fn test_card_move() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column(&file);

        let col2_output = kanban()
            .args([
                file.to_str().unwrap(),
                "column",
                "create",
                "--board-id",
                &board_id,
                "--name",
                "DONE",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let col2_json = parse_json_output(&String::from_utf8_lossy(&col2_output));
        let col2_id = extract_id(&col2_json);

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Task",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let card_id = extract_id(&create_json);

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "move",
                &card_id,
                "--column-id",
                &col2_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["column_id"], col2_id);
    }

    #[test]
    fn test_card_archive_and_restore() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column(&file);

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "To Archive",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let card_id = extract_id(&create_json);

        kanban()
            .args([file.to_str().unwrap(), "card", "archive", &card_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("\"archived\""));

        let archived_output = kanban()
            .args([file.to_str().unwrap(), "card", "list", "--archived"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let archived_json = parse_json_output(&String::from_utf8_lossy(&archived_output));
        assert_eq!(archived_json["data"]["total"], 1);
        assert!(archived_json["data"]["items"][0]["archived_at"].is_string());
        assert!(archived_json["data"]["items"][0]["original_column_id"].is_string());
        assert!(!archived_json["data"]["items"][0]["card"]
            .as_object()
            .unwrap()
            .contains_key("description"));

        let restore_output = kanban()
            .args([file.to_str().unwrap(), "card", "restore", &card_id])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let restore_json = parse_json_output(&String::from_utf8_lossy(&restore_output));
        assert!(restore_json["success"].as_bool().unwrap());
        assert_eq!(restore_json["data"]["title"], "To Archive");
    }

    #[test]
    fn test_card_branch_name() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column(&file);

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "My Feature Task",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let card_id = extract_id(&create_json);

        let output = kanban()
            .args([file.to_str().unwrap(), "card", "branch-name", &card_id])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert!(json["data"]["branch_name"]
            .as_str()
            .unwrap()
            .contains("my-feature-task"));
    }

    #[test]
    fn test_card_git_checkout() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column(&file);

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "My Task",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let card_id = extract_id(&create_json);

        let output = kanban()
            .args([file.to_str().unwrap(), "card", "git-checkout", &card_id])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert!(json["data"]["command"]
            .as_str()
            .unwrap()
            .starts_with("git checkout -b"));
    }

    #[test]
    fn test_card_archive_cards() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column(&file);

        let card1_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Task 1",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let card1_json = parse_json_output(&String::from_utf8_lossy(&card1_output));
        let card1_id = extract_id(&card1_json);

        let card2_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Task 2",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let card2_json = parse_json_output(&String::from_utf8_lossy(&card2_output));
        let card2_id = extract_id(&card2_json);

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "archive-cards",
                "--ids",
                &format!("{},{}", card1_id, card2_id),
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["succeeded_count"], 2);
        assert_eq!(json["data"]["failed_count"], 0);
    }

    #[test]
    fn test_card_move_cards() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column(&file);

        let col2_output = kanban()
            .args([
                file.to_str().unwrap(),
                "column",
                "create",
                "--board-id",
                &board_id,
                "--name",
                "DONE",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let col2_json = parse_json_output(&String::from_utf8_lossy(&col2_output));
        let col2_id = extract_id(&col2_json);

        let card1_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Task 1",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let card1_json = parse_json_output(&String::from_utf8_lossy(&card1_output));
        let card1_id = extract_id(&card1_json);

        let card2_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Task 2",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let card2_json = parse_json_output(&String::from_utf8_lossy(&card2_output));
        let card2_id = extract_id(&card2_json);

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "move-cards",
                "--ids",
                &format!("{},{}", card1_id, card2_id),
                "--column-id",
                &col2_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["succeeded_count"], 2);
        assert_eq!(json["data"]["failed_count"], 0);
    }

    fn setup_board_and_column_with_prefix(
        file: &std::path::Path,
        prefix: &str,
    ) -> (String, String) {
        let board_output = kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "Test Board",
                "--card-prefix",
                prefix,
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let board_json = parse_json_output(&String::from_utf8_lossy(&board_output));
        let board_id = extract_id(&board_json);

        let column_output = kanban()
            .args([
                file.to_str().unwrap(),
                "column",
                "create",
                "--board-id",
                &board_id,
                "--name",
                "TODO",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let column_json = parse_json_output(&String::from_utf8_lossy(&column_output));
        let column_id = extract_id(&column_json);

        (board_id, column_id)
    }

    #[test]
    fn test_card_get_by_prefix_identifier() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column_with_prefix(&file, "KAN");

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Prefix Test Card",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let card_uuid = extract_id(&create_json);

        let output = kanban()
            .args([file.to_str().unwrap(), "card", "get", "KAN-1"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["id"], card_uuid);
        assert_eq!(json["data"]["title"], "Prefix Test Card");
    }

    #[test]
    fn test_card_get_by_number_only() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column_with_prefix(&file, "KAN");

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Number Lookup Card",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let card_uuid = extract_id(&create_json);

        let output = kanban()
            .args([file.to_str().unwrap(), "card", "get", "1"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["id"], card_uuid);
        assert_eq!(json["data"]["title"], "Number Lookup Card");
    }

    #[test]
    fn test_card_archive_by_identifier() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column_with_prefix(&file, "KAN");

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Archive Me",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let card_uuid = extract_id(&create_json);

        let output = kanban()
            .args([file.to_str().unwrap(), "card", "archive", "KAN-1"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["archived"], card_uuid);
    }

    fn setup_two_boards_same_prefix(
        file: &std::path::Path,
    ) -> (String, String, String, String, String, String) {
        let (board_a_id, col_a_id) = setup_board_and_column_with_prefix(file, "KAN");
        let (board_b_id, col_b_id) = {
            let board_output = kanban()
                .args([
                    file.to_str().unwrap(),
                    "board",
                    "create",
                    "--name",
                    "Board B",
                    "--card-prefix",
                    "KAN",
                ])
                .assert()
                .success()
                .get_output()
                .stdout
                .clone();
            let board_json = parse_json_output(&String::from_utf8_lossy(&board_output));
            let board_b_id = extract_id(&board_json);
            let col_output = kanban()
                .args([
                    file.to_str().unwrap(),
                    "column",
                    "create",
                    "--board-id",
                    &board_b_id,
                    "--name",
                    "TODO",
                ])
                .assert()
                .success()
                .get_output()
                .stdout
                .clone();
            let col_json = parse_json_output(&String::from_utf8_lossy(&col_output));
            (board_b_id, extract_id(&col_json))
        };

        let card_a_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_a_id,
                "--column-id",
                &col_a_id,
                "--title",
                "Card on A",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let card_a_id = extract_id(&parse_json_output(&String::from_utf8_lossy(&card_a_output)));

        let card_b_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_b_id,
                "--column-id",
                &col_b_id,
                "--title",
                "Card on B",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let card_b_id = extract_id(&parse_json_output(&String::from_utf8_lossy(&card_b_output)));

        (
            board_a_id, col_a_id, board_b_id, col_b_id, card_a_id, card_b_id,
        )
    }

    #[test]
    fn test_card_get_ambiguous_identifier_returns_all_matches() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (_, _, _, _, _, _) = setup_two_boards_same_prefix(&file);

        let output = kanban()
            .args([file.to_str().unwrap(), "card", "get", "KAN-1"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        let items = json["data"]
            .as_array()
            .expect("data should be an array when multiple cards match");
        assert_eq!(items.len(), 2);
        let titles: Vec<&str> = items.iter().map(|c| c["title"].as_str().unwrap()).collect();
        assert!(titles.contains(&"Card on A"));
        assert!(titles.contains(&"Card on B"));
    }

    #[test]
    fn test_card_mutate_ambiguous_identifier_error_lists_candidates() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (_, _, _, _, card_a_id, card_b_id) = setup_two_boards_same_prefix(&file);

        kanban()
            .args([file.to_str().unwrap(), "card", "archive", "KAN-1"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("Ambiguous"))
            .stderr(predicate::str::contains(&card_a_id))
            .stderr(predicate::str::contains(&card_b_id));
    }
}

mod sprint_tests {
    use super::*;

    fn setup_board(file: &std::path::Path) -> String {
        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "Test Board",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        extract_id(&json)
    }

    #[test]
    fn test_sprint_create() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let board_id = setup_board(&file);

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "sprint",
                "create",
                "--board-id",
                &board_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["board_id"], board_id);
        assert_eq!(json["data"]["status"], "Planning");
    }

    #[test]
    fn test_sprint_create_with_options() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let board_id = setup_board(&file);

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "sprint",
                "create",
                "--board-id",
                &board_id,
                "--prefix",
                "SPRINT",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["prefix"], "SPRINT");
    }

    #[test]
    fn test_sprint_list() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let board_id = setup_board(&file);

        kanban()
            .args([
                file.to_str().unwrap(),
                "sprint",
                "create",
                "--board-id",
                &board_id,
            ])
            .assert()
            .success();

        kanban()
            .args([
                file.to_str().unwrap(),
                "sprint",
                "create",
                "--board-id",
                &board_id,
            ])
            .assert()
            .success();

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "sprint",
                "list",
                "--board-id",
                &board_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["total"], 2);
    }

    #[test]
    fn test_sprint_activate() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let board_id = setup_board(&file);

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "sprint",
                "create",
                "--board-id",
                &board_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let sprint_id = extract_id(&create_json);

        let output = kanban()
            .args([file.to_str().unwrap(), "sprint", "activate", &sprint_id])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["status"], "Active");
        assert!(json["data"]["start_date"].as_str().is_some());
        assert!(json["data"]["end_date"].as_str().is_some());
    }

    #[test]
    fn test_sprint_complete() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let board_id = setup_board(&file);

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "sprint",
                "create",
                "--board-id",
                &board_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let sprint_id = extract_id(&create_json);

        kanban()
            .args([file.to_str().unwrap(), "sprint", "activate", &sprint_id])
            .assert()
            .success();

        let output = kanban()
            .args([file.to_str().unwrap(), "sprint", "complete", &sprint_id])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["status"], "Completed");
    }

    #[test]
    fn test_sprint_cancel() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let board_id = setup_board(&file);

        let create_output = kanban()
            .args([
                file.to_str().unwrap(),
                "sprint",
                "create",
                "--board-id",
                &board_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let create_json = parse_json_output(&String::from_utf8_lossy(&create_output));
        let sprint_id = extract_id(&create_json);

        kanban()
            .args([file.to_str().unwrap(), "sprint", "activate", &sprint_id])
            .assert()
            .success();

        let output = kanban()
            .args([file.to_str().unwrap(), "sprint", "cancel", &sprint_id])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["status"], "Cancelled");
    }

    #[test]
    fn test_card_assign_sprint() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let board_id = setup_board(&file);

        let column_output = kanban()
            .args([
                file.to_str().unwrap(),
                "column",
                "create",
                "--board-id",
                &board_id,
                "--name",
                "TODO",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let column_json = parse_json_output(&String::from_utf8_lossy(&column_output));
        let column_id = extract_id(&column_json);

        let card_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Task",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let card_json = parse_json_output(&String::from_utf8_lossy(&card_output));
        let card_id = extract_id(&card_json);

        let sprint_output = kanban()
            .args([
                file.to_str().unwrap(),
                "sprint",
                "create",
                "--board-id",
                &board_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let sprint_json = parse_json_output(&String::from_utf8_lossy(&sprint_output));
        let sprint_id = extract_id(&sprint_json);

        kanban()
            .args([file.to_str().unwrap(), "sprint", "activate", &sprint_id])
            .assert()
            .success();

        let output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "assign-sprint",
                &card_id,
                "--sprint-id",
                &sprint_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert_eq!(json["data"]["sprint_id"], sprint_id);
    }

    #[test]
    fn test_card_unassign_sprint() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let board_id = setup_board(&file);

        let column_output = kanban()
            .args([
                file.to_str().unwrap(),
                "column",
                "create",
                "--board-id",
                &board_id,
                "--name",
                "TODO",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let column_json = parse_json_output(&String::from_utf8_lossy(&column_output));
        let column_id = extract_id(&column_json);

        let card_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Task",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let card_json = parse_json_output(&String::from_utf8_lossy(&card_output));
        let card_id = extract_id(&card_json);

        let sprint_output = kanban()
            .args([
                file.to_str().unwrap(),
                "sprint",
                "create",
                "--board-id",
                &board_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let sprint_json = parse_json_output(&String::from_utf8_lossy(&sprint_output));
        let sprint_id = extract_id(&sprint_json);

        kanban()
            .args([file.to_str().unwrap(), "sprint", "activate", &sprint_id])
            .assert()
            .success();

        kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "assign-sprint",
                &card_id,
                "--sprint-id",
                &sprint_id,
            ])
            .assert()
            .success();

        let output = kanban()
            .args([file.to_str().unwrap(), "card", "unassign-sprint", &card_id])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
        assert!(json["data"]["sprint_id"].is_null());
    }
}

mod export_import_tests {
    use super::*;

    #[test]
    fn test_export_full() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "Test Board",
            ])
            .assert()
            .success();

        kanban()
            .args([file.to_str().unwrap(), "export"])
            .assert()
            .success()
            .stdout(predicate::str::contains("\"boards\""))
            .stdout(predicate::str::contains("\"columns\""))
            .stdout(predicate::str::contains("Test Board"));
    }

    #[test]
    fn test_export_single_board() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        let board_output = kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "Test Board",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let board_json = parse_json_output(&String::from_utf8_lossy(&board_output));
        let board_id = extract_id(&board_json);

        kanban()
            .args([file.to_str().unwrap(), "export", "--board-id", &board_id])
            .assert()
            .success()
            .stdout(predicate::str::contains("\"boards\""))
            .stdout(predicate::str::contains("Test Board"));
    }

    #[test]
    fn test_import() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let import_file = dir.path().join("import.json");

        kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "Original Board",
            ])
            .assert()
            .success();

        let export_output = kanban()
            .args([file.to_str().unwrap(), "export"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        fs::write(
            &import_file,
            String::from_utf8_lossy(&export_output).as_ref(),
        )
        .unwrap();

        let new_file = dir.path().join("new.json");
        let output = kanban()
            .args([
                new_file.to_str().unwrap(),
                "import",
                "--file",
                import_file.to_str().unwrap(),
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json = parse_json_output(&String::from_utf8_lossy(&output));
        assert!(json["success"].as_bool().unwrap());
    }
}

mod error_tests {
    use super::*;

    #[test]
    fn test_missing_required_args() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        kanban()
            .args([file.to_str().unwrap(), "board", "create"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("--name"));
    }

    #[test]
    fn test_invalid_uuid() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        kanban()
            .args([file.to_str().unwrap(), "board", "get", "not-a-uuid"])
            .assert()
            .failure();
    }

    #[test]
    fn test_nonexistent_board() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "get",
                "00000000-0000-0000-0000-000000000000",
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("\"success\":false"))
            .stderr(predicate::str::contains("not found"));
    }

    #[test]
    fn test_column_list_requires_board_id() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        kanban()
            .args([file.to_str().unwrap(), "column", "list"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("--board-id"));
    }

    #[test]
    fn test_card_get_nonexistent_numeric_identifier() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        kanban()
            .args([file.to_str().unwrap(), "card", "get", "555"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("\"success\":false"))
            .stderr(predicate::str::contains("Card not found"))
            .stderr(predicate::str::contains("555"));
    }

    #[test]
    fn test_card_get_nonexistent_prefix_identifier() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        kanban()
            .args([file.to_str().unwrap(), "card", "get", "KAN-5"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("\"success\":false"))
            .stderr(predicate::str::contains("Card not found"))
            .stderr(predicate::str::contains("KAN-5"));
    }

    #[test]
    fn test_card_get_no_backtrace() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        kanban()
            .env("RUST_BACKTRACE", "1")
            .args([file.to_str().unwrap(), "card", "get", "555"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("stack backtrace").not());
    }

    fn setup_board_and_column(file: &std::path::Path) -> (String, String) {
        let board_output = kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "Test Board",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let board_json = parse_json_output(&String::from_utf8_lossy(&board_output));
        let board_id = extract_id(&board_json);

        let column_output = kanban()
            .args([
                file.to_str().unwrap(),
                "column",
                "create",
                "--board-id",
                &board_id,
                "--name",
                "TODO",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let column_json = parse_json_output(&String::from_utf8_lossy(&column_output));
        let column_id = extract_id(&column_json);

        (board_id, column_id)
    }

    #[test]
    fn test_card_create_invalid_priority() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column(&file);

        kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Test",
                "--priority",
                "badpriority",
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("\"success\":false"))
            .stderr(predicate::str::contains("Invalid priority"))
            .stderr(predicate::str::contains("badpriority"));
    }

    #[test]
    fn test_card_list_invalid_status() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");

        kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "list",
                "--status",
                "notastatus",
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("\"success\":false"))
            .stderr(predicate::str::contains("Invalid status"))
            .stderr(predicate::str::contains("notastatus"));
    }

    #[test]
    fn test_card_update_invalid_priority() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let (board_id, column_id) = setup_board_and_column(&file);

        let card_output = kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "create",
                "--board-id",
                &board_id,
                "--column-id",
                &column_id,
                "--title",
                "Test",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let card_json = parse_json_output(&String::from_utf8_lossy(&card_output));
        let card_id = extract_id(&card_json);

        kanban()
            .args([
                file.to_str().unwrap(),
                "card",
                "update",
                &card_id,
                "--priority",
                "badpriority",
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("\"success\":false"))
            .stderr(predicate::str::contains("Invalid priority"))
            .stderr(predicate::str::contains("badpriority"));
    }

    fn setup_sprint(file: &std::path::Path) -> String {
        let board_output = kanban()
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "Test Board",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let board_json = parse_json_output(&String::from_utf8_lossy(&board_output));
        let board_id = extract_id(&board_json);

        let sprint_output = kanban()
            .args([
                file.to_str().unwrap(),
                "sprint",
                "create",
                "--board-id",
                &board_id,
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let sprint_json = parse_json_output(&String::from_utf8_lossy(&sprint_output));
        extract_id(&sprint_json)
    }

    #[test]
    fn test_sprint_update_invalid_start_date() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let sprint_id = setup_sprint(&file);

        kanban()
            .args([
                file.to_str().unwrap(),
                "sprint",
                "update",
                &sprint_id,
                "--start-date",
                "notadate",
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("\"success\":false"))
            .stderr(predicate::str::contains("Invalid date"))
            .stderr(predicate::str::contains("notadate"));
    }

    #[test]
    fn test_sprint_update_invalid_end_date() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.json");
        let sprint_id = setup_sprint(&file);

        kanban()
            .args([
                file.to_str().unwrap(),
                "sprint",
                "update",
                &sprint_id,
                "--end-date",
                "notadate",
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("\"success\":false"))
            .stderr(predicate::str::contains("Invalid date"))
            .stderr(predicate::str::contains("notadate"));
    }
}

mod no_file_tests {
    use super::*;

    #[test]
    fn test_subcommand_with_no_file_and_no_config_fails_with_actionable_message() {
        let dir = tempdir().unwrap();
        kanban_no_config(dir.path())
            .args(["board", "list"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("No data file specified"))
            .stderr(predicate::str::contains("KANBAN_FILE"));
        assert!(
            !dir.path().join("kanban.json").exists(),
            "must not silently create kanban.json"
        );
    }

    #[test]
    fn test_kanban_file_env_var_accepted_without_positional_arg() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("via-env.json");
        // Seed the file via the positional path so this test verifies env-var
        // resolution rather than implicit JSON auto-create-on-open.
        kanban_no_config(dir.path())
            .args([
                file.to_str().unwrap(),
                "board",
                "create",
                "--name",
                "SeededViaPositional",
            ])
            .assert()
            .success();
        kanban_no_config(dir.path())
            .env("KANBAN_FILE", file.to_str().unwrap())
            .args(["board", "list"])
            .assert()
            .success()
            .stdout(predicate::str::contains("SeededViaPositional"));
    }

    #[test]
    fn test_completions_subcommand_requires_no_file() {
        let dir = tempdir().unwrap();
        kanban_no_config(dir.path())
            .args(["completions", "bash"])
            .assert()
            .success();
    }
}

mod version_and_help_tests {
    use super::*;

    // The version output must go to stdout with a clean exit, no
    // "Error:" prefix, and a single trailing newline. Both -V and
    // --version must behave identically.
    #[test]
    fn test_short_version_flag_writes_clean_to_stdout() {
        let dir = tempdir().unwrap();
        let assert = kanban_no_config(dir.path()).args(["-V"]).assert().success();
        let output = assert.get_output();
        assert!(
            output.stderr.is_empty(),
            "stderr must be empty for -V, got: {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.starts_with("kanban "),
            "stdout must start with \"kanban \", got: {:?}",
            stdout
        );
        assert!(
            !stdout.starts_with("Error:"),
            "stdout must not start with \"Error:\", got: {:?}",
            stdout
        );
        assert!(
            stdout.ends_with('\n') && !stdout.ends_with("\n\n"),
            "stdout must end with exactly one newline, got: {:?}",
            stdout
        );
    }

    #[test]
    fn test_long_version_flag_writes_clean_to_stdout() {
        let dir = tempdir().unwrap();
        let assert = kanban_no_config(dir.path())
            .args(["--version"])
            .assert()
            .success();
        let output = assert.get_output();
        assert!(
            output.stderr.is_empty(),
            "stderr must be empty for --version, got: {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.starts_with("kanban "),
            "stdout must start with \"kanban \", got: {:?}",
            stdout
        );
        assert!(
            !stdout.starts_with("Error:"),
            "stdout must not start with \"Error:\", got: {:?}",
            stdout
        );
        assert!(
            stdout.ends_with('\n') && !stdout.ends_with("\n\n"),
            "stdout must end with exactly one newline, got: {:?}",
            stdout
        );
    }

    // --help must also reach stdout with exit 0 — same clap pitfall.
    #[test]
    fn test_help_flag_writes_clean_to_stdout() {
        let dir = tempdir().unwrap();
        let assert = kanban_no_config(dir.path())
            .args(["--help"])
            .assert()
            .success();
        let output = assert.get_output();
        assert!(
            output.stderr.is_empty(),
            "stderr must be empty for --help, got: {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.starts_with("Error:"),
            "--help stdout must not start with \"Error:\", got: {:?}",
            stdout
        );
        assert!(
            stdout.contains("Usage:"),
            "--help stdout must include a Usage: section, got: {:?}",
            stdout
        );
    }

    // Real argument errors continue to be treated as errors.
    #[test]
    fn test_unknown_flag_still_errors_to_stderr() {
        let dir = tempdir().unwrap();
        let assert = kanban_no_config(dir.path())
            .args(["--no-such-flag"])
            .assert()
            .failure();
        let output = assert.get_output();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.is_empty(),
            "an unknown flag must surface a stderr error message"
        );
    }
}
