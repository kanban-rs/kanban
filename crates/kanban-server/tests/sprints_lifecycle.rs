#![cfg(feature = "test-helpers")]

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use kanban_domain::{CardStatus, CardUpdate, CreateCardOptions, KanbanOperations};
use kanban_server::state::AppState;
use kanban_server::test_helpers::{json_of, make_sqlite_state, make_state, send};
use serde_json::json;
use tempfile::tempdir;
use uuid::Uuid;

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

async fn seed_board_column_and_two_sprints(state: &AppState) -> (Uuid, Uuid, Uuid, Uuid) {
    let mut ctx = state.ctx.lock().await;
    let board_id = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id;
    let column_id = ctx
        .create_column(board_id, "Todo".to_string(), None)
        .unwrap()
        .id;
    let sprint1 = ctx
        .create_sprint(board_id, Some("SPR".to_string()), Some("One".to_string()))
        .unwrap()
        .id;
    let sprint2 = ctx
        .create_sprint(board_id, Some("SPR".to_string()), Some("Two".to_string()))
        .unwrap()
        .id;
    (board_id, column_id, sprint1, sprint2)
}

fn parse_date(v: &serde_json::Value) -> DateTime<Utc> {
    v.as_str()
        .unwrap_or_else(|| panic!("expected a date string, got {v}"))
        .parse()
        .unwrap_or_else(|e| panic!("failed to parse date {v}: {e}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_activate_sprint_route_sets_status_active_and_dates() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}/activate"),
        Some(&json!({"duration_days": 7})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["status"], "active");
    let start = parse_date(&json["start_date"]);
    let end = parse_date(&json["end_date"]);
    assert_eq!((end - start).num_days(), 7);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_activate_sprint_route_with_empty_body_defaults_to_14_days() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}/activate"),
        Some(&json!({})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let start = parse_date(&json["start_date"]);
    let end = parse_date(&json["end_date"]);
    assert_eq!((end - start).num_days(), 14);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_complete_sprint_route_sets_status_completed() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}/complete"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["status"], "completed");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_cancel_sprint_route_sets_status_cancelled() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}/cancel"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["status"], "cancelled");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_activate_already_active_sprint_succeeds_and_resets_dates() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;

    let first = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}/activate"),
        Some(&json!({"duration_days": 7})),
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_json = json_of(first).await;
    let first_start = parse_date(&first_json["start_date"]);

    let second = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}/activate"),
        Some(&json!({"duration_days": 7})),
    )
    .await;
    assert_eq!(second.status(), StatusCode::OK);
    let second_json = json_of(second).await;
    let second_start = parse_date(&second_json["start_date"]);
    let first_end = parse_date(&first_json["end_date"]);
    let second_end = parse_date(&second_json["end_date"]);

    assert!(
        second_start > first_start,
        "re-activating must reset start_date: {first_start} -> {second_start}"
    );
    assert!(
        second_end > first_end,
        "re-activating must reset end_date: {first_end} -> {second_end}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_activate_sprint_route_with_negative_duration_returns_422_validation_failed() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}/activate"),
        Some(&json!({"duration_days": -1})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = json_of(response).await;
    assert_eq!(json["code"], "VALIDATION_FAILED");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_lifecycle_routes_on_sprint_of_other_board_return_404_not_found() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_a, sprint_a) = seed_board_and_sprint(&state, "Alpha").await;
    let board_b = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Other".to_string(), Some("OTH".to_string()))
            .unwrap()
            .id;
        ctx.create_column(board_id, "Todo".to_string(), None)
            .unwrap();
        board_id
    };

    for path in ["activate", "complete", "cancel"] {
        let response = send(
            &state,
            "POST",
            &format!("/v1/boards/{board_b}/sprints/{sprint_a}/{path}"),
            Some(&json!({})),
        )
        .await;
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "path {path} expected 404"
        );
        let json = json_of(response).await;
        assert_eq!(json["code"], "NOT_FOUND", "path {path}");
    }

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_b}/sprints/{sprint_a}/carry-over"),
        Some(&json!({"to_sprint_id": Uuid::new_v4()})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_activate_missing_sprint_returns_404_not_found_code() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{}/activate", Uuid::new_v4()),
        Some(&json!({})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_carry_over_moves_uncompleted_cards_to_planning_sprint() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, column_id, sprint1, sprint2) = seed_board_column_and_two_sprints(&state).await;

    let (todo_card_id, done_card_id) = {
        let mut ctx = state.ctx.lock().await;
        let todo = ctx
            .create_card(
                board_id,
                column_id,
                "Todo card".to_string(),
                CreateCardOptions {
                    sprint_id: Some(sprint1),
                    ..Default::default()
                },
            )
            .unwrap()
            .id;
        let done = ctx
            .create_card(
                board_id,
                column_id,
                "Done card".to_string(),
                CreateCardOptions {
                    sprint_id: Some(sprint1),
                    ..Default::default()
                },
            )
            .unwrap()
            .id;
        ctx.update_card(
            done,
            CardUpdate {
                status: Some(CardStatus::Done),
                ..Default::default()
            },
        )
        .unwrap();
        (todo, done)
    };

    let complete = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint1}/complete"),
        None,
    )
    .await;
    assert_eq!(complete.status(), StatusCode::OK);

    let carry_over = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint1}/carry-over"),
        Some(&json!({"to_sprint_id": sprint2})),
    )
    .await;
    assert_eq!(carry_over.status(), StatusCode::OK);
    let body = json_of(carry_over).await;
    assert_eq!(body["moved"], 1);

    let to_sprint2 = json_of(
        send(
            &state,
            "GET",
            &format!("/v1/boards/{board_id}/cards?sprint_id={sprint2}"),
            None,
        )
        .await,
    )
    .await;
    let ids: Vec<String> = to_sprint2["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec![todo_card_id.to_string()]);

    let still_in_sprint1 = json_of(
        send(
            &state,
            "GET",
            &format!("/v1/boards/{board_id}/cards?sprint_id={sprint1}"),
            None,
        )
        .await,
    )
    .await;
    let ids: Vec<String> = still_in_sprint1["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec![done_card_id.to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_carry_over_moves_uncompleted_cards_to_planning_sprint_on_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("s.sqlite")).await;
    let (board_id, column_id, sprint1, sprint2) = seed_board_column_and_two_sprints(&state).await;

    let (todo_card_id, done_card_id) = {
        let mut ctx = state.ctx.lock().await;
        let todo = ctx
            .create_card(
                board_id,
                column_id,
                "Todo card".to_string(),
                CreateCardOptions {
                    sprint_id: Some(sprint1),
                    ..Default::default()
                },
            )
            .unwrap()
            .id;
        let done = ctx
            .create_card(
                board_id,
                column_id,
                "Done card".to_string(),
                CreateCardOptions {
                    sprint_id: Some(sprint1),
                    ..Default::default()
                },
            )
            .unwrap()
            .id;
        ctx.update_card(
            done,
            CardUpdate {
                status: Some(CardStatus::Done),
                ..Default::default()
            },
        )
        .unwrap();
        (todo, done)
    };

    let complete = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint1}/complete"),
        None,
    )
    .await;
    assert_eq!(complete.status(), StatusCode::OK);

    let carry_over = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint1}/carry-over"),
        Some(&json!({"to_sprint_id": sprint2})),
    )
    .await;
    assert_eq!(carry_over.status(), StatusCode::OK);
    let body = json_of(carry_over).await;
    assert_eq!(body["moved"], 1);

    let to_sprint2 = json_of(
        send(
            &state,
            "GET",
            &format!("/v1/boards/{board_id}/cards?sprint_id={sprint2}"),
            None,
        )
        .await,
    )
    .await;
    let ids: Vec<String> = to_sprint2["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec![todo_card_id.to_string()]);

    let still_in_sprint1 = json_of(
        send(
            &state,
            "GET",
            &format!("/v1/boards/{board_id}/cards?sprint_id={sprint1}"),
            None,
        )
        .await,
    )
    .await;
    let ids: Vec<String> = still_in_sprint1["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec![done_card_id.to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_carry_over_from_planning_sprint_returns_422_validation_failed() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, _column_id, sprint1, sprint2) = seed_board_column_and_two_sprints(&state).await;

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint1}/carry-over"),
        Some(&json!({"to_sprint_id": sprint2})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = json_of(response).await;
    assert_eq!(json["code"], "VALIDATION_FAILED");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_carry_over_to_non_planning_sprint_returns_422_validation_failed() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, _column_id, sprint1, sprint2) = seed_board_column_and_two_sprints(&state).await;

    let complete = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint1}/complete"),
        None,
    )
    .await;
    assert_eq!(complete.status(), StatusCode::OK);

    let activate_target = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint2}/activate"),
        Some(&json!({})),
    )
    .await;
    assert_eq!(activate_target.status(), StatusCode::OK);

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint1}/carry-over"),
        Some(&json!({"to_sprint_id": sprint2})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = json_of(response).await;
    assert_eq!(json["code"], "VALIDATION_FAILED");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_carry_over_to_sprint_of_other_board_returns_404_not_found() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_a, sprint_a) = seed_board_and_sprint(&state, "Alpha").await;
    let sprint_b = {
        let mut ctx = state.ctx.lock().await;
        let board_b = ctx
            .create_board("Other".to_string(), Some("OTH".to_string()))
            .unwrap()
            .id;
        ctx.create_sprint(board_b, Some("SPR".to_string()), Some("Other".to_string()))
            .unwrap()
            .id
    };

    let complete = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_a}/sprints/{sprint_a}/complete"),
        None,
    )
    .await;
    assert_eq!(complete.status(), StatusCode::OK);

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_a}/sprints/{sprint_a}/carry-over"),
        Some(&json!({"to_sprint_id": sprint_b})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_sprint_after_lifecycle_write_reads_fresh_status_and_resolved_name() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    let created = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints"),
        Some(&json!({"name": "Alpha", "prefix": "SPR"})),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_json = json_of(created).await;
    let sprint_id = created_json["id"].as_str().unwrap().to_string();

    let activate = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}/activate"),
        Some(&json!({})),
    )
    .await;
    assert_eq!(activate.status(), StatusCode::OK);

    let get = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}"),
        None,
    )
    .await;
    assert_eq!(get.status(), StatusCode::OK);
    let json = json_of(get).await;
    assert_eq!(json["status"], "active");
    assert_eq!(json["name"], "Alpha");
}
