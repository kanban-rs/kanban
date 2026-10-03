#![cfg(feature = "test-helpers")]

//! Card write routes (POST, PUT, PATCH, DELETE /v1/columns/*/cards* and /v1/boards/*/cards*).
//! Each handler acquires the context lock, calls the seam layer, broadcasts
//! a change event on success, then returns the appropriate status.

use axum::http::StatusCode;
use kanban_domain::{CardStatus, ColumnUpdate, CreateCardOptions, KanbanOperations};
use kanban_server::state::AppState;
use kanban_server::test_helpers::{
    json_of, make_sqlite_state, make_state, send, send_with_headers,
};
use serde_json::json;
use tempfile::tempdir;
use uuid::Uuid;

const STALE_IF_MATCH: &str = "\"00000000000000000000000000000000\"";

fn etag_of(response: &axum::response::Response) -> String {
    response
        .headers()
        .get("etag")
        .expect("etag header")
        .to_str()
        .unwrap()
        .to_string()
}

async fn seed_board(state: &AppState) -> Uuid {
    let mut ctx = state.ctx.lock().await;
    ctx.create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id
}

async fn seed_board_and_column(state: &AppState, name: &str) -> (Uuid, Uuid) {
    let mut ctx = state.ctx.lock().await;
    let board_id = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id;
    let col = ctx.create_column(board_id, name.to_string(), None).unwrap();
    (board_id, col.id)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_card_creates_with_append_position_and_returns_201() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (_board_id, column_id) = seed_board_and_column(&state, "To Do").await;

    // POST first card
    let response = send(
        &state,
        "POST",
        &format!("/v1/columns/{column_id}/cards"),
        Some(&json!({"title": "Task 1"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let json = json_of(response).await;
    assert_eq!(json["title"], "Task 1");
    assert_eq!(json["position"], 0);

    // POST second card
    let response = send(
        &state,
        "POST",
        &format!("/v1/columns/{column_id}/cards"),
        Some(&json!({"title": "Task 2"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let json = json_of(response).await;
    assert_eq!(json["title"], "Task 2");
    assert_eq!(json["position"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_card_response_carries_an_invalidation_naming_the_card() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (_board_id, column_id) = seed_board_and_column(&state, "To Do").await;

    let response = send(
        &state,
        "POST",
        &format!("/v1/columns/{column_id}/cards"),
        Some(&json!({"title": "A card"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = json_of(response).await;
    let entity: kanban_service::api::CardResponse = serde_json::from_value(body.clone()).unwrap();
    assert_eq!(entity.title, "A card");
    let card_id = body["id"].as_str().unwrap();
    let invalidated_cards = body["invalidation"]["entities"]["cards"]
        .as_array()
        .expect("entities invalidation must name cards");
    assert!(invalidated_cards.iter().any(|v| v == card_id));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_card_unknown_column_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let unknown_column = Uuid::new_v4();

    let response = send(
        &state,
        "POST",
        &format!("/v1/columns/{unknown_column}/cards"),
        Some(&json!({"title": "Task"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_of(response).await["code"], "NOT_FOUND");
}

// POST honours a client-supplied id for idempotent create (into_new_card),
// so without a guard it hits the same relocation hole as PUT: a body id
// matching a card that already exists under a DIFFERENT column must 404,
// not silently move it.
#[tokio::test(flavor = "multi_thread")]
async fn test_post_card_wrong_column_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, column_a, card_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col_a = ctx
            .create_column(board_id, "Column A".to_string(), None)
            .unwrap();
        let card = ctx
            .create_card(board_id, col_a.id, "Task".to_string(), Default::default())
            .unwrap();
        (board_id, col_a.id, card.id)
    };
    let column_b = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_column(board_id, "Column B".to_string(), None)
            .unwrap()
            .id
    };

    let response = send(
        &state,
        "POST",
        &format!("/v1/columns/{column_b}/cards"),
        Some(&json!({"id": card_id, "title": "Hijacked"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let ctx = state.ctx.lock().await;
    let card = ctx.get_card(card_id).unwrap().unwrap();
    assert_eq!(card.column_id, column_a);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_card_creates_when_absent_returns_201() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (_board_id, column_id) = seed_board_and_column(&state, "To Do").await;
    let card_id = Uuid::new_v4();

    let response = send(
        &state,
        "PUT",
        &format!("/v1/columns/{column_id}/cards/{card_id}"),
        Some(&json!({"title": "New Task"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let json = json_of(response).await;
    assert_eq!(json["id"], card_id.to_string());
    assert_eq!(json["title"], "New Task");
}

// Mirrors test_put_column_wrong_board_returns_404 (columns_write.rs): PUT is
// idempotent create-or-replace keyed on the path id, so without a guard a
// client could PUT an id that already exists under a DIFFERENT column and
// silently relocate it into the path's column instead of getting a 404.
#[tokio::test(flavor = "multi_thread")]
async fn test_put_card_wrong_column_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, column_a, card_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col_a = ctx
            .create_column(board_id, "Column A".to_string(), None)
            .unwrap();
        let card = ctx
            .create_card(board_id, col_a.id, "Task".to_string(), Default::default())
            .unwrap();
        (board_id, col_a.id, card.id)
    };
    let column_b = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_column(board_id, "Column B".to_string(), None)
            .unwrap()
            .id
    };

    let response = send(
        &state,
        "PUT",
        &format!("/v1/columns/{column_b}/cards/{card_id}"),
        Some(&json!({"title": "Hijacked"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // The card must not have been relocated into column_b.
    let ctx = state.ctx.lock().await;
    let card = ctx.get_card(card_id).unwrap().unwrap();
    assert_eq!(card.column_id, column_a);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_card_replaces_when_present_returns_200() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (_board_id, column_id, card_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col = ctx
            .create_column(board_id, "To Do".to_string(), None)
            .unwrap();
        let card = ctx
            .create_card(board_id, col.id, "Original".to_string(), Default::default())
            .unwrap();
        (board_id, col.id, card.id)
    };

    let response = send(
        &state,
        "PUT",
        &format!("/v1/columns/{column_id}/cards/{card_id}"),
        Some(&json!({"title": "Replaced"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["title"], "Replaced");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_card_updates_title_and_priority_returns_200() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, card_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col = ctx
            .create_column(board_id, "To Do".to_string(), None)
            .unwrap();
        let card = ctx
            .create_card(
                board_id,
                col.id,
                "Original Title".to_string(),
                Default::default(),
            )
            .unwrap();
        (board_id, card.id)
    };

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/cards/{card_id}"),
        Some(&json!({"title": "Patched Title", "priority": "high"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["title"], "Patched Title");
    assert_eq!(json["priority"], "high");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_card_move_to_full_column_returns_409() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, _source_col, dest_col, card_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let source = ctx
            .create_column(board_id, "Source".to_string(), None)
            .unwrap();
        let dest = ctx
            .create_column(board_id, "Full".to_string(), None)
            .unwrap();
        // Set the WIP limit on the destination column
        ctx.update_column(
            dest.id,
            kanban_domain::ColumnUpdate {
                wip_limit: kanban_domain::FieldUpdate::Set(1),
                ..Default::default()
            },
        )
        .unwrap();
        // Add a card to the destination column to fill the WIP limit
        ctx.create_card(
            board_id,
            dest.id,
            "Blocking Task".to_string(),
            Default::default(),
        )
        .unwrap();
        // Create a card in the source column to move
        let card = ctx
            .create_card(
                board_id,
                source.id,
                "To Move".to_string(),
                Default::default(),
            )
            .unwrap();
        (board_id, source.id, dest.id, card.id)
    };

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/cards/{card_id}"),
        Some(&json!({"column_id": dest_col.to_string()})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(json_of(response).await["code"], "WIP_LIMIT_EXCEEDED");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_card_wrong_board_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let card_id = {
        let mut ctx = state.ctx.lock().await;
        let board_a = ctx
            .create_board("Board A".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col = ctx
            .create_column(board_a, "To Do".to_string(), None)
            .unwrap();
        ctx.create_card(board_a, col.id, "Task".to_string(), Default::default())
            .unwrap()
            .id
    };

    let board_b = seed_board(&state).await;

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_b}/cards/{card_id}"),
        Some(&json!({"title": "Hijacked"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_card_unknown_id_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = seed_board(&state).await;
    let unknown_card = Uuid::new_v4();

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/cards/{unknown_card}"),
        Some(&json!({"title": "Patched"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_nested_card_delete_keeps_its_204_no_content() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, card_id) = {
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
    };

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/boards/{board_id}/cards/{card_id}"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(body.is_empty());

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/cards/{card_id}"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_card_wrong_board_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let card_id = {
        let mut ctx = state.ctx.lock().await;
        let board_a = ctx
            .create_board("Board A".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col = ctx
            .create_column(board_a, "To Do".to_string(), None)
            .unwrap();
        ctx.create_card(board_a, col.id, "Task".to_string(), Default::default())
            .unwrap()
            .id
    };

    let board_b = seed_board(&state).await;

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/boards/{board_b}/cards/{card_id}"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_successful_write_broadcasts_one_change_event() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (_board_id, column_id) = seed_board_and_column(&state, "To Do").await;

    // Subscribe to change events
    let mut rx = state.event_tx.subscribe();

    // POST a card
    let response = send(
        &state,
        "POST",
        &format!("/v1/columns/{column_id}/cards"),
        Some(&json!({"title": "Task"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    // Assert exactly one frame arrives
    assert!(rx.try_recv().is_ok(), "expected one change event");
    assert!(
        rx.try_recv().is_err(),
        "expected no additional change events"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_failed_write_does_not_broadcast() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = seed_board(&state).await;

    // Subscribe to change events
    let mut rx = state.event_tx.subscribe();

    // PATCH an unknown card (404)
    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/cards/{}", Uuid::new_v4()),
        Some(&json!({"title": "Patched"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // Assert no change event was broadcast
    assert!(
        rx.try_recv().is_err(),
        "expected no change event on failed write"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_card_with_stale_if_match_returns_412_and_leaves_card_unchanged() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, card_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col = ctx
            .create_column(board_id, "To Do".to_string(), None)
            .unwrap();
        let card = ctx
            .create_card(
                board_id,
                col.id,
                "Original Title".to_string(),
                Default::default(),
            )
            .unwrap();
        (board_id, card.id)
    };

    let response = send_with_headers(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/cards/{card_id}"),
        Some(&json!({"title": "Patched Title"})),
        &[("if-match", STALE_IF_MATCH)],
    )
    .await;

    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
    assert_eq!(json_of(response).await["code"], "PRECONDITION_FAILED");

    let ctx = state.ctx.lock().await;
    assert_eq!(
        ctx.get_card(card_id).unwrap().unwrap().title,
        "Original Title"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_card_with_the_get_etag_succeeds_then_the_reused_etag_returns_412() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, card_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col = ctx
            .create_column(board_id, "To Do".to_string(), None)
            .unwrap();
        let card = ctx
            .create_card(
                board_id,
                col.id,
                "Original Title".to_string(),
                Default::default(),
            )
            .unwrap();
        (board_id, card.id)
    };

    let get_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/cards/{card_id}"),
        None,
    )
    .await;
    let tag = etag_of(&get_response);

    let first = send_with_headers(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/cards/{card_id}"),
        Some(&json!({"title": "Renamed Once"})),
        &[("if-match", &tag)],
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);

    let second = send_with_headers(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/cards/{card_id}"),
        Some(&json!({"title": "Renamed Twice"})),
        &[("if-match", &tag)],
    )
    .await;
    assert_eq!(second.status(), StatusCode::PRECONDITION_FAILED);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_card_with_stale_if_match_returns_412_and_keeps_the_card() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, card_id) = {
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
    };

    let response = send_with_headers(
        &state,
        "DELETE",
        &format!("/v1/boards/{board_id}/cards/{card_id}"),
        None,
        &[("if-match", STALE_IF_MATCH)],
    )
    .await;

    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);

    let get_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/cards/{card_id}"),
        None,
    )
    .await;
    assert_eq!(get_response.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_card_create_with_if_match_returns_412_and_creates_nothing() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (_board_id, column_id) = seed_board_and_column(&state, "To Do").await;
    let fresh_id = Uuid::new_v4();

    let response = send_with_headers(
        &state,
        "PUT",
        &format!("/v1/columns/{column_id}/cards/{fresh_id}"),
        Some(&json!({"title": "New Task"})),
        &[("if-match", "*")],
    )
    .await;

    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);

    let get_response = send(&state, "GET", &format!("/v1/cards/{fresh_id}"), None).await;
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_move_card_route_appends_and_chains_status_into_a_completion_column() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, todo_id, done_id, card_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let todo = ctx
            .create_column(board_id, "Todo".to_string(), None)
            .unwrap();
        let done = ctx
            .create_column(board_id, "Done".to_string(), None)
            .unwrap();
        ctx.update_column(
            done.id,
            ColumnUpdate {
                default_status: Some(Some(CardStatus::Done)),
                ..Default::default()
            },
        )
        .unwrap();
        ctx.create_card(
            board_id,
            done.id,
            "Already done".to_string(),
            Default::default(),
        )
        .unwrap();
        let card = ctx
            .create_card(board_id, todo.id, "Task".to_string(), Default::default())
            .unwrap();
        (board_id, todo.id, done.id, card.id)
    };
    let _ = (board_id, todo_id);

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{card_id}/move?column_id={done_id}"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert_eq!(body["position"], 1);
    assert_eq!(body["status"], "done");
    assert_eq!(body["column_id"], done_id.to_string());
    assert_eq!(body["invalidation"]["scope"], "entities");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_move_card_route_unknown_card_is_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (_board_id, column_id) = seed_board_and_column(&state, "To Do").await;
    let unknown_id = Uuid::new_v4();

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{unknown_id}/move?column_id={column_id}"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_assign_sprint_route_pushes_a_sprint_log_entry() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, card_id) = {
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
    };
    let sprint_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_sprint(board_id, None, None).unwrap().id
    };

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{card_id}/assign-sprint?sprint_id={sprint_id}"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert_eq!(body["sprint_id"], sprint_id.to_string());
    assert_eq!(body["invalidation"]["scope"], "entities");

    let ctx = state.ctx.lock().await;
    let card = ctx.get_card(card_id).unwrap().unwrap();
    assert_eq!(card.sprint_logs.len(), 1);
    assert_eq!(card.sprint_logs[0].sprint_id, sprint_id);
    assert!(card.sprint_logs[0].ended_at.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_unassign_sprint_route_closes_the_open_sprint_log() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, card_id) = {
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
    };
    let sprint_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_sprint(board_id, None, None).unwrap().id
    };

    let assign_response = send(
        &state,
        "POST",
        &format!("/v1/cards/{card_id}/assign-sprint?sprint_id={sprint_id}"),
        None,
    )
    .await;
    assert_eq!(assign_response.status(), StatusCode::OK);

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{card_id}/unassign-sprint"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert!(body.get("sprint_id").map_or(true, |v| v.is_null()));
    assert_eq!(body["invalidation"]["scope"], "entities");

    let ctx = state.ctx.lock().await;
    let card = ctx.get_card(card_id).unwrap().unwrap();
    assert_eq!(card.sprint_logs.len(), 1);
    assert!(card.sprint_logs[0].ended_at.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_batch_move_route_still_routes_to_the_batch_handler_after_the_flat_move_route_exists()
{
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, col_a, card_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col_a = ctx
            .create_column(board_id, "To Do".to_string(), None)
            .unwrap()
            .id;
        let card = ctx
            .create_card(board_id, col_a, "Task".to_string(), Default::default())
            .unwrap();
        (board_id, col_a, card.id)
    };
    let col_b = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_column(board_id, "Doing".to_string(), None)
            .unwrap()
            .id
    };
    let _ = col_a;

    let response = send(
        &state,
        "POST",
        "/v1/cards/batch/move",
        Some(&json!({ "ids": [card_id], "column_id": col_b })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert_eq!(body["succeeded"].as_array().unwrap().len(), 1);
    assert_eq!(body["failed"].as_array().unwrap().len(), 0);
}

async fn scenario_move_route_to_another_boards_column_clears_the_sprint(state: AppState) {
    let (card_id, board_b_id, col_b_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_a_id = ctx
            .create_board("A".to_string(), Some("AAA".to_string()))
            .unwrap()
            .id;
        let col_a_id = ctx
            .create_column(board_a_id, "Col".to_string(), None)
            .unwrap()
            .id;
        let sprint_a_id = ctx.create_sprint(board_a_id, None, None).unwrap().id;
        let card_id = ctx
            .create_card(
                board_a_id,
                col_a_id,
                "Task".to_string(),
                CreateCardOptions {
                    sprint_id: Some(sprint_a_id),
                    ..Default::default()
                },
            )
            .unwrap()
            .id;

        let board_b_id = ctx
            .create_board("B".to_string(), Some("BBB".to_string()))
            .unwrap()
            .id;
        let col_b_id = ctx
            .create_column(board_b_id, "Col".to_string(), None)
            .unwrap()
            .id;
        (card_id, board_b_id, col_b_id)
    };

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{card_id}/move?column_id={col_b_id}"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert!(body["sprint_id"].is_null());
    assert_eq!(body["board_id"], board_b_id.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_move_route_to_another_boards_column_returns_the_card_with_its_sprint_cleared_on_json()
{
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    scenario_move_route_to_another_boards_column_clears_the_sprint(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_move_route_to_another_boards_column_returns_the_card_with_its_sprint_cleared_on_sqlite(
) {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("s.sqlite")).await;
    scenario_move_route_to_another_boards_column_clears_the_sprint(state).await;
}

async fn scenario_patch_with_status_and_a_cross_board_column_moves_the_card(state: AppState) {
    let (card_id, board_b_id, col_b_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_a_id = ctx
            .create_board("A".to_string(), Some("AAA".to_string()))
            .unwrap()
            .id;
        let col_a_id = ctx
            .create_column(board_a_id, "Col".to_string(), None)
            .unwrap()
            .id;
        let sprint_a_id = ctx.create_sprint(board_a_id, None, None).unwrap().id;
        let card_id = ctx
            .create_card(
                board_a_id,
                col_a_id,
                "Task".to_string(),
                CreateCardOptions {
                    sprint_id: Some(sprint_a_id),
                    ..Default::default()
                },
            )
            .unwrap()
            .id;

        let board_b_id = ctx
            .create_board("B".to_string(), Some("BBB".to_string()))
            .unwrap()
            .id;
        let col_b_id = ctx
            .create_column(board_b_id, "Col".to_string(), None)
            .unwrap()
            .id;
        ctx.create_card(board_b_id, col_b_id, "B1".to_string(), Default::default())
            .unwrap();
        (card_id, board_b_id, col_b_id)
    };

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/cards/{card_id}"),
        Some(&json!({"status": "in_progress", "column_id": col_b_id.to_string()})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert_eq!(body["board_id"], board_b_id.to_string());
    assert_eq!(body["column_id"], col_b_id.to_string());
    assert_eq!(body["position"], 1);
    assert_eq!(body["status"], "in_progress");
    assert!(body["sprint_id"].is_null());

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_b_id}/cards/{card_id}"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_card_with_status_and_a_cross_board_column_moves_it_to_the_destination_board_on_json(
) {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    scenario_patch_with_status_and_a_cross_board_column_moves_the_card(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_card_with_status_and_a_cross_board_column_moves_it_to_the_destination_board_on_sqlite(
) {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("s.sqlite")).await;
    scenario_patch_with_status_and_a_cross_board_column_moves_the_card(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_card_with_status_and_column_into_a_full_column_returns_409() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (_board_id, dest_col, card_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let source = ctx
            .create_column(board_id, "Source".to_string(), None)
            .unwrap();
        let dest = ctx
            .create_column(board_id, "Full".to_string(), None)
            .unwrap();
        ctx.update_column(
            dest.id,
            kanban_domain::ColumnUpdate {
                wip_limit: kanban_domain::FieldUpdate::Set(1),
                ..Default::default()
            },
        )
        .unwrap();
        ctx.create_card(
            board_id,
            dest.id,
            "Blocking Task".to_string(),
            Default::default(),
        )
        .unwrap();
        let card = ctx
            .create_card(
                board_id,
                source.id,
                "To Move".to_string(),
                Default::default(),
            )
            .unwrap();
        (board_id, dest.id, card.id)
    };

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/cards/{card_id}"),
        Some(&json!({"status": "in_progress", "column_id": dest_col.to_string()})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(json_of(response).await["code"], "WIP_LIMIT_EXCEEDED");
}
