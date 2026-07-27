//! Board write routes (POST, PUT, PATCH, DELETE /v1/boards*).
//! Each handler acquires the context lock, calls the seam layer, broadcasts
//! a change event on success, then returns the appropriate status.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use kanban_persistence_json::JsonFileStore;
use kanban_server::app;
use kanban_server::state::AppState;
use kanban_service::json_backend::JsonDataStore;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;
use uuid::Uuid;

fn make_state(path: &std::path::Path) -> AppState {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    let ctx = KanbanContext::open_deferred(backend, AppConfig::default());
    AppState::new(ctx)
}

async fn send(state: &AppState, method: &str, uri: &str, body: Option<&Value>) -> Response {
    let mut builder = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(v) => {
            builder = builder.header("content-type", "application/json");
            Body::from(serde_json::to_string(v).unwrap())
        }
        None => Body::empty(),
    };
    app::router(state.clone())
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap()
}

async fn json_of(response: Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_board_creates_and_returns_201() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let response = send(
        &state,
        "POST",
        "/v1/boards",
        Some(&json!({"name": "My New Board", "card_prefix": "KAN"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let json = json_of(response).await;
    let board_id = Uuid::parse_str(json["id"].as_str().unwrap()).unwrap();

    let get_response = send(&state, "GET", &format!("/v1/boards/{board_id}"), None).await;
    assert_eq!(get_response.status(), StatusCode::OK);
    assert_eq!(json_of(get_response).await["name"], "My New Board");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_board_existing_id_conflicts_409() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = Uuid::new_v4();
    let body = json!({"id": board_id, "name": "First Board", "card_prefix": "KB"});

    let first = send(&state, "POST", "/v1/boards", Some(&body)).await;
    assert_eq!(first.status(), StatusCode::CREATED);

    let second = send(&state, "POST", "/v1/boards", Some(&body)).await;
    assert_eq!(second.status(), StatusCode::CONFLICT);
    assert_eq!(json_of(second).await["code"], "ALREADY_EXISTS");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_board_creates_when_absent_returns_201() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let fresh_id = Uuid::new_v4();

    let response = send(
        &state,
        "PUT",
        &format!("/v1/boards/{fresh_id}"),
        Some(&json!({
            "name": "Brand New Board",
            "task_sort_field": "priority",
            "task_sort_order": "descending",
            "task_list_view": "grouped_by_column"
        })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(json_of(response).await["id"], fresh_id.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_board_replaces_when_present_returns_200() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Original Board".to_string(), Some("OB".to_string()))
            .unwrap()
            .id
    };

    let response = send(
        &state,
        "PUT",
        &format!("/v1/boards/{board_id}"),
        Some(&json!({
            "name": "Replaced Board",
            "task_sort_field": "created_at",
            "task_sort_order": "ascending",
            "task_list_view": "flat"
        })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["id"], board_id.to_string());
    assert_eq!(json["name"], "Replaced Board");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_board_missing_required_field_returns_422() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let board_id = Uuid::new_v4();

    // task_sort_field deliberately omitted.
    let response = send(
        &state,
        "PUT",
        &format!("/v1/boards/{board_id}"),
        Some(&json!({
            "name": "Incomplete Board",
            "task_sort_order": "ascending",
            "task_list_view": "flat"
        })),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = json_of(response).await;
    assert_eq!(
        json["code"], "VALIDATION_FAILED",
        "a bad request body must use the same {{code, message}} envelope as every other error, not axum's default rejection body: {json}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_board_applies_merge_patch_and_returns_200() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Original Name".to_string(), Some("ON".to_string()))
            .unwrap()
            .id
    };

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}"),
        Some(&json!({"name": "Renamed Board"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json_of(response).await["name"], "Renamed Board");

    let get_response = send(&state, "GET", &format!("/v1/boards/{board_id}"), None).await;
    assert_eq!(json_of(get_response).await["name"], "Renamed Board");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_board_unknown_id_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let random_id = Uuid::new_v4();

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{random_id}"),
        Some(&json!({"name": "Attempted Patch"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_board_returns_204_and_removes_board() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board to Delete".to_string(), Some("BD".to_string()))
            .unwrap()
            .id
    };

    let response = send(&state, "DELETE", &format!("/v1/boards/{board_id}"), None).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let get_response = send(&state, "GET", &format!("/v1/boards/{board_id}"), None).await;
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_board_unknown_id_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let random_id = Uuid::new_v4();

    let response = send(&state, "DELETE", &format!("/v1/boards/{random_id}"), None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_board_write_lifecycle() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let create_response = send(
        &state,
        "POST",
        "/v1/boards",
        Some(&json!({"name": "Lifecycle Board", "card_prefix": "LC"})),
    )
    .await;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let board_id = Uuid::parse_str(json_of(create_response).await["id"].as_str().unwrap()).unwrap();

    let patch_response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}"),
        Some(&json!({"name": "Updated Lifecycle Board"})),
    )
    .await;
    assert_eq!(patch_response.status(), StatusCode::OK);

    let delete_response = send(&state, "DELETE", &format!("/v1/boards/{board_id}"), None).await;
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let get_response = send(&state, "GET", &format!("/v1/boards/{board_id}"), None).await;
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_board_removes_owned_columns_and_cards() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, column_id, card_id) = {
        let mut ctx = state.ctx.lock().await;
        let board = ctx
            .create_board("Board With Subtree".to_string(), Some("BS".to_string()))
            .unwrap();
        let column = ctx
            .create_column(board.id, "Todo".to_string(), None)
            .unwrap();
        let card = ctx
            .create_card(
                board.id,
                column.id,
                "A card".to_string(),
                Default::default(),
            )
            .unwrap();
        (board.id, column.id, card.id)
    };

    let response = send(&state, "DELETE", &format!("/v1/boards/{board_id}"), None).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let ctx = state.ctx.lock().await;
    assert!(
        ctx.get_column(column_id).unwrap().is_none(),
        "deleting a board must cascade-delete its columns"
    );
    assert!(
        ctx.get_card(card_id).unwrap().is_none(),
        "deleting a board must cascade-delete its cards"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_board_removes_owned_subtree_on_sqlite_backend() {
    use kanban_service::sqlite_backend::SqliteBackend;

    let dir = tempdir().unwrap();
    let path = dir.path().join("s.sqlite");
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(SqliteBackend::open(path.to_str().unwrap()).await.unwrap());
    let ctx = KanbanContext::open_deferred(backend, AppConfig::default());
    let state = AppState::new(ctx);

    let (board_id, column_id, card_id) = {
        let mut ctx = state.ctx.lock().await;
        let board = ctx
            .create_board("SQLite Board".to_string(), Some("SQ".to_string()))
            .unwrap();
        let column = ctx
            .create_column(board.id, "Todo".to_string(), None)
            .unwrap();
        let card = ctx
            .create_card(
                board.id,
                column.id,
                "A card".to_string(),
                Default::default(),
            )
            .unwrap();
        (board.id, column.id, card.id)
    };

    let response = send(&state, "DELETE", &format!("/v1/boards/{board_id}"), None).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let ctx = state.ctx.lock().await;
    assert!(
        ctx.get_column(column_id).unwrap().is_none(),
        "SQLite: deleting a board must cascade-delete its columns"
    );
    assert!(
        ctx.get_card(card_id).unwrap().is_none(),
        "SQLite: deleting a board must cascade-delete its cards"
    );
}
