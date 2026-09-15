#![cfg(feature = "test-helpers")]

use axum::http::StatusCode;
use kanban_core::ClientId;
use kanban_domain::{EntityIds, Invalidation, KanbanOperations};
use kanban_server::state::AppState;
use kanban_server::test_helpers::{json_of, make_state, send, send_with_headers, TestServer};
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
    let invalidation = Invalidation::Entities(EntityIds::boards([id]));
    state.broadcast_change(
        ClientId::nil(),
        EntityType::Board,
        id,
        ChangeKind::Updated,
        &invalidation,
    );

    let frame = rx.try_recv().unwrap();
    assert_eq!(frame.entity_type, Some(EntityType::Board));
    assert_eq!(frame.entity_id, Some(id));
    assert_eq!(frame.kind, Some(ChangeKind::Updated));
    assert_eq!(frame.writer_instance_id, state.instance_id);
    assert_eq!(frame.issued_by, ClientId::nil());
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
    assert_eq!(response.status(), StatusCode::OK);
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
    assert_eq!(response.status(), StatusCode::OK);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Card));
    assert_eq!(frame.entity_id, Some(card_id));
    assert_eq!(frame.kind, Some(ChangeKind::Deleted));

    let response = send(&state, "DELETE", &format!("/v1/columns/{column_id}"), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Column));
    assert_eq!(frame.entity_id, Some(column_id));
    assert_eq!(frame.kind, Some(ChangeKind::Deleted));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mutation_frame_carries_header_client_id() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let mut rx = state.event_tx.subscribe();
    let client = Uuid::new_v4();

    let response = send_with_headers(
        &state,
        "POST",
        "/v1/boards",
        Some(&json!({"name": "Board", "card_prefix": "KAN"})),
        &[("x-kanban-client-id", &client.to_string())],
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.issued_by, ClientId::from(client));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_client_identity_does_not_leak_between_requests() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_id, column_id) = seed_board_and_column(&state).await;
    let mut rx = state.event_tx.subscribe();
    let client = Uuid::new_v4();

    let response = send_with_headers(
        &state,
        "POST",
        "/v1/boards",
        Some(&json!({"name": "Board A", "card_prefix": "AAA"})),
        &[("x-kanban-client-id", &client.to_string())],
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.issued_by, ClientId::from(client));

    // The second write sends no header, so it acquires lock_for_write with a
    // nil client id; no earlier request's identity survives into it.
    let response = send(
        &state,
        "POST",
        &format!("/v1/columns/{column_id}/cards"),
        Some(&json!({"title": "Task 1"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.issued_by, ClientId::nil());
}

async fn read_one_sse_frame(response: &mut reqwest::Response) -> serde_json::Value {
    let mut buf = Vec::new();
    loop {
        let chunk = response
            .chunk()
            .await
            .unwrap()
            .expect("stream ended before a full SSE frame arrived");
        buf.extend_from_slice(&chunk);
        let text = String::from_utf8_lossy(&buf);
        if let Some(idx) = text.find("\n\n") {
            let frame_text = text[..idx].to_string();
            let data_line = frame_text
                .lines()
                .find(|l| l.starts_with("data:"))
                .expect("frame must have a data: line");
            return serde_json::from_str(data_line.trim_start_matches("data:").trim()).unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_two_clients_frames_carry_distinct_ids() {
    let server = TestServer::start().await;
    let client_a = Uuid::new_v4();
    let client_b = Uuid::new_v4();

    let mut events_response = server
        .client()
        .get(format!("{}/v1/events", server.base_url()))
        .send()
        .await
        .unwrap();

    server
        .client()
        .post(format!("{}/v1/boards", server.base_url()))
        .header("x-kanban-client-id", client_a.to_string())
        .json(&json!({"name": "Board A", "card_prefix": "AAA"}))
        .send()
        .await
        .unwrap();
    let frame_a = read_one_sse_frame(&mut events_response).await;

    server
        .client()
        .post(format!("{}/v1/boards", server.base_url()))
        .header("x-kanban-client-id", client_b.to_string())
        .json(&json!({"name": "Board B", "card_prefix": "BBB"}))
        .send()
        .await
        .unwrap();
    let frame_b = read_one_sse_frame(&mut events_response).await;

    assert_eq!(frame_a["issued_by"], client_a.to_string());
    assert_eq!(frame_b["issued_by"], client_b.to_string());
    assert_ne!(frame_a["issued_by"], frame_b["issued_by"]);

    drop(events_response);
    server.shutdown().await;
}
