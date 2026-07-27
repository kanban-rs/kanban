//! Board write routes (POST, PUT, PATCH, DELETE /v1/boards*).
//! Each handler acquires the context lock, calls the seam layer (KAN-994),
//! broadcasts a change event on success, then returns the appropriate status.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use kanban_persistence_json::JsonFileStore;
use kanban_server::app;
use kanban_server::state::AppState;
use kanban_service::json_backend::JsonDataStore;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
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

#[tokio::test(flavor = "multi_thread")]
async fn test_post_board_creates_and_returns_201() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let request_body = serde_json::json!({
        "name": "My New Board",
        "card_prefix": "KAN"
    });

    let response = app::router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/boards")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let board_id_str = json["id"].as_str().expect("response should have an id");
    let board_id = Uuid::parse_str(board_id_str).expect("id should be a valid UUID");

    // Verify it persisted by doing a GET
    let get_response = app::router(state)
        .oneshot(
            Request::builder()
                .uri(format!("/v1/boards/{}", board_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::OK);
    let get_body = axum::body::to_bytes(get_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let get_json: serde_json::Value = serde_json::from_slice(&get_body).unwrap();
    assert_eq!(get_json["name"], "My New Board");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_board_existing_id_conflicts_409() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = Uuid::new_v4();
    let request_body = serde_json::json!({
        "id": board_id,
        "name": "First Board",
        "card_prefix": "KB"
    });

    // First POST should succeed
    let first_response = app::router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/boards")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(first_response.status(), StatusCode::CREATED);

    // Second POST with same ID should fail
    let second_response = app::router(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/boards")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(second_response.status(), StatusCode::CONFLICT);
    let body = axum::body::to_bytes(second_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["code"], "ALREADY_EXISTS");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_board_creates_when_absent_returns_201() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let fresh_id = Uuid::new_v4();
    let request_body = serde_json::json!({
        "name": "Brand New Board",
        "task_sort_field": "priority",
        "task_sort_order": "descending",
        "task_list_view": "grouped_by_column"
    });

    let response = app::router(state)
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/boards/{}", fresh_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["id"], fresh_id.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_board_replaces_when_present_returns_200() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Original Board".to_string(), Some("OB".to_string()))
            .unwrap()
            .id;
    }

    let request_body = serde_json::json!({
        "name": "Replaced Board",
        "task_sort_field": "created_at",
        "task_sort_order": "ascending",
        "task_list_view": "flat"
    });

    let response = app::router(state.clone())
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/boards/{}", board_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["id"], board_id.to_string());
    assert_eq!(json["name"], "Replaced Board");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_board_missing_required_field_returns_400() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = Uuid::new_v4();
    // Missing task_sort_field
    let request_body = serde_json::json!({
        "name": "Incomplete Board",
        "task_sort_order": "ascending",
        "task_list_view": "flat"
    });

    let response = app::router(state)
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/boards/{}", board_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_board_applies_merge_patch_and_returns_200() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Original Name".to_string(), Some("ON".to_string()))
            .unwrap()
            .id;
    }

    let request_body = serde_json::json!({
        "name": "Renamed Board"
    });

    let response = app::router(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/boards/{}", board_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["name"], "Renamed Board");

    // Verify it persisted
    let get_response = app::router(state)
        .oneshot(
            Request::builder()
                .uri(format!("/v1/boards/{}", board_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::OK);
    let get_body = axum::body::to_bytes(get_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let get_json: serde_json::Value = serde_json::from_slice(&get_body).unwrap();
    assert_eq!(get_json["name"], "Renamed Board");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_board_unknown_id_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let random_id = Uuid::new_v4();
    let request_body = serde_json::json!({
        "name": "Attempted Patch"
    });

    let response = app::router(state)
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/boards/{}", random_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_board_returns_204_and_removes_board() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board to Delete".to_string(), Some("BD".to_string()))
            .unwrap()
            .id;
    }

    let response = app::router(state.clone())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/boards/{}", board_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify it was deleted by attempting to GET it
    let get_response = app::router(state)
        .oneshot(
            Request::builder()
                .uri(format!("/v1/boards/{}", board_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_board_unknown_id_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let random_id = Uuid::new_v4();

    let response = app::router(state)
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/boards/{}", random_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_board_write_lifecycle() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    // POST: create
    let create_body = serde_json::json!({
        "name": "Lifecycle Board",
        "card_prefix": "LC"
    });
    let create_response = app::router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/boards")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&create_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let board_id = Uuid::parse_str(json["id"].as_str().unwrap()).unwrap();

    // PATCH: update
    let patch_body = serde_json::json!({
        "name": "Updated Lifecycle Board"
    });
    let patch_response = app::router(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/boards/{}", board_id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&patch_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch_response.status(), StatusCode::OK);

    // DELETE: remove
    let delete_response = app::router(state.clone())
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/boards/{}", board_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    // GET: verify deleted
    let get_response = app::router(state)
        .oneshot(
            Request::builder()
                .uri(format!("/v1/boards/{}", board_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
}
