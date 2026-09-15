#![cfg(feature = "test-helpers")]

//! Flat entity routes: GET/PATCH/DELETE /v1/columns/{id} and /v1/cards/{id}
//! These are aliases to the board-scoped routes, without requiring the caller
//! to know the owning board id. Response shape is identical to board-scoped routes.

use axum::http::StatusCode;
use kanban_domain::KanbanOperations;
use kanban_server::state::AppState;
use kanban_server::test_helpers::{json_of, make_state, send, send_with_headers};
use serde_json::json;
use tempfile::tempdir;
use uuid::Uuid;

fn etag_of(response: &axum::response::Response) -> String {
    response
        .headers()
        .get("etag")
        .expect("etag header")
        .to_str()
        .unwrap()
        .to_string()
}

fn is_quoted_32_hex(tag: &str) -> bool {
    tag.len() == 34
        && tag.starts_with('"')
        && tag.ends_with('"')
        && tag[1..33]
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

const STALE_IF_MATCH: &str = "\"00000000000000000000000000000000\"";

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

async fn seed_board_and_sprint(state: &AppState, name: &str) -> (Uuid, Uuid) {
    let mut ctx = state.ctx.lock().await;
    let board_id = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id;
    let sprint = ctx
        .create_sprint(board_id, Some("SPR".to_string()), Some(name.to_string()))
        .unwrap();
    (board_id, sprint.id)
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
async fn test_patch_column_flat_response_carries_invalidation_naming_the_column() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, col_id, _card_id) = seed_board_column_and_card(&state).await;

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/columns/{col_id}"),
        Some(&json!({"name": "Renamed"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert_eq!(body["name"], "Renamed");
    let invalidated_columns = body["invalidation"]["entities"]["columns"]
        .as_array()
        .expect("entities invalidation must name columns");
    assert!(invalidated_columns.iter().any(|v| v == &col_id.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_column_flat_deletes() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, col_id) = seed_board_and_column(&state).await;

    let delete_response = send(&state, "DELETE", &format!("/v1/columns/{col_id}"), None).await;
    assert_eq!(delete_response.status(), StatusCode::OK);

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
async fn test_delete_column_flat_returns_200_with_the_invalidation_naming_the_column() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, col_id) = seed_board_and_column(&state).await;

    let response = send(&state, "DELETE", &format!("/v1/columns/{col_id}"), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    let invalidated_columns = body["invalidation"]["entities"]["columns"]
        .as_array()
        .expect("entities invalidation must name columns");
    assert!(invalidated_columns.iter().any(|v| v == &col_id.to_string()));
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
async fn test_patch_card_flat_response_carries_invalidation_naming_the_card() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, _col_id, card_id) = seed_board_column_and_card(&state).await;

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/cards/{card_id}"),
        Some(&json!({"title": "Renamed"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert_eq!(body["title"], "Renamed");
    let invalidated_cards = body["invalidation"]["entities"]["cards"]
        .as_array()
        .expect("entities invalidation must name cards");
    assert!(invalidated_cards.iter().any(|v| v == &card_id.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_card_flat_deletes() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, card_id) = seed_board_and_card(&state).await;

    let delete_response = send(&state, "DELETE", &format!("/v1/cards/{card_id}"), None).await;
    assert_eq!(delete_response.status(), StatusCode::OK);

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
async fn test_delete_card_flat_returns_200_with_the_invalidation_naming_the_card() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, card_id) = seed_board_and_card(&state).await;

    let response = send(&state, "DELETE", &format!("/v1/cards/{card_id}"), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    let invalidated_cards = body["invalidation"]["entities"]["cards"]
        .as_array()
        .expect("entities invalidation must name cards");
    assert!(invalidated_cards.iter().any(|v| v == &card_id.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_flat_missing_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let unknown_card = Uuid::new_v4();

    let response = send(&state, "GET", &format!("/v1/cards/{unknown_card}"), None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_sprint_flat_returns_same_shape_as_board_scoped() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;

    let board_scoped_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}"),
        None,
    )
    .await;
    let flat_response = send(&state, "GET", &format!("/v1/sprints/{sprint_id}"), None).await;

    assert_eq!(board_scoped_response.status(), StatusCode::OK);
    assert_eq!(flat_response.status(), StatusCode::OK);

    let board_scoped_json = json_of(board_scoped_response).await;
    let flat_json = json_of(flat_response).await;

    assert_eq!(board_scoped_json["id"], flat_json["id"]);
    assert_eq!(board_scoped_json["board_id"], flat_json["board_id"]);
    assert_eq!(board_scoped_json["name"], flat_json["name"]);
    assert_eq!(
        board_scoped_json["sprint_number"],
        flat_json["sprint_number"]
    );
    assert_eq!(board_scoped_json["prefix"], flat_json["prefix"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_sprint_flat_updates_and_matches_board_scoped_route() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;

    let flat_response = send(
        &state,
        "PATCH",
        &format!("/v1/sprints/{sprint_id}"),
        Some(&json!({"name": "Updated via flat"})),
    )
    .await;

    assert_eq!(flat_response.status(), StatusCode::OK);
    let flat_json = json_of(flat_response).await;
    assert_eq!(flat_json["name"], "Updated via flat");

    let verify_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}"),
        None,
    )
    .await;
    let verify_json = json_of(verify_response).await;
    assert_eq!(verify_json["name"], "Updated via flat");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_sprint_flat_deletes() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;

    let delete_response = send(&state, "DELETE", &format!("/v1/sprints/{sprint_id}"), None).await;
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let verify_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}"),
        None,
    )
    .await;
    assert_eq!(verify_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_sprint_flat_missing_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let unknown_sprint = Uuid::new_v4();

    let response = send(
        &state,
        "GET",
        &format!("/v1/sprints/{unknown_sprint}"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_flat_serves_an_archived_card_from_the_per_id_tier_without_stamping_archived_at(
) {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let card_id: Uuid;
    {
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
        card_id = card.id;
        ctx.archive_card(card_id).unwrap();
    }

    let response = send(&state, "GET", &format!("/v1/cards/{card_id}"), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert!(
        json.get("archived_at").is_none(),
        "flat get_card must not stamp archived_at"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_sprint_flat_unknown_id_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let unknown_sprint = Uuid::new_v4();

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/sprints/{unknown_sprint}"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_flat_carries_etag_header_and_matching_if_none_match_returns_304() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, card_id) = seed_board_and_card(&state).await;

    let uri = format!("/v1/cards/{card_id}");
    let first = send(&state, "GET", &uri, None).await;
    assert_eq!(first.status(), StatusCode::OK);
    let tag = etag_of(&first);
    assert!(
        is_quoted_32_hex(&tag),
        "expected quoted 32-hex etag, got {tag}"
    );

    let second = send_with_headers(&state, "GET", &uri, None, &[("if-none-match", &tag)]).await;
    assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(etag_of(&second), tag);
    let bytes = axum::body::to_bytes(second.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(bytes.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_column_flat_carries_etag_header_and_matching_if_none_match_returns_304() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, col_id) = seed_board_and_column(&state).await;

    let uri = format!("/v1/columns/{col_id}");
    let first = send(&state, "GET", &uri, None).await;
    assert_eq!(first.status(), StatusCode::OK);
    let tag = etag_of(&first);
    assert!(
        is_quoted_32_hex(&tag),
        "expected quoted 32-hex etag, got {tag}"
    );

    let second = send_with_headers(&state, "GET", &uri, None, &[("if-none-match", &tag)]).await;
    assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(etag_of(&second), tag);
    let bytes = axum::body::to_bytes(second.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(bytes.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_card_flat_with_the_get_etag_succeeds_then_the_reused_etag_returns_412() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, card_id) = seed_board_and_card(&state).await;

    let get_response = send(&state, "GET", &format!("/v1/cards/{card_id}"), None).await;
    let tag = etag_of(&get_response);

    let first = send_with_headers(
        &state,
        "PATCH",
        &format!("/v1/cards/{card_id}"),
        Some(&json!({"title": "Renamed Once"})),
        &[("if-match", &tag)],
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let second = send_with_headers(
        &state,
        "PATCH",
        &format!("/v1/cards/{card_id}"),
        Some(&json!({"title": "Renamed Twice"})),
        &[("if-match", &tag)],
    )
    .await;
    assert_eq!(second.status(), StatusCode::PRECONDITION_FAILED);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_card_flat_with_stale_if_match_returns_412() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, card_id) = seed_board_and_card(&state).await;

    let response = send_with_headers(
        &state,
        "DELETE",
        &format!("/v1/cards/{card_id}"),
        None,
        &[("if-match", STALE_IF_MATCH)],
    )
    .await;

    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);

    let get_response = send(&state, "GET", &format!("/v1/cards/{card_id}"), None).await;
    assert_eq!(get_response.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_column_flat_with_the_get_etag_succeeds_then_the_reused_etag_returns_412() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, col_id) = seed_board_and_column(&state).await;

    let get_response = send(&state, "GET", &format!("/v1/columns/{col_id}"), None).await;
    let tag = etag_of(&get_response);

    let first = send_with_headers(
        &state,
        "PATCH",
        &format!("/v1/columns/{col_id}"),
        Some(&json!({"name": "Renamed Once"})),
        &[("if-match", &tag)],
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let second = send_with_headers(
        &state,
        "PATCH",
        &format!("/v1/columns/{col_id}"),
        Some(&json!({"name": "Renamed Twice"})),
        &[("if-match", &tag)],
    )
    .await;
    assert_eq!(second.status(), StatusCode::PRECONDITION_FAILED);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_column_flat_with_stale_if_match_returns_412() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, col_id) = seed_board_and_column(&state).await;

    let response = send_with_headers(
        &state,
        "DELETE",
        &format!("/v1/columns/{col_id}"),
        None,
        &[("if-match", STALE_IF_MATCH)],
    )
    .await;

    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);

    let get_response = send(&state, "GET", &format!("/v1/columns/{col_id}"), None).await;
    assert_eq!(get_response.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_sprint_flat_carries_etag_header_and_matching_if_none_match_returns_304() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;

    let uri = format!("/v1/sprints/{sprint_id}");
    let first = send(&state, "GET", &uri, None).await;
    assert_eq!(first.status(), StatusCode::OK);
    let tag = etag_of(&first);
    assert!(
        is_quoted_32_hex(&tag),
        "expected quoted 32-hex etag, got {tag}"
    );

    let second = send_with_headers(&state, "GET", &uri, None, &[("if-none-match", &tag)]).await;
    assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(etag_of(&second), tag);
    let bytes = axum::body::to_bytes(second.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(bytes.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_sprint_flat_with_the_get_etag_succeeds_then_the_reused_etag_returns_412() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;

    let get_response = send(&state, "GET", &format!("/v1/sprints/{sprint_id}"), None).await;
    let tag = etag_of(&get_response);

    let first = send_with_headers(
        &state,
        "PATCH",
        &format!("/v1/sprints/{sprint_id}"),
        Some(&json!({"name": "Renamed Once"})),
        &[("if-match", &tag)],
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let second = send_with_headers(
        &state,
        "PATCH",
        &format!("/v1/sprints/{sprint_id}"),
        Some(&json!({"name": "Renamed Twice"})),
        &[("if-match", &tag)],
    )
    .await;
    assert_eq!(second.status(), StatusCode::PRECONDITION_FAILED);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_sprint_flat_with_stale_if_match_returns_412() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;

    let response = send_with_headers(
        &state,
        "DELETE",
        &format!("/v1/sprints/{sprint_id}"),
        None,
        &[("if-match", STALE_IF_MATCH)],
    )
    .await;

    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);

    let get_response = send(&state, "GET", &format!("/v1/sprints/{sprint_id}"), None).await;
    assert_eq!(get_response.status(), StatusCode::OK);
}
