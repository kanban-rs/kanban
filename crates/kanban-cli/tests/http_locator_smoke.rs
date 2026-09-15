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
