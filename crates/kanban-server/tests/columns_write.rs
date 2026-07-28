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
use kanban_server::state::AppState;

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

#[tokio::test(flavor = "multi_thread")]
async fn test_put_column_negative_position_returns_422() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let board_id = seed_board(&state).await;
    let col_id = Uuid::new_v4();

    let response = send(
        &state,
        "PUT",
        &format!("/v1/boards/{board_id}/columns/{col_id}"),
        Some(&json!({"name": "X", "position": -1})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json_of(response).await["code"], "VALIDATION_FAILED");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_column_negative_wip_limit_returns_422() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let board_id = seed_board(&state).await;
    let col_id = Uuid::new_v4();

    let response = send(
        &state,
        "PUT",
        &format!("/v1/boards/{board_id}/columns/{col_id}"),
        Some(&json!({"name": "X", "position": 0, "wip_limit": -1})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json_of(response).await["code"], "VALIDATION_FAILED");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_column_create_arm_ignores_requested_position() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    // Seed one existing column so the natural append position (1) differs
    // from whatever position a client-created id might request.
    let (board_id, _existing) = seed_board_and_column(&state, "Existing").await;
    let new_col_id = Uuid::new_v4();

    let response = send(
        &state,
        "PUT",
        &format!("/v1/boards/{board_id}/columns/{new_col_id}"),
        Some(&json!({"name": "New", "position": 99})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let json = json_of(response).await;
    assert_eq!(
        json["position"], 1,
        "create arm appends at the next server-managed position, ignoring the client's requested position"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_column_replace_wrong_board_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_a, col_id) = seed_board_and_column(&state, "In A").await;
    let board_b = seed_board(&state).await;

    let response = send(
        &state,
        "PUT",
        &format!("/v1/boards/{board_b}/columns/{col_id}"),
        Some(&json!({"name": "Hijacked", "position": 0})),
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "PUT must not replace a column that belongs to a different board"
    );

    // Confirm the column in board A was left untouched.
    let ctx_check = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_a}/columns/{col_id}"),
        None,
    )
    .await;
    let json = json_of(ctx_check).await;
    assert_eq!(json["name"], "In A", "column content must be unchanged");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_column_wrong_board_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (_board_a, col_id) = seed_board_and_column(&state, "In A").await;
    let board_b = seed_board(&state).await;

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_b}/columns/{col_id}"),
        Some(&json!({"name": "Hijacked"})),
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "PATCH must not modify a column that belongs to a different board"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_column_wrong_board_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_a, col_id) = seed_board_and_column(&state, "In A").await;
    let board_b = seed_board(&state).await;

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/boards/{board_b}/columns/{col_id}"),
        None,
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "DELETE must not remove a column that belongs to a different board"
    );

    // Confirm the column still exists under its real board.
    let check = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_a}/columns/{col_id}"),
        None,
    )
    .await;
    assert_eq!(check.status(), StatusCode::OK, "column must survive");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reorder_column_wrong_board_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_a, col_id) = seed_board_and_column(&state, "In A").await;
    let board_b = seed_board(&state).await;

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_b}/columns/{col_id}/reorder"),
        Some(&json!({"position": 5})),
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "reorder must not move a column that belongs to a different board"
    );

    // Confirm the column's position under its real board is unchanged.
    let check = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_a}/columns/{col_id}"),
        None,
    )
    .await;
    let json = json_of(check).await;
    assert_eq!(json["position"], 0, "position must be unchanged");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_column_persists_to_disk() {
    use kanban_persistence_json::JsonFileStore;
    use kanban_service::json_backend::JsonDataStore;
    use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
    use std::sync::Arc;

    let dir = tempdir().unwrap();
    let path = dir.path().join("s.json");

    // Create a board and column via the HTTP API
    let state = make_state(&path);
    let (board_id, col_id) = seed_board_and_column(&state, "Original Name").await;

    // PATCH the column through the router
    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/columns/{col_id}"),
        Some(&json!({"name": "Updated Name"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["name"], "Updated Name");

    // Open a SECOND, independent context against the same file to verify the write reached disk
    let independent_backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(&path))));
    let independent_ctx = KanbanContext::open(independent_backend, AppConfig::default())
        .await
        .unwrap();

    // Assert that the independent context sees the updated column name on disk
    let col_from_disk = independent_ctx
        .get_column(col_id)
        .unwrap()
        .expect("column should exist on disk");
    assert_eq!(
        col_from_disk.name, "Updated Name",
        "PATCH update must be persisted to disk, not just in-memory state"
    );
}
