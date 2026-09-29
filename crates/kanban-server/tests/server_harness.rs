#![cfg(feature = "test-helpers")]

use kanban_domain::NewBoard;
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_server::test_helpers::TestServer;
use kanban_service::{AppConfig, KanbanContext};
use std::sync::Arc;
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

#[tokio::test(flavor = "multi_thread")]
async fn test_start_on_json_serves_and_persists_to_the_given_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("remote.json");
    let server = TestServer::start_on_json(&path).await;

    let create_response = server
        .client()
        .post(format!("{}/v1/boards", server.base_url()))
        .json(&serde_json::json!({"name": "Persisted", "card_prefix": "PJ"}))
        .send()
        .await
        .unwrap();
    assert_eq!(create_response.status(), reqwest::StatusCode::CREATED);

    server.shutdown().await;

    let backend: Arc<dyn kanban_service::KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(&path))));
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let boards = ctx.data_store().list_boards().unwrap();
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "Persisted");
    assert_eq!(boards[0].card_prefix, Some("PJ".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_start_on_sqlite_serves_and_persists_to_the_given_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("remote.sqlite");
    let server = TestServer::start_on_sqlite(&path).await;

    let create_response = server
        .client()
        .post(format!("{}/v1/boards", server.base_url()))
        .json(&serde_json::json!({"name": "Persisted", "card_prefix": "PJ"}))
        .send()
        .await
        .unwrap();
    assert_eq!(create_response.status(), reqwest::StatusCode::CREATED);

    server.shutdown().await;

    assert!(path.exists(), "sqlite file should exist on disk");

    let backend: Arc<dyn kanban_service::KanbanBackend> =
        Arc::new(SqliteBackend::open(path.to_str().unwrap()).await.unwrap());
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let boards = ctx.data_store().list_boards().unwrap();
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "Persisted");
    assert_eq!(boards[0].card_prefix, Some("PJ".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_start_recording_logs_a_request_whose_handler_returned_404() {
    let (server, log) = TestServer::start_recording(|_| {}).await;
    let missing_path = format!("/v1/no-such-route/{}", Uuid::new_v4());

    let response = server
        .client()
        .get(format!("{}{}", server.base_url(), missing_path))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);

    let entries = log.lock().unwrap().clone();
    assert!(
        entries
            .iter()
            .any(|(method, path)| method == reqwest::Method::GET && path == &missing_path),
        "expected the 404'd request to be logged, got {entries:?}"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_start_recording_logs_each_request_in_arrival_order() {
    let (server, log) = TestServer::start_recording(|_| {}).await;

    server
        .client()
        .get(format!("{}/health", server.base_url()))
        .send()
        .await
        .unwrap();
    server
        .client()
        .get(format!("{}/v1/boards", server.base_url()))
        .send()
        .await
        .unwrap();

    let entries = log.lock().unwrap().clone();
    assert_eq!(
        entries,
        vec![
            (reqwest::Method::GET, "/health".to_string()),
            (reqwest::Method::GET, "/v1/boards".to_string()),
        ]
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_start_recording_records_the_path_without_the_query_string() {
    let (server, log) = TestServer::start_recording(|ctx| {
        let _ = ctx
            .create_board_from_spec(
                None,
                NewBoard {
                    name: "Recorded".to_string(),
                    description: None,
                    sprint_prefix: None,
                    card_prefix: Some("RC".to_string()),
                    task_sort_field: None,
                    task_sort_order: None,
                    sprint_duration_days: None,
                    task_list_view: None,
                },
            )
            .unwrap();
    })
    .await;

    let boards: serde_json::Value = server
        .client()
        .get(format!("{}/v1/boards", server.base_url()))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let board_id = boards["items"][0]["id"].as_str().unwrap();
    let path_with_query = format!("/v1/boards/{board_id}/cards?column_id={}", Uuid::new_v4());

    server
        .client()
        .get(format!("{}{}", server.base_url(), path_with_query))
        .send()
        .await
        .unwrap();

    let entries = log.lock().unwrap().clone();
    let expected_path = format!("/v1/boards/{board_id}/cards");
    assert!(
        entries
            .iter()
            .any(|(_, path)| path == &expected_path && !path.contains('?')),
        "expected a logged entry for {expected_path} without a query string, got {entries:?}"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_start_recording_seeds_the_context_like_start_with() {
    let (server, _log) = TestServer::start_recording(|ctx| {
        let _ = ctx
            .create_board_from_spec(
                None,
                NewBoard {
                    name: "Seeded".to_string(),
                    description: None,
                    sprint_prefix: None,
                    card_prefix: Some("SD".to_string()),
                    task_sort_field: None,
                    task_sort_order: None,
                    sprint_duration_days: None,
                    task_list_view: None,
                },
            )
            .unwrap();
    })
    .await;

    let boards: serde_json::Value = server
        .client()
        .get(format!("{}/v1/boards", server.base_url()))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(boards["items"][0]["name"], "Seeded");

    server.shutdown().await;
}
