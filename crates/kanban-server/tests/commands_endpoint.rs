//! Batch commands endpoint tests (POST /v1/commands).
//! Tests verify that CommandBatch execution is atomic, broadcasts exactly once
//! with the request's correlation_id/issued_by, and properly handles errors.

use axum::http::StatusCode;
use kanban_core::ClientId;
use kanban_domain::commands::{CardCommand, Command, CreateCard};
use kanban_domain::{CardPriority, CommandBatch, CreateCardOptions};
use serde_json::json;
use tempfile::tempdir;
use uuid::Uuid;

mod common;
use common::{json_of, make_state, send};
use kanban_domain::KanbanOperations;
use kanban_server::state::AppState;

async fn seed_board_and_column(state: &AppState, name: &str) -> (Uuid, Uuid) {
    let mut ctx = state.ctx.lock().await;
    let board_id = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id;
    let col = ctx.create_column(board_id, name.to_string(), None).unwrap();
    (board_id, col.id)
}

fn make_create_card_command(board_id: Uuid, column_id: Uuid) -> Command {
    Command::Card(CardCommand::Create(CreateCard {
        id: Uuid::new_v4(),
        card_number: 1,
        board_id,
        column_id,
        title: "Task".to_string(),
        position: 0,
        options: CreateCardOptions {
            description: None,
            priority: Some(CardPriority::Medium),
            due_date: None,
            points: None,
            sprint_id: None,
        },
        timestamp: chrono::Utc::now(),
    }))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_commands_executes_batch_returns_executed_count() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, column_id) = seed_board_and_column(&state, "To Do").await;

    let cmd1 = make_create_card_command(board_id, column_id);
    let cmd2 = make_create_card_command(board_id, column_id);
    let batch = CommandBatch::wrap(vec![cmd1, cmd2], ClientId::nil());
    let batch_json = serde_json::to_value(&batch).unwrap();

    let response = send(&state, "POST", "/v1/commands", Some(&batch_json)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let resp_body = json_of(response).await;
    assert_eq!(resp_body["executed"], 2);

    let ctx = state.ctx.lock().await;
    let cards = ctx.list_cards(Default::default()).unwrap();
    assert_eq!(cards.len(), 2, "both commands should have been executed");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_commands_is_atomic_partial_failure_rolls_back_whole_batch() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, column_id) = seed_board_and_column(&state, "To Do").await;

    let valid_cmd = make_create_card_command(board_id, column_id);
    let invalid_cmd = make_create_card_command(board_id, Uuid::new_v4());
    let batch = CommandBatch::wrap(vec![valid_cmd, invalid_cmd], ClientId::nil());
    let batch_json = serde_json::to_value(&batch).unwrap();

    let response = send(&state, "POST", "/v1/commands", Some(&batch_json)).await;

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "invalid column should surface as a 404 (KanbanError::not_found)"
    );

    let ctx = state.ctx.lock().await;
    let cards = ctx.list_cards(Default::default()).unwrap();
    assert_eq!(
        cards.len(),
        0,
        "no cards should exist after failed batch (rollback verified)"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_commands_broadcasts_one_frame_per_batch_with_correlation_id() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, column_id) = seed_board_and_column(&state, "To Do").await;

    let issued_by = ClientId::new();
    let cmd = make_create_card_command(board_id, column_id);
    let batch = CommandBatch::wrap(vec![cmd], issued_by);
    let batch_correlation_id = batch.correlation_id;
    let batch_issued_by = batch.issued_by;
    let batch_json = serde_json::to_value(&batch).unwrap();

    let mut rx = state.event_tx.subscribe();

    let response = send(&state, "POST", "/v1/commands", Some(&batch_json)).await;

    assert_eq!(response.status(), StatusCode::OK);

    let frame = rx.try_recv().expect("should receive one broadcast frame");
    assert_eq!(
        frame.correlation_id, batch_correlation_id,
        "frame correlation_id should match request batch"
    );
    assert_eq!(
        frame.issued_by, batch_issued_by,
        "frame issued_by should match request batch"
    );

    assert!(
        rx.try_recv().is_err(),
        "should only receive exactly one broadcast frame"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_commands_invalid_command_maps_to_error_status() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, _) = seed_board_and_column(&state, "To Do").await;
    let nonexistent_column = Uuid::new_v4();

    let cmd = make_create_card_command(board_id, nonexistent_column);
    let batch = CommandBatch::wrap(vec![cmd], ClientId::nil());
    let batch_json = serde_json::to_value(&batch).unwrap();

    let mut rx = state.event_tx.subscribe();

    let response = send(&state, "POST", "/v1/commands", Some(&batch_json)).await;

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "invalid command should surface as a 404 (KanbanError::not_found)"
    );

    assert!(
        rx.try_recv().is_err(),
        "should not broadcast on failed batch"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_commands_malformed_body_returns_4xx() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let malformed_json = json!({});

    let response = send(&state, "POST", "/v1/commands", Some(&malformed_json)).await;

    let status = response.status();
    assert!(
        status.is_client_error(),
        "malformed body should return 4xx, got {}",
        status
    );
}
