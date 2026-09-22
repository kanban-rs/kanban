#![cfg(feature = "http")]

use kanban_domain::KanbanOperations;
use kanban_server::test_helpers::TestServer;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread")]
async fn test_cli_card_create_against_http_locator_succeeds() {
    let ids = Arc::new(Mutex::new(None::<(Uuid, Uuid)>));
    let ids_for_seed = Arc::clone(&ids);

    let server = TestServer::start_with(move |ctx| {
        let board_id = ctx
            .create_board("Smoke Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let column_id = ctx
            .create_column(board_id, "To Do".to_string(), None)
            .unwrap()
            .id;
        *ids_for_seed.lock().unwrap() = Some((board_id, column_id));
    })
    .await;
    let (board_id, column_id) = ids.lock().unwrap().take().unwrap();
    let base_url = server.base_url();

    let output = tokio::task::spawn_blocking(move || {
        use assert_cmd::cargo_bin_cmd;
        cargo_bin_cmd!("kanban")
            .args([
                &base_url,
                "card",
                "create",
                "--board",
                &board_id.to_string(),
                "--column",
                &column_id.to_string(),
                "--title",
                "Smoke",
            ])
            .output()
            .unwrap()
    })
    .await
    .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(response["success"], true);
    assert_eq!(response["data"]["title"], "Smoke");
    let card_id = response["data"]["id"].as_str().unwrap();

    let card: Value = server
        .client()
        .get(format!("{}/v1/cards/{card_id}", server.base_url()))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        card["title"], "Smoke",
        "server should hold the created card: {card:?}"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cli_column_get_by_name_with_a_board_resolves_over_an_http_locator() {
    let ids = Arc::new(Mutex::new(None::<Uuid>));
    let ids_for_seed = Arc::clone(&ids);

    let server = TestServer::start_with(move |ctx| {
        let board_a = ctx
            .create_board("Board A".to_string(), Some("A".to_string()))
            .unwrap()
            .id;
        let board_b = ctx
            .create_board("Board B".to_string(), Some("B".to_string()))
            .unwrap()
            .id;
        ctx.create_column(board_a, "Ready".to_string(), None)
            .unwrap();
        let column_b = ctx
            .create_column(board_b, "Ready".to_string(), None)
            .unwrap()
            .id;
        *ids_for_seed.lock().unwrap() = Some(column_b);
    })
    .await;
    let column_b_id = ids.lock().unwrap().take().unwrap();
    let base_url = server.base_url();
    let base_url_for_second = base_url.clone();

    let output = tokio::task::spawn_blocking(move || {
        use assert_cmd::cargo_bin_cmd;
        cargo_bin_cmd!("kanban")
            .args([&base_url, "column", "get", "Ready", "--board", "Board B"])
            .output()
            .unwrap()
    })
    .await
    .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(response["success"], true);
    assert_eq!(response["data"]["id"], column_b_id.to_string());

    let without_board = tokio::task::spawn_blocking(move || {
        use assert_cmd::cargo_bin_cmd;
        cargo_bin_cmd!("kanban")
            .args([&base_url_for_second, "column", "get", "Ready"])
            .output()
            .unwrap()
    })
    .await
    .unwrap();

    assert!(!without_board.status.success());
    let stderr = String::from_utf8_lossy(&without_board.stderr);
    assert!(stderr.contains("--board"), "stderr: {stderr}");
    assert!(!stderr.contains("list_all_columns"), "stderr: {stderr}");

    server.shutdown().await;
}

// Sprint NAME resolution needs the board's name-index pool
// (`Board::sprint_names`), which `kanban-backend-http`'s `board_from_response`
// deliberately drops (see its doc comment) -- that gap predates this card and
// is out of its scope. Sprint NUMBER resolution needs no such pool, so it is
// what proves the board-scoped contract end to end over HTTP here.
#[tokio::test(flavor = "multi_thread")]
async fn test_cli_sprint_get_by_number_with_a_board_resolves_over_an_http_locator() {
    let ids = Arc::new(Mutex::new(None::<Uuid>));
    let ids_for_seed = Arc::clone(&ids);

    let server = TestServer::start_with(move |ctx| {
        let board_a = ctx
            .create_board("Board A".to_string(), Some("A".to_string()))
            .unwrap()
            .id;
        let board_b = ctx
            .create_board("Board B".to_string(), Some("B".to_string()))
            .unwrap()
            .id;
        ctx.create_sprint(board_a, Some("ALPHA".to_string()), None)
            .unwrap();
        let sprint_b = ctx
            .create_sprint(board_b, Some("BETA".to_string()), None)
            .unwrap()
            .id;
        *ids_for_seed.lock().unwrap() = Some(sprint_b);
    })
    .await;
    let sprint_b_id = ids.lock().unwrap().take().unwrap();
    let base_url = server.base_url();
    let base_url_for_second = base_url.clone();

    let output = tokio::task::spawn_blocking(move || {
        use assert_cmd::cargo_bin_cmd;
        cargo_bin_cmd!("kanban")
            .args([&base_url, "sprint", "get", "1", "--board", "Board B"])
            .output()
            .unwrap()
    })
    .await
    .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let response: Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(response["success"], true);
    assert_eq!(response["data"]["id"], sprint_b_id.to_string());

    let without_board = tokio::task::spawn_blocking(move || {
        use assert_cmd::cargo_bin_cmd;
        cargo_bin_cmd!("kanban")
            .args([&base_url_for_second, "sprint", "get", "1"])
            .output()
            .unwrap()
    })
    .await
    .unwrap();

    assert!(!without_board.status.success());
    let stderr = String::from_utf8_lossy(&without_board.stderr);
    assert!(stderr.contains("--board"), "stderr: {stderr}");
    assert!(!stderr.contains("list_all_sprints"), "stderr: {stderr}");

    server.shutdown().await;
}
