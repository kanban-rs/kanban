#![cfg(feature = "test-helpers")]

//! Card write routes (POST, PUT, PATCH, DELETE /v1/columns/*/cards* and /v1/boards/*/cards*).
//! Each handler acquires the context lock, calls the seam layer, broadcasts
//! a change event on success, then returns the appropriate status.

use axum::http::StatusCode;
use kanban_domain::KanbanOperations;
use kanban_server::state::AppState;
use kanban_server::test_helpers::{json_of, make_state, send};
use serde_json::json;
use tempfile::tempdir;
use uuid::Uuid;

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
async fn test_delete_card_returns_204_then_get_404() {
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

    // Verify it's deleted via GET
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
