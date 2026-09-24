#![cfg(feature = "test-helpers")]

//! GET /v1/cards/lookup?identifier=<string> resolves a card identifier
//! (`KAN-7` or a bare `7`) to zero or more cards, without touching a board.

use axum::http::StatusCode;
use kanban_domain::KanbanOperations;
use kanban_server::state::AppState;
use kanban_server::test_helpers::{json_of, make_sqlite_state, make_state, send};
use tempfile::tempdir;
use uuid::Uuid;

async fn seed_card_with_prefix(state: &AppState, card_prefix: &str) -> Uuid {
    let mut ctx = state.ctx.lock().await;
    let board_id = ctx
        .create_board("Board".to_string(), Some(card_prefix.to_string()))
        .unwrap()
        .id;
    let col = ctx
        .create_column(board_id, "To Do".to_string(), None)
        .unwrap();
    ctx.create_card(board_id, col.id, "Task".to_string(), Default::default())
        .unwrap()
        .id
}

#[tokio::test(flavor = "multi_thread")]
async fn test_lookup_by_prefix_and_number_returns_single_match() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let card_id = seed_card_with_prefix(&state, "KAN").await;

    let response = send(&state, "GET", "/v1/cards/lookup?identifier=KAN-1", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    let arr = body.as_array().expect("array body");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], card_id.to_string());
    assert_eq!(arr[0]["card_number"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_lookup_normalizes_prefix_case_server_side() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let card_id = seed_card_with_prefix(&state, "KAN").await;

    let response = send(&state, "GET", "/v1/cards/lookup?identifier=kan-1", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    let arr = body.as_array().expect("array body");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], card_id.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_lookup_bare_number_matches_across_boards() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let a = seed_card_with_prefix(&state, "AAA").await;
    let b = seed_card_with_prefix(&state, "BBB").await;

    let response = send(&state, "GET", "/v1/cards/lookup?identifier=1", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    let arr = body.as_array().expect("array body");
    assert_eq!(arr.len(), 2);
    let ids: std::collections::HashSet<String> = arr
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    let expected: std::collections::HashSet<String> =
        [a.to_string(), b.to_string()].into_iter().collect();
    assert_eq!(ids, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_lookup_no_match_returns_empty_array_not_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    seed_card_with_prefix(&state, "KAN").await;

    let response = send(&state, "GET", "/v1/cards/lookup?identifier=KAN-999", None).await;
    assert_ne!(
        response.status(),
        StatusCode::NOT_FOUND,
        "no match must not be a 404"
    );
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert_eq!(body, serde_json::json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_lookup_unparseable_identifier_returns_empty_array() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    seed_card_with_prefix(&state, "KAN").await;

    let response = send(&state, "GET", "/v1/cards/lookup?identifier=---", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert_eq!(body, serde_json::json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_lookup_missing_identifier_is_a_400_naming_the_identifier_param() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let response = send(&state, "GET", "/v1/cards/lookup", None).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        text.contains("identifier"),
        "expected body to name the missing `identifier` param, got: {text}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_lookup_on_sqlite_backend_returns_match() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("s.db")).await;
    let card_id = seed_card_with_prefix(&state, "KAN").await;

    let response = send(&state, "GET", "/v1/cards/lookup?identifier=KAN-1", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    let arr = body.as_array().expect("array body");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], card_id.to_string());
}
