#![cfg(feature = "test-helpers")]

use axum::http::StatusCode;
use kanban_server::state::AppState;
use kanban_server::test_helpers::{json_of, make_sqlite_state, make_state, send};
use kanban_service::api::{ArchivedCardResponse, ChangeEventFrame, ChangeKind, EntityType, Page};
use kanban_service::KanbanOperations;
use serde_json::Value;
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::broadcast;
use uuid::Uuid;

async fn next_frame(rx: &mut broadcast::Receiver<ChangeEventFrame>) -> ChangeEventFrame {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out waiting for a change event frame")
        .expect("broadcast channel closed unexpectedly")
}

async fn seed(state: &AppState) -> (Uuid, Uuid, Uuid, Uuid) {
    let mut ctx = state.ctx.lock().await;
    let board_id = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id;
    let col_id = ctx
        .create_column(board_id, "To Do".to_string(), None)
        .unwrap()
        .id;
    let other_col_id = ctx
        .create_column(board_id, "Doing".to_string(), None)
        .unwrap()
        .id;
    let card_id = ctx
        .create_card(board_id, col_id, "Task".to_string(), Default::default())
        .unwrap()
        .id;
    (board_id, col_id, other_col_id, card_id)
}

async fn scenario_archive_stamps_archived_at(state: AppState) {
    let (_board_id, _col_id, _other_col_id, card_id) = seed(&state).await;

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{}/archive", card_id),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = json_of(response).await;
    assert_eq!(body["id"], card_id.to_string());
    assert!(body["archived_at"].is_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_card_returns_200_with_archived_at_stamped_json() {
    let dir = tempdir().unwrap();
    scenario_archive_stamps_archived_at(make_state(&dir.path().join("s.json"))).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_card_returns_200_with_archived_at_stamped_sqlite() {
    let dir = tempdir().unwrap();
    scenario_archive_stamps_archived_at(make_sqlite_state(&dir.path().join("s.sqlite")).await)
        .await;
}

async fn scenario_archive_then_restore_round_trips(state: AppState) {
    let (board_id, col_id, _other_col_id, card_id) = seed(&state).await;

    let before_resp = send(&state, "GET", &format!("/v1/cards/{}", card_id), None).await;
    assert_eq!(before_resp.status(), StatusCode::OK);
    let mut before: Value = json_of(before_resp).await;

    let archive_resp = send(
        &state,
        "POST",
        &format!("/v1/cards/{}/archive", card_id),
        None,
    )
    .await;
    assert_eq!(archive_resp.status(), StatusCode::OK);

    let restore_resp = send(
        &state,
        "POST",
        &format!("/v1/cards/{}/restore", card_id),
        None,
    )
    .await;
    assert_eq!(restore_resp.status(), StatusCode::OK);
    let restore_body: Value = json_of(restore_resp).await;
    assert!(restore_body.get("archived_at").is_none());

    let after_resp = send(&state, "GET", &format!("/v1/cards/{}", card_id), None).await;
    assert_eq!(after_resp.status(), StatusCode::OK);
    let mut after: Value = json_of(after_resp).await;

    before.as_object_mut().unwrap().remove("updated_at");
    after.as_object_mut().unwrap().remove("updated_at");
    assert_eq!(before, after);

    let live_list_resp = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards", board_id),
        None,
    )
    .await;
    let live_page: Value = json_of(live_list_resp).await;
    let items = live_page["items"].as_array().unwrap();
    let restored = items
        .iter()
        .find(|c| c["id"] == card_id.to_string())
        .expect("restored card should be present in the live list");
    assert_eq!(restored["column_id"], col_id.to_string());

    let archived_list_resp = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/archived-cards", board_id),
        None,
    )
    .await;
    let archived_page: Page<ArchivedCardResponse> =
        serde_json::from_value(json_of(archived_list_resp).await).unwrap();
    assert!(archived_page.items.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_then_restore_round_trips_the_card_over_the_wire_json() {
    let dir = tempdir().unwrap();
    scenario_archive_then_restore_round_trips(make_state(&dir.path().join("s.json"))).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_then_restore_round_trips_the_card_over_the_wire_sqlite() {
    let dir = tempdir().unwrap();
    scenario_archive_then_restore_round_trips(
        make_sqlite_state(&dir.path().join("s.sqlite")).await,
    )
    .await;
}

async fn scenario_archive_appears_in_archived_list_and_leaves_live_list(state: AppState) {
    let (board_id, _col_id, _other_col_id, card_id) = seed(&state).await;

    let archive_resp = send(
        &state,
        "POST",
        &format!("/v1/cards/{}/archive", card_id),
        None,
    )
    .await;
    assert_eq!(archive_resp.status(), StatusCode::OK);

    let archived_list_resp = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/archived-cards", board_id),
        None,
    )
    .await;
    let archived_page: Page<ArchivedCardResponse> =
        serde_json::from_value(json_of(archived_list_resp).await).unwrap();
    assert_eq!(archived_page.items.len(), 1);
    assert_eq!(archived_page.items[0].entity_id, card_id);
    assert_eq!(archived_page.items[0].board_id, board_id);
    assert!(archived_page.items[0].archived_at <= chrono::Utc::now());

    let live_list_resp = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards", board_id),
        None,
    )
    .await;
    let live_page: Value = json_of(live_list_resp).await;
    let items = live_page["items"].as_array().unwrap();
    assert!(!items.iter().any(|c| c["id"] == card_id.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_card_appears_in_archived_cards_list_and_leaves_the_live_list_json() {
    let dir = tempdir().unwrap();
    scenario_archive_appears_in_archived_list_and_leaves_live_list(make_state(
        &dir.path().join("s.json"),
    ))
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_card_appears_in_archived_cards_list_and_leaves_the_live_list_sqlite() {
    let dir = tempdir().unwrap();
    scenario_archive_appears_in_archived_list_and_leaves_live_list(
        make_sqlite_state(&dir.path().join("s.sqlite")).await,
    )
    .await;
}

async fn scenario_archive_already_archived_returns_404(state: AppState) {
    let (_board_id, _col_id, _other_col_id, card_id) = seed(&state).await;

    let first = send(
        &state,
        "POST",
        &format!("/v1/cards/{}/archive", card_id),
        None,
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let second = send(
        &state,
        "POST",
        &format!("/v1/cards/{}/archive", card_id),
        None,
    )
    .await;
    assert_eq!(second.status(), StatusCode::NOT_FOUND);
    let body: Value = json_of(second).await;
    assert_eq!(body["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_already_archived_card_returns_404_json() {
    let dir = tempdir().unwrap();
    scenario_archive_already_archived_returns_404(make_state(&dir.path().join("s.json"))).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_already_archived_card_returns_404_sqlite() {
    let dir = tempdir().unwrap();
    scenario_archive_already_archived_returns_404(
        make_sqlite_state(&dir.path().join("s.sqlite")).await,
    )
    .await;
}

async fn scenario_restore_with_column_id_moves_card(state: AppState) {
    let (board_id, _col_id, other_col_id, card_id) = seed(&state).await;

    let archive_resp = send(
        &state,
        "POST",
        &format!("/v1/cards/{}/archive", card_id),
        None,
    )
    .await;
    assert_eq!(archive_resp.status(), StatusCode::OK);

    let restore_resp = send(
        &state,
        "POST",
        &format!("/v1/cards/{}/restore?column_id={}", card_id, other_col_id),
        None,
    )
    .await;
    assert_eq!(restore_resp.status(), StatusCode::OK);
    let body: Value = json_of(restore_resp).await;
    assert_eq!(body["column_id"], other_col_id.to_string());

    let list_resp = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards?column_id={}", board_id, other_col_id),
        None,
    )
    .await;
    let page: Value = json_of(list_resp).await;
    let items = page["items"].as_array().unwrap();
    assert!(items.iter().any(|c| c["id"] == card_id.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_with_column_id_query_moves_card_to_that_column_json() {
    let dir = tempdir().unwrap();
    scenario_restore_with_column_id_moves_card(make_state(&dir.path().join("s.json"))).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_with_column_id_query_moves_card_to_that_column_sqlite() {
    let dir = tempdir().unwrap();
    scenario_restore_with_column_id_moves_card(
        make_sqlite_state(&dir.path().join("s.sqlite")).await,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_missing_card_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let random_id = Uuid::new_v4();
    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{}/archive", random_id),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: Value = json_of(response).await;
    assert_eq!(body["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_live_card_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, _col_id, _other_col_id, card_id) = seed(&state).await;

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{}/restore", card_id),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: Value = json_of(response).await;
    assert_eq!(body["code"], "NOT_FOUND");
    assert!(body["message"].as_str().unwrap().contains("archived card"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_with_missing_column_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, _col_id, _other_col_id, card_id) = seed(&state).await;

    let archive_resp = send(
        &state,
        "POST",
        &format!("/v1/cards/{}/archive", card_id),
        None,
    )
    .await;
    assert_eq!(archive_resp.status(), StatusCode::OK);

    let random_col = Uuid::new_v4();
    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{}/restore?column_id={}", card_id, random_col),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body: Value = json_of(response).await;
    assert_eq!(body["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_and_restore_emit_card_updated_frames() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, _col_id, _other_col_id, card_id) = seed(&state).await;
    let mut rx = state.event_tx.subscribe();

    let archive_resp = send(
        &state,
        "POST",
        &format!("/v1/cards/{}/archive", card_id),
        None,
    )
    .await;
    assert_eq!(archive_resp.status(), StatusCode::OK);

    let archive_frame = next_frame(&mut rx).await;
    assert_eq!(archive_frame.entity_type, Some(EntityType::Card));
    assert_eq!(archive_frame.entity_id, Some(card_id));
    assert_eq!(archive_frame.kind, Some(ChangeKind::Updated));
    assert_eq!(archive_frame.writer_instance_id, state.instance_id);

    let restore_resp = send(
        &state,
        "POST",
        &format!("/v1/cards/{}/restore", card_id),
        None,
    )
    .await;
    assert_eq!(restore_resp.status(), StatusCode::OK);

    let restore_frame = next_frame(&mut rx).await;
    assert_eq!(restore_frame.entity_type, Some(EntityType::Card));
    assert_eq!(restore_frame.entity_id, Some(card_id));
    assert_eq!(restore_frame.kind, Some(ChangeKind::Updated));
    assert_eq!(restore_frame.writer_instance_id, state.instance_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_is_visible_to_subsequent_reads() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, _col_id, _other_col_id, card_id) = seed(&state).await;

    let before_resp = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards", board_id),
        None,
    )
    .await;
    let before: Value = json_of(before_resp).await;
    assert!(before["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["id"] == card_id.to_string()));

    let archive_resp = send(
        &state,
        "POST",
        &format!("/v1/cards/{}/archive", card_id),
        None,
    )
    .await;
    assert_eq!(archive_resp.status(), StatusCode::OK);

    let after_resp = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards", board_id),
        None,
    )
    .await;
    let after: Value = json_of(after_resp).await;
    assert!(!after["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["id"] == card_id.to_string()));

    let archived_only_resp = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards?archived=archived_only", board_id),
        None,
    )
    .await;
    let archived_only: Value = json_of(archived_only_resp).await;
    let found = archived_only["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == card_id.to_string())
        .expect("archived card should be present in archived_only filter");
    assert!(found["archived_at"].is_string());
}
