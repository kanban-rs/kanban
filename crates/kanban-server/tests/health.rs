//! KAN-689: the axum transport scaffold (`AppState`, `app::router`, `/health`)
//! that every route card attaches to. Proven here via `tower::ServiceExt::oneshot`
//! against the router directly, with no real TCP socket.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use kanban_persistence_json::JsonFileStore;
use kanban_server::app;
use kanban_server::state::AppState;
use kanban_service::json_backend::JsonDataStore;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;
use tower::ServiceExt;

fn make_state(path: &std::path::Path) -> AppState {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    let ctx = KanbanContext::open_deferred(backend, AppConfig::default());
    AppState::new(ctx)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_health_route_returns_ok_with_instance_id() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let response = app::router(state)
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    let instance_id = json["instance_id"].as_str().expect("instance_id present");
    uuid::Uuid::parse_str(instance_id).expect("instance_id is a valid uuid");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_health_instance_id_is_stable_across_requests() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let router = app::router(state);

    let first = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let first_body = axum::body::to_bytes(first.into_body(), usize::MAX)
        .await
        .unwrap();
    let first_json: serde_json::Value = serde_json::from_slice(&first_body).unwrap();

    let second = router
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let second_body = axum::body::to_bytes(second.into_body(), usize::MAX)
        .await
        .unwrap();
    let second_json: serde_json::Value = serde_json::from_slice(&second_body).unwrap();

    assert_eq!(first_json["instance_id"], second_json["instance_id"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_router_unknown_path_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let response = app::router(state)
        .oneshot(Request::builder().uri("/nope").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
