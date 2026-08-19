#![cfg(feature = "test-helpers")]

use axum::http::StatusCode;
use kanban_domain::KanbanOperations;
use kanban_server::state::AppState;
use kanban_server::test_helpers::{json_of, make_state, send};
use kanban_service::api::{ChangeEventFrame, ChangeKind, EntityType};
use serde_json::json;
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

#[tokio::test(flavor = "multi_thread")]
async fn test_broadcast_change_includes_entity_identity() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let mut rx = state.event_tx.subscribe();

    let id = Uuid::new_v4();
    state.broadcast_change(EntityType::Board, id, ChangeKind::Updated);

    let frame = rx.try_recv().unwrap();
    assert_eq!(frame.entity_type, Some(EntityType::Board));
    assert_eq!(frame.entity_id, Some(id));
    assert_eq!(frame.kind, Some(ChangeKind::Updated));
    assert_eq!(frame.writer_instance_id, state.instance_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_unscoped_broadcast_leaves_entity_fields_none() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let mut rx = state.event_tx.subscribe();

    state.broadcast_unscoped_change();

    let frame = rx.try_recv().unwrap();
    assert!(frame.entity_type.is_none());
    assert!(frame.entity_id.is_none());
    assert!(frame.kind.is_none());
    assert_eq!(frame.writer_instance_id, state.instance_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_card_broadcasts_created_kind() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, column_id) = seed_board_and_column(&state).await;
    let mut rx = state.event_tx.subscribe();

    let response = send(
        &state,
        "POST",
        &format!("/v1/columns/{column_id}/cards"),
        Some(&json!({"title": "Task 1"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_of(response).await;
    let card_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Card));
    assert_eq!(frame.entity_id, Some(card_id));
    assert_eq!(frame.kind, Some(ChangeKind::Created));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_card_broadcasts_deleted_kind() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, column_id) = seed_board_and_column(&state).await;

    let response = send(
        &state,
        "POST",
        &format!("/v1/columns/{column_id}/cards"),
        Some(&json!({"title": "Task 1"})),
    )
    .await;
    let body = json_of(response).await;
    let card_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    let mut rx = state.event_tx.subscribe();
    let response = send(
        &state,
        "DELETE",
        &format!("/v1/boards/{board_id}/cards/{card_id}"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Card));
    assert_eq!(frame.entity_id, Some(card_id));
    assert_eq!(frame.kind, Some(ChangeKind::Deleted));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_card_create_or_replace_reports_created_or_updated_correctly() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, column_id) = seed_board_and_column(&state).await;
    let id = Uuid::new_v4();
    let mut rx = state.event_tx.subscribe();

    let response = send(
        &state,
        "PUT",
        &format!("/v1/columns/{column_id}/cards/{id}"),
        Some(&json!({"title": "Task 1"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Card));
    assert_eq!(frame.entity_id, Some(id));
    assert_eq!(frame.kind, Some(ChangeKind::Created));

    let response = send(
        &state,
        "PUT",
        &format!("/v1/columns/{column_id}/cards/{id}"),
        Some(&json!({"title": "Task 1 renamed"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Card));
    assert_eq!(frame.entity_id, Some(id));
    assert_eq!(frame.kind, Some(ChangeKind::Updated));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_card_with_existing_body_id_broadcasts_updated_kind() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, column_id) = seed_board_and_column(&state).await;
    let id = Uuid::new_v4();
    let mut rx = state.event_tx.subscribe();

    let response = send(
        &state,
        "POST",
        &format!("/v1/columns/{column_id}/cards"),
        Some(&json!({"id": id, "title": "Task 1"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_id, Some(id));
    assert_eq!(frame.kind, Some(ChangeKind::Created));

    let response = send(
        &state,
        "POST",
        &format!("/v1/columns/{column_id}/cards"),
        Some(&json!({"id": id, "title": "Task 1 again"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_id, Some(id));
    assert_eq!(frame.kind, Some(ChangeKind::Updated));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_write_routes_broadcast_their_own_entity_type() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let mut rx = state.event_tx.subscribe();

    let response = send(
        &state,
        "POST",
        "/v1/boards",
        Some(&json!({"name": "Board", "card_prefix": "KAN"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_of(response).await;
    let board_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Board));
    assert_eq!(frame.entity_id, Some(board_id));
    assert_eq!(frame.kind, Some(ChangeKind::Created));

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/columns"),
        Some(&json!({"name": "To Do"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_of(response).await;
    let column_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Column));
    assert_eq!(frame.entity_id, Some(column_id));
    assert_eq!(frame.kind, Some(ChangeKind::Created));

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints"),
        Some(&json!({"name": "Sprint 1"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_of(response).await;
    let sprint_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Sprint));
    assert_eq!(frame.entity_id, Some(sprint_id));
    assert_eq!(frame.kind, Some(ChangeKind::Created));

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}"),
        Some(&json!({"name": "Sprint 1 renamed"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Sprint));
    assert_eq!(frame.entity_id, Some(sprint_id));
    assert_eq!(frame.kind, Some(ChangeKind::Updated));

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/columns/{column_id}/reorder"),
        Some(&json!({"position": 0})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Column));
    assert_eq!(frame.entity_id, Some(column_id));
    assert_eq!(frame.kind, Some(ChangeKind::Updated));

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/boards/{board_id}/columns/{column_id}"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Column));
    assert_eq!(frame.entity_id, Some(column_id));
    assert_eq!(frame.kind, Some(ChangeKind::Deleted));

    let response = send(&state, "DELETE", &format!("/v1/boards/{board_id}"), None).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Board));
    assert_eq!(frame.entity_id, Some(board_id));
    assert_eq!(frame.kind, Some(ChangeKind::Deleted));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_flat_routes_broadcast_their_own_entity_identity() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, column_id) = seed_board_and_column(&state).await;

    let response = send(
        &state,
        "POST",
        &format!("/v1/columns/{column_id}/cards"),
        Some(&json!({"title": "Task 1"})),
    )
    .await;
    let body = json_of(response).await;
    let card_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board2".to_string(), Some("KAN2".to_string()))
            .unwrap()
            .id
    };
    let sprint_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_sprint(board_id, None, None).unwrap().id
    };

    let mut rx = state.event_tx.subscribe();

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/cards/{card_id}"),
        Some(&json!({"title": "Renamed"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Card));
    assert_eq!(frame.entity_id, Some(card_id));
    assert_eq!(frame.kind, Some(ChangeKind::Updated));

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/columns/{column_id}"),
        Some(&json!({"name": "Renamed"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Column));
    assert_eq!(frame.entity_id, Some(column_id));
    assert_eq!(frame.kind, Some(ChangeKind::Updated));

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/sprints/{sprint_id}"),
        Some(&json!({"name": "Renamed"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Sprint));
    assert_eq!(frame.entity_id, Some(sprint_id));
    assert_eq!(frame.kind, Some(ChangeKind::Updated));

    let response = send(&state, "DELETE", &format!("/v1/sprints/{sprint_id}"), None).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Sprint));
    assert_eq!(frame.entity_id, Some(sprint_id));
    assert_eq!(frame.kind, Some(ChangeKind::Deleted));

    let response = send(&state, "DELETE", &format!("/v1/cards/{card_id}"), None).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Card));
    assert_eq!(frame.entity_id, Some(card_id));
    assert_eq!(frame.kind, Some(ChangeKind::Deleted));

    let response = send(&state, "DELETE", &format!("/v1/columns/{column_id}"), None).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Column));
    assert_eq!(frame.entity_id, Some(column_id));
    assert_eq!(frame.kind, Some(ChangeKind::Deleted));
}
