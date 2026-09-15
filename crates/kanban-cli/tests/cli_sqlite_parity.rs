use assert_cmd::{cargo_bin_cmd, Command};
use serde_json::Value;
use tempfile::tempdir;

fn kanban() -> Command {
    cargo_bin_cmd!("kanban")
}

fn kanban_no_config(dir: &std::path::Path) -> Command {
    let mut cmd = kanban();
    cmd.current_dir(dir)
        .env_remove("KANBAN_FILE")
        .env_remove("XDG_CONFIG_HOME")
        .env("HOME", dir)
        .env("KANBAN_CONFIG", dir.join("config.toml"));
    cmd
}

fn parse_json(output: &str) -> Value {
    serde_json::from_str(output).expect("Failed to parse JSON output")
}

fn seed_graph_and_assert(file: &str, dir: &std::path::Path) -> (Value, Value) {
    kanban_no_config(dir)
        .args([file, "init"])
        .assert()
        .success();
    kanban_no_config(dir)
        .args([file, "board", "create", "--name", "Parity"])
        .assert()
        .success();
    kanban_no_config(dir)
        .args([
            file, "column", "create", "--board", "Parity", "--name", "Todo",
        ])
        .assert()
        .success();
    kanban_no_config(dir)
        .args([
            file, "card", "create", "--board", "Parity", "--column", "Todo", "--title", "Parent",
        ])
        .assert()
        .success();
    kanban_no_config(dir)
        .args([
            file, "card", "create", "--board", "Parity", "--column", "Todo", "--title", "Child",
        ])
        .assert()
        .success();

    let cards_output = kanban_no_config(dir)
        .args([file, "card", "list", "--board", "Parity"])
        .output()
        .unwrap();
    let cards = parse_json(std::str::from_utf8(&cards_output.stdout).unwrap());
    let cards_list = cards["data"]["items"].as_array().unwrap();
    let parent_id = cards_list.iter().find(|c| c["title"] == "Parent").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let child_id = cards_list.iter().find(|c| c["title"] == "Child").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    kanban_no_config(dir)
        .args([file, "relation", "add", &parent_id, &child_id])
        .assert()
        .success();

    let board_get = kanban_no_config(dir)
        .args([file, "board", "get", "Parity"])
        .output()
        .unwrap();
    let board_json = parse_json(std::str::from_utf8(&board_get.stdout).unwrap());

    let children = kanban_no_config(dir)
        .args([file, "relation", "children", &parent_id])
        .output()
        .unwrap();
    let children_json = parse_json(std::str::from_utf8(&children.stdout).unwrap());

    (board_json, children_json)
}

#[test]
fn test_board_get_and_relation_children_agree_across_json_and_sqlite_locators() {
    let json_dir = tempdir().unwrap();
    let json_file = json_dir.path().join("parity.json");
    let (json_board, json_children) =
        seed_graph_and_assert(json_file.to_str().unwrap(), json_dir.path());

    let sqlite_dir = tempdir().unwrap();
    let sqlite_file = sqlite_dir.path().join("parity.sqlite");
    let (sqlite_board, sqlite_children) =
        seed_graph_and_assert(sqlite_file.to_str().unwrap(), sqlite_dir.path());

    assert_eq!(json_board["data"]["name"], sqlite_board["data"]["name"]);
    assert_eq!(
        json_board["data"]["card_prefix"],
        sqlite_board["data"]["card_prefix"]
    );
    assert_eq!(
        json_children["data"].as_array().unwrap().len(),
        sqlite_children["data"].as_array().unwrap().len()
    );
}
