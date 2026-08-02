#![cfg(feature = "test-helpers")]

use kanban_server::test_helpers::TestServer;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread")]
async fn test_health_endpoint_returns_ok_over_real_socket() {
    let server = TestServer::start().await;

    let response = server
        .client()
        .get(format!("{}/health", server.base_url()))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let json: serde_json::Value = response.json().await.unwrap();
    let instance_id = json["instance_id"].as_str().expect("instance_id present");
    Uuid::parse_str(instance_id).expect("instance_id is a valid uuid");

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_two_test_servers_bind_distinct_nonzero_ports() {
    let server_a = TestServer::start().await;
    let server_b = TestServer::start().await;

    assert!(server_a.addr().port() > 0);
    assert!(server_b.addr().port() > 0);
    assert_ne!(
        server_a.addr().port(),
        server_b.addr().port(),
        "each TestServer must be bound to its own OS-assigned port"
    );

    server_a.shutdown().await;
    server_b.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_and_fetch_board_over_real_socket() {
    let server = TestServer::start().await;

    let create_response = server
        .client()
        .post(format!("{}/v1/boards", server.base_url()))
        .json(&serde_json::json!({"name": "Real Socket Board", "card_prefix": "RS"}))
        .send()
        .await
        .unwrap();
    assert_eq!(create_response.status(), reqwest::StatusCode::CREATED);
    let created: serde_json::Value = create_response.json().await.unwrap();
    let board_id = created["id"].as_str().unwrap();

    let get_response = server
        .client()
        .get(format!("{}/v1/boards/{board_id}", server.base_url()))
        .send()
        .await
        .unwrap();
    assert_eq!(get_response.status(), reqwest::StatusCode::OK);
    let fetched: serde_json::Value = get_response.json().await.unwrap();
    assert_eq!(fetched["name"], "Real Socket Board");

    server.shutdown().await;
}
