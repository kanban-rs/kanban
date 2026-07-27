//! KAN-716: board read routes (GET /v1/boards, GET /v1/boards/{id}).
//! Read-only, no mutation, no event broadcast. Established via `tower::ServiceExt::oneshot`
//! against the router directly, with no real TCP socket.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use kanban_persistence_json::JsonFileStore;
use kanban_server::app;
use kanban_server::state::AppState;
use kanban_service::json_backend::JsonDataStore;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;
use uuid::Uuid;

fn make_state(path: &std::path::Path) -> AppState {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    let ctx = KanbanContext::open_deferred(backend, AppConfig::default());
    AppState::new(ctx)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_boards_empty_returns_200_empty_array() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let response = app::router(state)
        .oneshot(
            Request::builder()
                .uri("/v1/boards")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_boards_returns_all_seeded_boards() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    // Seed two boards
    let board1_id: Uuid;
    let board2_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board1_id = ctx
            .create_board("Board 1".to_string(), Some("B1".to_string()))
            .unwrap()
            .id;
        board2_id = ctx
            .create_board("Board 2".to_string(), Some("B2".to_string()))
            .unwrap()
            .id;
    }

    let response = app::router(state)
        .oneshot(
            Request::builder()
                .uri("/v1/boards")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json.is_array(), "response should be an array");
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2, "should have 2 boards");

    let returned_ids: std::collections::HashSet<_> = arr
        .iter()
        .map(|b| Uuid::parse_str(b["id"].as_str().unwrap()).expect("id should be a valid UUID"))
        .collect();

    let expected_ids: std::collections::HashSet<_> =
        vec![board1_id, board2_id].into_iter().collect();
    assert_eq!(
        returned_ids, expected_ids,
        "returned ids should match seeded ids"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_returns_board_response_for_existing_id() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    // Seed one board
    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("My Board".to_string(), Some("MB".to_string()))
            .unwrap()
            .id;
    }

    let response = app::router(state)
        .oneshot(
            Request::builder()
                .uri(format!("/v1/boards/{}", board_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["id"], board_id.to_string());
    assert_eq!(json["name"], "My Board");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_response_omits_internal_allocation_state() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    // Seed one board
    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Test Board".to_string(), Some("TB".to_string()))
            .unwrap()
            .id;
    }

    let response = app::router(state)
        .oneshot(
            Request::builder()
                .uri(format!("/v1/boards/{}", board_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Assert all internal allocation state fields are absent
    for hidden in [
        "card_counter",
        "next_sprint_number",
        "sprint_counters",
        "sprint_names",
        "sprint_name_used_count",
    ] {
        assert!(
            json.get(hidden).is_none(),
            "field {} should be absent from response",
            hidden
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_unknown_id_returns_404_not_found() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let random_id = Uuid::new_v4();

    let response = app::router(state)
        .oneshot(
            Request::builder()
                .uri(format!("/v1/boards/{}", random_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["code"], "NOT_FOUND");
}
