#![cfg(feature = "test-helpers")]

use axum::http::StatusCode;
use kanban_domain::{ColumnUpdate, FieldUpdate, KanbanOperations};
use kanban_server::state::AppState;
use kanban_server::test_helpers::{json_of, make_sqlite_state, make_state, send};
use serde_json::json;
use std::time::Duration;
use tempfile::tempdir;
use uuid::Uuid;

async fn seed_two_cards(state: &AppState) -> (Uuid, Uuid, Uuid, Uuid) {
    let mut ctx = state.ctx.lock().await;
    let board_id = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id;
    let col_id = ctx
        .create_column(board_id, "To Do".to_string(), None)
        .unwrap()
        .id;
    let c1 = ctx
        .create_card(board_id, col_id, "One".to_string(), Default::default())
        .unwrap()
        .id;
    let c2 = ctx
        .create_card(board_id, col_id, "Two".to_string(), Default::default())
        .unwrap()
        .id;
    (board_id, col_id, c1, c2)
}

async fn scenario_batch_archive_reports_per_id_success_and_failure(state: AppState) {
    let (board_id, _col_id, c1, c2) = seed_two_cards(&state).await;
    let missing = Uuid::new_v4();

    let response = send(
        &state,
        "POST",
        "/v1/cards/batch/archive",
        Some(&json!({ "ids": [c1, c2, missing] })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    let succeeded: Vec<String> = body["succeeded"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        succeeded
            .into_iter()
            .collect::<std::collections::HashSet<_>>(),
        [c1.to_string(), c2.to_string()].into_iter().collect()
    );
    let failed = body["failed"].as_array().unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["id"], missing.to_string());
    assert!(failed[0]["error"].as_str().unwrap().contains("not found"));

    let archived_resp = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/archived-cards", board_id),
        None,
    )
    .await;
    let archived = json_of(archived_resp).await;
    let ids: Vec<String> = archived["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["entity_id"].as_str().unwrap().to_string())
        .collect();
    assert!(ids.contains(&c1.to_string()));
    assert!(ids.contains(&c2.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_batch_archive_route_reports_per_id_success_and_failure_json() {
    let dir = tempdir().unwrap();
    scenario_batch_archive_reports_per_id_success_and_failure(make_state(
        &dir.path().join("s.json"),
    ))
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_batch_archive_route_reports_per_id_success_and_failure_sqlite() {
    let dir = tempdir().unwrap();
    scenario_batch_archive_reports_per_id_success_and_failure(
        make_sqlite_state(&dir.path().join("s.sqlite")).await,
    )
    .await;
}

async fn scenario_batch_archive_path_not_captured_by_flat_card_id_route(state: AppState) {
    let (_board_id, _col_id, c1, c2) = seed_two_cards(&state).await;

    let batch_response = send(
        &state,
        "POST",
        "/v1/cards/batch/archive",
        Some(&json!({ "ids": [c1] })),
    )
    .await;
    assert_eq!(batch_response.status(), StatusCode::OK);

    let flat_response = send(&state, "POST", &format!("/v1/cards/{}/archive", c2), None).await;
    assert_eq!(flat_response.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_batch_archive_path_is_not_captured_by_the_flat_card_id_route() {
    let dir = tempdir().unwrap();
    scenario_batch_archive_path_not_captured_by_flat_card_id_route(make_state(
        &dir.path().join("s.json"),
    ))
    .await;
}

async fn scenario_batch_archive_route_with_empty_ids_returns_empty_result(state: AppState) {
    let response = send(
        &state,
        "POST",
        "/v1/cards/batch/archive",
        Some(&json!({ "ids": [] })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert_eq!(body["succeeded"].as_array().unwrap().len(), 0);
    assert_eq!(body["failed"].as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_batch_archive_route_with_empty_ids_returns_empty_result() {
    let dir = tempdir().unwrap();
    scenario_batch_archive_route_with_empty_ids_returns_empty_result(make_state(
        &dir.path().join("s.json"),
    ))
    .await;
}

async fn scenario_batch_move_route_moves_valid_ids_into_target_column(state: AppState) {
    let (board_id, col_a, c1, c2) = seed_two_cards(&state).await;
    let mut ctx = state.ctx.lock().await;
    let col_b = ctx
        .create_column(board_id, "Doing".to_string(), None)
        .unwrap()
        .id;
    drop(ctx);
    let _ = col_a;

    let response = send(
        &state,
        "POST",
        "/v1/cards/batch/move",
        Some(&json!({ "ids": [c1, c2], "column_id": col_b })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert_eq!(body["succeeded"].as_array().unwrap().len(), 2);
    assert_eq!(body["failed"].as_array().unwrap().len(), 0);

    for id in [c1, c2] {
        let card_resp = send(&state, "GET", &format!("/v1/cards/{}", id), None).await;
        let card = json_of(card_resp).await;
        assert_eq!(card["column_id"], col_b.to_string());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_batch_move_route_moves_valid_ids_into_target_column() {
    let dir = tempdir().unwrap();
    scenario_batch_move_route_moves_valid_ids_into_target_column(make_state(
        &dir.path().join("s.json"),
    ))
    .await;
}

async fn scenario_batch_move_route_wip_violation_fails_every_id_and_changes_nothing(
    state: AppState,
) {
    let (board_id, col_a, c1, c2) = seed_two_cards(&state).await;
    let mut ctx = state.ctx.lock().await;
    let col_b = ctx
        .create_column(board_id, "Doing".to_string(), None)
        .unwrap()
        .id;
    ctx.update_column(
        col_b,
        ColumnUpdate {
            wip_limit: FieldUpdate::Set(1),
            ..Default::default()
        },
    )
    .unwrap();
    drop(ctx);

    let response = send(
        &state,
        "POST",
        "/v1/cards/batch/move",
        Some(&json!({ "ids": [c1, c2], "column_id": col_b })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert_eq!(body["succeeded"].as_array().unwrap().len(), 0);
    let failed = body["failed"].as_array().unwrap();
    assert_eq!(failed.len(), 2);
    for f in failed {
        assert!(f["error"].as_str().unwrap().to_lowercase().contains("wip"));
    }

    for id in [c1, c2] {
        let card_resp = send(&state, "GET", &format!("/v1/cards/{}", id), None).await;
        let card = json_of(card_resp).await;
        assert_eq!(card["column_id"], col_a.to_string());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_batch_move_route_wip_violation_fails_every_id_and_changes_nothing_json() {
    let dir = tempdir().unwrap();
    scenario_batch_move_route_wip_violation_fails_every_id_and_changes_nothing(make_state(
        &dir.path().join("s.json"),
    ))
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_batch_move_route_wip_violation_fails_every_id_and_changes_nothing_sqlite() {
    let dir = tempdir().unwrap();
    scenario_batch_move_route_wip_violation_fails_every_id_and_changes_nothing(
        make_sqlite_state(&dir.path().join("s.sqlite")).await,
    )
    .await;
}

async fn scenario_batch_assign_sprint_route_missing_sprint_fails_every_id(state: AppState) {
    let (_board_id, _col_id, c1, c2) = seed_two_cards(&state).await;
    let missing_sprint = Uuid::new_v4();

    let response = send(
        &state,
        "POST",
        "/v1/cards/batch/assign-sprint",
        Some(&json!({ "ids": [c1, c2], "sprint_id": missing_sprint })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert_eq!(body["succeeded"].as_array().unwrap().len(), 0);
    let failed = body["failed"].as_array().unwrap();
    assert_eq!(failed.len(), 2);
    for f in failed {
        assert!(f["error"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("not found"));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_batch_assign_sprint_route_missing_sprint_fails_every_id() {
    let dir = tempdir().unwrap();
    scenario_batch_assign_sprint_route_missing_sprint_fails_every_id(make_state(
        &dir.path().join("s.json"),
    ))
    .await;
}

async fn scenario_batch_assign_sprint_route_assigns_and_is_visible_via_sprint_filter(
    state: AppState,
) {
    let (board_id, _col_id, c1, c2) = seed_two_cards(&state).await;
    let mut ctx = state.ctx.lock().await;
    let sprint_id = ctx
        .create_sprint(board_id, None, Some("Sprint 1".to_string()))
        .unwrap()
        .id;
    drop(ctx);

    let response = send(
        &state,
        "POST",
        "/v1/cards/batch/assign-sprint",
        Some(&json!({ "ids": [c1, c2], "sprint_id": sprint_id })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert_eq!(body["succeeded"].as_array().unwrap().len(), 2);
    assert_eq!(body["failed"].as_array().unwrap().len(), 0);

    let list_resp = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards?sprint_id={}", board_id, sprint_id),
        None,
    )
    .await;
    let page = json_of(list_resp).await;
    let ids: std::collections::HashSet<String> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, [c1.to_string(), c2.to_string()].into_iter().collect());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_batch_assign_sprint_route_assigns_and_is_visible_via_sprint_filter() {
    let dir = tempdir().unwrap();
    scenario_batch_assign_sprint_route_assigns_and_is_visible_via_sprint_filter(make_state(
        &dir.path().join("s.json"),
    ))
    .await;
}

async fn scenario_batch_update_route_applies_all_updates_atomically(state: AppState) {
    let (_board_id, _col_id, c1, c2) = seed_two_cards(&state).await;

    let response = send(
        &state,
        "POST",
        "/v1/cards/batch/update",
        Some(&json!({
            "updates": [
                { "id": c1, "title": "One Renamed" },
                { "id": c2, "priority": "high" }
            ]
        })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    let succeeded: Vec<String> = body["succeeded"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(succeeded, vec![c1.to_string(), c2.to_string()]);
    assert_eq!(body["failed"].as_array().unwrap().len(), 0);

    let card1 = json_of(send(&state, "GET", &format!("/v1/cards/{}", c1), None).await).await;
    assert_eq!(card1["title"], "One Renamed");
    let card2 = json_of(send(&state, "GET", &format!("/v1/cards/{}", c2), None).await).await;
    assert_eq!(card2["priority"], "high");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_batch_update_route_applies_all_updates_atomically() {
    let dir = tempdir().unwrap();
    scenario_batch_update_route_applies_all_updates_atomically(make_state(
        &dir.path().join("s.json"),
    ))
    .await;
}

async fn scenario_batch_update_route_unknown_id_returns_404_and_applies_nothing(state: AppState) {
    let (_board_id, _col_id, c1, _c2) = seed_two_cards(&state).await;
    let missing = Uuid::new_v4();

    let response = send(
        &state,
        "POST",
        "/v1/cards/batch/update",
        Some(&json!({
            "updates": [
                { "id": c1, "title": "Changed" },
                { "id": missing, "title": "X" }
            ]
        })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = json_of(response).await;
    assert_eq!(body["code"], "NOT_FOUND");

    let card1 = json_of(send(&state, "GET", &format!("/v1/cards/{}", c1), None).await).await;
    assert_eq!(card1["title"], "One");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_batch_update_route_unknown_id_returns_404_envelope_and_applies_nothing_json() {
    let dir = tempdir().unwrap();
    scenario_batch_update_route_unknown_id_returns_404_and_applies_nothing(make_state(
        &dir.path().join("s.json"),
    ))
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_batch_update_route_unknown_id_returns_404_envelope_and_applies_nothing_sqlite() {
    let dir = tempdir().unwrap();
    scenario_batch_update_route_unknown_id_returns_404_and_applies_nothing(
        make_sqlite_state(&dir.path().join("s.sqlite")).await,
    )
    .await;
}

async fn scenario_batch_route_rejects_malformed_body_with_validation_envelope(state: AppState) {
    let response = send(
        &state,
        "POST",
        "/v1/cards/batch/archive",
        Some(&json!({ "ids": "not-an-array" })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = json_of(response).await;
    assert_eq!(body["code"], "VALIDATION_FAILED");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_batch_route_rejects_malformed_body_with_validation_envelope() {
    let dir = tempdir().unwrap();
    scenario_batch_route_rejects_malformed_body_with_validation_envelope(make_state(
        &dir.path().join("s.json"),
    ))
    .await;
}

async fn scenario_batch_archive_emits_one_change_frame_per_succeeded_card(state: AppState) {
    let (_board_id, _col_id, c1, c2) = seed_two_cards(&state).await;
    let missing = Uuid::new_v4();
    let mut rx = state.event_tx.subscribe();

    let response = send(
        &state,
        "POST",
        "/v1/cards/batch/archive",
        Some(&json!({ "ids": [c1, c2, missing] })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let mut seen = std::collections::HashSet::new();
    for _ in 0..2 {
        let frame = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("timed out waiting for a change event frame")
            .expect("broadcast channel closed unexpectedly");
        assert_eq!(
            frame.entity_type.unwrap(),
            kanban_service::api::EntityType::Card
        );
        assert_eq!(
            frame.kind.unwrap(),
            kanban_service::api::ChangeKind::Updated
        );
        seen.insert(frame.entity_id.unwrap());
    }
    assert_eq!(seen, std::collections::HashSet::from([c1, c2]));

    let extra = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await;
    assert!(extra.is_err(), "expected no extra frame, got one");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_batch_archive_emits_one_change_frame_per_succeeded_card() {
    let dir = tempdir().unwrap();
    scenario_batch_archive_emits_one_change_frame_per_succeeded_card(make_state(
        &dir.path().join("s.json"),
    ))
    .await;
}
