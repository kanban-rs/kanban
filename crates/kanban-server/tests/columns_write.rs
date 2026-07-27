//! Column write routes (POST, PUT, PATCH, DELETE /v1/boards/{id}/columns*).
//! Each handler acquires the context lock, calls the seam layer, broadcasts
//! a change event on success, then returns the appropriate status.

use axum::http::StatusCode;
use kanban_domain::KanbanOperations;
use serde_json::json;
use tempfile::tempdir;
use uuid::Uuid;

mod common;
use common::{json_of, make_state, send};

#[tokio::test(flavor = "multi_thread")]
async fn test_post_column_creates_with_append_position_and_returns_201() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    // Seed a board
    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    // POST first column
    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/columns"),
        Some(&json!({"name": "To Do"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let json = json_of(response).await;
    assert_eq!(json["name"], "To Do");
    assert_eq!(json["position"], 0);

    // POST second column
    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/columns"),
        Some(&json!({"name": "Doing"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let json = json_of(response).await;
    assert_eq!(json["name"], "Doing");
    assert_eq!(json["position"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_column_unknown_board_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let unknown_board = Uuid::new_v4();

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{unknown_board}/columns"),
        Some(&json!({"name": "To Do"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(json_of(response).await["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_column_negative_wip_limit_returns_422() {
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
        &format!("/v1/boards/{board_id}/columns"),
        Some(&json!({"name": "To Do", "wip_limit": -1})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json_of(response).await["code"], "VALIDATION_FAILED");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_column_creates_when_absent_returns_201() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    let col_id = Uuid::new_v4();

    let response = send(
        &state,
        "PUT",
        &format!("/v1/boards/{board_id}/columns/{col_id}"),
        Some(&json!({"name": "New Column", "position": 0})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let json = json_of(response).await;
    assert_eq!(json["id"], col_id.to_string());
    assert_eq!(json["name"], "New Column");
    assert_eq!(json["position"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_column_replace_sets_position_from_request() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, col_id, old_position) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col = ctx
            .create_column(board_id, "Original".to_string(), None)
            .unwrap();
        (board_id, col.id, col.position)
    };

    // PUT with a different position
    let new_position = old_position + 5;
    let response = send(
        &state,
        "PUT",
        &format!("/v1/boards/{board_id}/columns/{col_id}"),
        Some(&json!({"name": "Renamed", "position": new_position})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["name"], "Renamed");
    assert_eq!(
        json["position"], new_position,
        "position should be set from request"
    );

    // Verify the same position can be sent back (no-op replace)
    let response = send(
        &state,
        "PUT",
        &format!("/v1/boards/{board_id}/columns/{col_id}"),
        Some(&json!({"name": "Renamed", "position": new_position})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["position"], new_position);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_column_missing_required_field_returns_422() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    let col_id = Uuid::new_v4();

    // Missing 'name' field
    let response = send(
        &state,
        "PUT",
        &format!("/v1/boards/{board_id}/columns/{col_id}"),
        Some(&json!({"position": 0})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json_of(response).await["code"], "VALIDATION_FAILED");

    // Missing 'position' field
    let response = send(
        &state,
        "PUT",
        &format!("/v1/boards/{board_id}/columns/{col_id}"),
        Some(&json!({"name": "Column"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_column_applies_merge_patch_and_returns_200() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, col_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col = ctx
            .create_column(board_id, "Original".to_string(), None)
            .unwrap();
        (board_id, col.id)
    };

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/columns/{col_id}"),
        Some(&json!({"name": "Patched"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["name"], "Patched");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_column_unknown_id_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    let unknown_col = Uuid::new_v4();

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/columns/{unknown_col}"),
        Some(&json!({"name": "Patched"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_column_negative_wip_limit_returns_422() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, col_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col = ctx
            .create_column(board_id, "Original".to_string(), None)
            .unwrap();
        (board_id, col.id)
    };

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/columns/{col_id}"),
        Some(&json!({"wip_limit": -1})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reorder_column_updates_position_and_returns_200() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, col_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col = ctx
            .create_column(board_id, "Original".to_string(), None)
            .unwrap();
        (board_id, col.id)
    };

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/columns/{col_id}/reorder"),
        Some(&json!({"position": 5})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["position"], 5);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reorder_column_unknown_id_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    let unknown_col = Uuid::new_v4();

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/columns/{unknown_col}/reorder"),
        Some(&json!({"position": 0})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reorder_column_negative_position_returns_422() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, col_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col = ctx
            .create_column(board_id, "Original".to_string(), None)
            .unwrap();
        (board_id, col.id)
    };

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/columns/{col_id}/reorder"),
        Some(&json!({"position": -1})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_column_returns_204_and_removes_column() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, col_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col = ctx
            .create_column(board_id, "To Delete".to_string(), None)
            .unwrap();
        (board_id, col.id)
    };

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/boards/{board_id}/columns/{col_id}"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify it's deleted
    {
        let ctx = state.ctx.lock().await;
        assert!(ctx.get_column(col_id).unwrap().is_none());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_column_unknown_id_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    let unknown_col = Uuid::new_v4();

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/boards/{board_id}/columns/{unknown_col}"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_column_with_cards_returns_422() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, col_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col = ctx
            .create_column(board_id, "Non-Empty".to_string(), None)
            .unwrap();
        // Add a card to the column
        let _ = ctx
            .create_card(board_id, col.id, "Task".to_string(), Default::default())
            .unwrap();
        (board_id, col.id)
    };

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/boards/{board_id}/columns/{col_id}"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json_of(response).await["code"], "VALIDATION_FAILED");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_column_write_lifecycle() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    // POST create at position 0
    let col_id = {
        let response = send(
            &state,
            "POST",
            &format!("/v1/boards/{board_id}/columns"),
            Some(&json!({"name": "To Do"})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
        Uuid::parse_str(json_of(response).await["id"].as_str().unwrap()).unwrap()
    };

    // PUT replace with position change
    {
        let response = send(
            &state,
            "PUT",
            &format!("/v1/boards/{board_id}/columns/{col_id}"),
            Some(&json!({"name": "Doing", "position": 2})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_of(response).await["position"], 2);
    }

    // PATCH merge patch
    {
        let response = send(
            &state,
            "PATCH",
            &format!("/v1/boards/{board_id}/columns/{col_id}"),
            Some(&json!({"name": "In Progress"})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_of(response).await["name"], "In Progress");
    }

    // POST reorder
    {
        let response = send(
            &state,
            "POST",
            &format!("/v1/boards/{board_id}/columns/{col_id}/reorder"),
            Some(&json!({"position": 3})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json_of(response).await["position"], 3);
    }

    // DELETE
    {
        let response = send(
            &state,
            "DELETE",
            &format!("/v1/boards/{board_id}/columns/{col_id}"),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    // Verify deletion
    {
        let response = send(
            &state,
            "GET",
            &format!("/v1/boards/{board_id}/columns/{col_id}"),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
