#![cfg(feature = "test-helpers")]

use axum::http::StatusCode;
use kanban_core::ClientId;
use kanban_domain::{CardStatus, CardUpdate, CreateCardOptions, KanbanOperations};
use kanban_server::state::AppState;
use kanban_server::test_helpers::{
    json_of, make_sqlite_state, make_state, send, send_with_headers,
};
use kanban_service::api::ChangeEventFrame;
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

async fn card_write_routes(
    state: &AppState,
    client: Uuid,
    mut rx: broadcast::Receiver<ChangeEventFrame>,
) {
    let headers = [("x-kanban-client-id", client.to_string())];
    let headers: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();

    let (board_id, column_id) = seed_board_and_column(state).await;

    let response = send_with_headers(
        state,
        "POST",
        &format!("/v1/columns/{column_id}/cards"),
        Some(&json!({"title": "Task 1"})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_of(response).await;
    let card_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let new_id = Uuid::new_v4();
    let response = send_with_headers(
        state,
        "PUT",
        &format!("/v1/columns/{column_id}/cards/{new_id}"),
        Some(&json!({"title": "Task 2"})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        state,
        "PATCH",
        &format!("/v1/boards/{board_id}/cards/{card_id}"),
        Some(&json!({"title": "Renamed"})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        state,
        "PATCH",
        &format!("/v1/cards/{card_id}"),
        Some(&json!({"title": "Renamed again"})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        state,
        "POST",
        &format!("/v1/cards/{card_id}/archive"),
        None,
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        state,
        "POST",
        &format!("/v1/cards/{card_id}/restore?column_id={column_id}"),
        None,
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        state,
        "DELETE",
        &format!("/v1/cards/{card_id}"),
        None,
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        state,
        "DELETE",
        &format!("/v1/boards/{board_id}/cards/{new_id}"),
        None,
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_card_write_routes_stamp_the_header_client_id() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let rx = state.event_tx.subscribe();
    let client = Uuid::new_v4();
    card_write_routes(&state, client, rx).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_card_write_routes_stamp_the_header_client_id_sqlite_backend() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    let rx = state.event_tx.subscribe();
    let client = Uuid::new_v4();
    card_write_routes(&state, client, rx).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_card_batch_routes_stamp_the_header_client_id() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let client = Uuid::new_v4();
    let headers = [("x-kanban-client-id", client.to_string())];
    let headers: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();

    let (board_id, column_id) = seed_board_and_column(&state).await;
    let (sprint_id, card_a, card_b) = {
        let mut ctx = state.ctx.lock().await;
        let sprint_id = ctx.create_sprint(board_id, None, None).unwrap().id;
        let card_a = ctx
            .create_card(board_id, column_id, "A".to_string(), Default::default())
            .unwrap()
            .id;
        let card_b = ctx
            .create_card(board_id, column_id, "B".to_string(), Default::default())
            .unwrap()
            .id;
        (sprint_id, card_a, card_b)
    };

    let mut rx = state.event_tx.subscribe();

    let response = send_with_headers(
        &state,
        "POST",
        "/v1/cards/batch/update",
        Some(&json!({"updates": [{"id": card_a, "title": "A2"}]})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "POST",
        "/v1/cards/batch/move",
        Some(&json!({"ids": [card_a, card_b], "column_id": column_id})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "POST",
        "/v1/cards/batch/assign-sprint",
        Some(&json!({"ids": [card_a, card_b], "sprint_id": sprint_id})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "POST",
        "/v1/cards/batch/archive",
        Some(&json!({"ids": [card_a, card_b]})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_graph_write_routes_stamp_the_header_client_id() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let client = Uuid::new_v4();
    let headers = [("x-kanban-client-id", client.to_string())];
    let headers: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();

    let (board_id, column_id) = seed_board_and_column(&state).await;
    let (a, b, c) = {
        let mut ctx = state.ctx.lock().await;
        let a = ctx
            .create_card(board_id, column_id, "A".to_string(), Default::default())
            .unwrap()
            .id;
        let b = ctx
            .create_card(board_id, column_id, "B".to_string(), Default::default())
            .unwrap()
            .id;
        let c = ctx
            .create_card(board_id, column_id, "C".to_string(), Default::default())
            .unwrap()
            .id;
        (a, b, c)
    };

    let mut rx = state.event_tx.subscribe();

    let response = send_with_headers(
        &state,
        "POST",
        &format!("/v1/cards/{a}/children"),
        Some(&json!({"children": [b]})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "DELETE",
        &format!("/v1/cards/{a}/children/{b}"),
        None,
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "POST",
        &format!("/v1/cards/{a}/blocks"),
        Some(&json!({"blocked": c, "severity": "critical"})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "DELETE",
        &format!("/v1/cards/{a}/blocks/{c}"),
        None,
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "POST",
        &format!("/v1/cards/{a}/related"),
        Some(&json!({"other": b, "kind": "duplicates"})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "DELETE",
        &format!("/v1/cards/{a}/related/{b}"),
        None,
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_column_write_routes_stamp_the_header_client_id() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let client = Uuid::new_v4();
    let headers = [("x-kanban-client-id", client.to_string())];
    let headers: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    let mut rx = state.event_tx.subscribe();

    let response = send_with_headers(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/columns"),
        Some(&json!({"name": "To Do"})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_of(response).await;
    let column_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let new_id = Uuid::new_v4();
    let response = send_with_headers(
        &state,
        "PUT",
        &format!("/v1/boards/{board_id}/columns/{new_id}"),
        Some(&json!({"name": "Doing", "position": 1})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/columns/{column_id}"),
        Some(&json!({"name": "Renamed"})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/columns/{column_id}/reorder"),
        Some(&json!({"position": 0})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "PATCH",
        &format!("/v1/columns/{column_id}"),
        Some(&json!({"name": "Renamed flat"})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "DELETE",
        &format!("/v1/columns/{column_id}"),
        None,
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "DELETE",
        &format!("/v1/boards/{board_id}/columns/{new_id}"),
        None,
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sprint_write_routes_stamp_the_header_client_id() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let client = Uuid::new_v4();
    let headers = [("x-kanban-client-id", client.to_string())];
    let headers: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    let mut rx = state.event_tx.subscribe();

    let response = send_with_headers(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints"),
        Some(&json!({"name": "Sprint 1"})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_of(response).await;
    let sprint_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let new_id = Uuid::new_v4();
    let response = send_with_headers(
        &state,
        "PUT",
        &format!("/v1/boards/{board_id}/sprints/{new_id}"),
        Some(&json!({"name": "Sprint 2"})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}"),
        Some(&json!({"name": "Renamed"})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "PATCH",
        &format!("/v1/sprints/{sprint_id}"),
        Some(&json!({"name": "Renamed flat"})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "DELETE",
        &format!("/v1/sprints/{sprint_id}"),
        None,
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "DELETE",
        &format!("/v1/boards/{board_id}/sprints/{new_id}"),
        None,
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sprint_lifecycle_routes_stamp_the_header_client_id() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let client = Uuid::new_v4();
    let headers = [("x-kanban-client-id", client.to_string())];
    let headers: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();

    let (board_id, column_id, sprint1, sprint2) = {
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
        (board_id, column_id, sprint1, sprint2)
    };
    let _ = column_id;

    let mut rx = state.event_tx.subscribe();

    let response = send_with_headers(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint1}/activate"),
        Some(&json!({"duration_days": 7})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint1}/complete"),
        None,
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint1}/carry-over"),
        Some(&json!({"to_sprint_id": sprint2})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint2}/activate"),
        Some(&json!({"duration_days": 7})),
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));

    let response = send_with_headers(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints/{sprint2}/cancel"),
        None,
        &headers,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::from(client));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_writes_without_the_header_emit_nil_issued_by() {
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
    assert_eq!(next_frame(&mut rx).await.issued_by, ClientId::nil());
}
