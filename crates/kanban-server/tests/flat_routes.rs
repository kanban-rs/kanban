//! Flat entity routes: GET/PATCH/DELETE /v1/columns/{id} and /v1/cards/{id}
//! These are aliases to the board-scoped routes, without requiring the caller
//! to know the owning board id. Response shape is identical to board-scoped routes.

use axum::http::StatusCode;
use kanban_domain::KanbanOperations;
use serde_json::json;
use tempfile::tempdir;
use uuid::Uuid;

mod common;
use common::{json_of, make_state, send};
use kanban_server::state::AppState;

async fn seed_board_column_and_card(state: &AppState) -> (Uuid, Uuid, Uuid) {
    let mut ctx = state.ctx.lock().await;
    let board_id = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id;
    let col = ctx
        .create_column(board_id, "To Do".to_string(), None)
        .unwrap();
    let card = ctx
        .create_card(board_id, col.id, "Task".to_string(), Default::default())
        .unwrap();
    (board_id, col.id, card.id)
}

async fn seed_board_and_column(state: &AppState) -> (Uuid, Uuid) {
    let mut ctx = state.ctx.lock().await;
    let board_id = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id;
    let col = ctx
        .create_column(board_id, "To Do".to_string(), None)
        .unwrap();
    (board_id, col.id)
}

async fn seed_board_and_card(state: &AppState) -> (Uuid, Uuid) {
    let mut ctx = state.ctx.lock().await;
    let board_id = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id;
    let col = ctx
        .create_column(board_id, "To Do".to_string(), None)
        .unwrap();
    let card = ctx
        .create_card(board_id, col.id, "Task".to_string(), Default::default())
        .unwrap();
    (board_id, card.id)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_column_flat_returns_same_shape_as_board_scoped() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, col_id, _card_id) = seed_board_column_and_card(&state).await;

    let board_scoped_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/columns/{col_id}"),
        None,
    )
    .await;
    let flat_response = send(&state, "GET", &format!("/v1/columns/{col_id}"), None).await;

    assert_eq!(board_scoped_response.status(), StatusCode::OK);
    assert_eq!(flat_response.status(), StatusCode::OK);

    let board_scoped_json = json_of(board_scoped_response).await;
    let flat_json = json_of(flat_response).await;

    assert_eq!(board_scoped_json["id"], flat_json["id"]);
    assert_eq!(board_scoped_json["name"], flat_json["name"]);
    assert_eq!(board_scoped_json["position"], flat_json["position"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_column_flat_updates_and_matches_board_scoped_route() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, col_id, _card_id) = seed_board_column_and_card(&state).await;

    let flat_response = send(
        &state,
        "PATCH",
        &format!("/v1/columns/{col_id}"),
        Some(&json!({"name": "Updated via flat"})),
    )
    .await;

    assert_eq!(flat_response.status(), StatusCode::OK);
    let flat_json = json_of(flat_response).await;
    assert_eq!(flat_json["name"], "Updated via flat");

    let verify_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/columns/{col_id}"),
        None,
    )
    .await;
    let verify_json = json_of(verify_response).await;
    assert_eq!(verify_json["name"], "Updated via flat");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_column_flat_deletes() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, col_id) = seed_board_and_column(&state).await;

    let delete_response = send(&state, "DELETE", &format!("/v1/columns/{col_id}"), None).await;
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let verify_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/columns/{col_id}"),
        None,
    )
    .await;
    assert_eq!(verify_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_column_flat_missing_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let unknown_col = Uuid::new_v4();

    let response = send(&state, "GET", &format!("/v1/columns/{unknown_col}"), None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_flat_returns_same_shape_as_board_scoped() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, _col_id, card_id) = seed_board_column_and_card(&state).await;

    let board_scoped_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/cards/{card_id}"),
        None,
    )
    .await;
    let flat_response = send(&state, "GET", &format!("/v1/cards/{card_id}"), None).await;

    assert_eq!(board_scoped_response.status(), StatusCode::OK);
    assert_eq!(flat_response.status(), StatusCode::OK);

    let board_scoped_json = json_of(board_scoped_response).await;
    let flat_json = json_of(flat_response).await;

    assert_eq!(board_scoped_json["id"], flat_json["id"]);
    assert_eq!(board_scoped_json["title"], flat_json["title"]);
    assert_eq!(board_scoped_json["board_id"], flat_json["board_id"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_card_flat_updates_and_matches_board_scoped_route() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, _col_id, card_id) = seed_board_column_and_card(&state).await;

    let flat_response = send(
        &state,
        "PATCH",
        &format!("/v1/cards/{card_id}"),
        Some(&json!({"title": "Updated via flat"})),
    )
    .await;

    assert_eq!(flat_response.status(), StatusCode::OK);
    let flat_json = json_of(flat_response).await;
    assert_eq!(flat_json["title"], "Updated via flat");

    let verify_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/cards/{card_id}"),
        None,
    )
    .await;
    let verify_json = json_of(verify_response).await;
    assert_eq!(verify_json["title"], "Updated via flat");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_card_flat_deletes() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, card_id) = seed_board_and_card(&state).await;

    let delete_response = send(&state, "DELETE", &format!("/v1/cards/{card_id}"), None).await;
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let verify_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/cards/{card_id}"),
        None,
    )
    .await;
    assert_eq!(verify_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_flat_missing_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let unknown_card = Uuid::new_v4();

    let response = send(&state, "GET", &format!("/v1/cards/{unknown_card}"), None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
