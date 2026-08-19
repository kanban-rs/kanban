#![cfg(feature = "test-helpers")]

//! Sprint write routes (POST, PUT, PATCH, DELETE /v1/boards/{id}/sprints*).
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

async fn seed_board_and_sprint(state: &AppState, name: &str) -> (Uuid, Uuid) {
    let mut ctx = state.ctx.lock().await;
    let board_id = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id;
    let sprint = ctx
        .create_sprint(board_id, Some("SPR".to_string()), Some(name.to_string()))
        .unwrap();
    (board_id, sprint.id)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_sprint_creates_and_returns_201() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let board_id = seed_board(&state).await;

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints"),
        Some(&json!({"name": "Alpha", "prefix": "SPR"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let json = json_of(response).await;
    assert_eq!(json["name"], "Alpha");
    assert_eq!(json["board_id"], board_id.to_string());
    assert_eq!(json["sprint_number"], 1);
    assert_eq!(json["prefix"], "SPR");

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints"),
        Some(&json!({"name": "Beta", "prefix": "SPR"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let json = json_of(response).await;
    assert_eq!(json["sprint_number"], 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_sprint_unknown_board_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let random_board_id = Uuid::new_v4();
    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{random_board_id}/sprints"),
        Some(&json!({"name": "Alpha"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_post_sprint_with_existing_id_returns_409() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let board_id = seed_board(&state).await;
    let explicit_id = Uuid::new_v4();

    let first = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints"),
        Some(&json!({"id": explicit_id, "name": "Alpha"})),
    )
    .await;
    assert_eq!(first.status(), StatusCode::CREATED);

    let second = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/sprints"),
        Some(&json!({"id": explicit_id, "name": "Alpha Again"})),
    )
    .await;
    assert_eq!(second.status(), StatusCode::CONFLICT);
    let json = json_of(second).await;
    assert_eq!(json["code"], "ALREADY_EXISTS");

    let list = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/sprints"),
        None,
    )
    .await;
    let arr = json_of(list).await;
    assert_eq!(arr.as_array().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_sprint_creates_when_absent_then_replaces_with_200() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let board_id = seed_board(&state).await;
    let fresh_id = Uuid::new_v4();

    let create_response = send(
        &state,
        "PUT",
        &format!("/v1/boards/{board_id}/sprints/{fresh_id}"),
        Some(&json!({"name": "Alpha", "prefix": "SPR"})),
    )
    .await;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created_json = json_of(create_response).await;
    assert_eq!(created_json["id"], fresh_id.to_string());
    let sprint_number = created_json["sprint_number"].clone();

    let replace_response = send(
        &state,
        "PUT",
        &format!("/v1/boards/{board_id}/sprints/{fresh_id}"),
        Some(&json!({"name": "Alpha", "prefix": "SPR"})),
    )
    .await;
    assert_eq!(replace_response.status(), StatusCode::OK);
    let replaced_json = json_of(replace_response).await;
    assert_eq!(replaced_json["id"], fresh_id.to_string());
    assert_eq!(replaced_json["sprint_number"], sprint_number);

    let list = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/sprints"),
        None,
    )
    .await;
    let arr = json_of(list).await;
    assert_eq!(arr.as_array().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_sprint_from_another_board_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_a, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;
    let board_b = seed_board(&state).await;

    let response = send(
        &state,
        "PUT",
        &format!("/v1/boards/{board_b}/sprints/{sprint_id}"),
        Some(&json!({"name": "Hijacked"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");

    let check = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_a}/sprints/{sprint_id}"),
        None,
    )
    .await;
    let check_json = json_of(check).await;
    assert_eq!(check_json["name"], "Alpha", "original name must be unchanged");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_sprint_updates_name_and_returns_200() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}"),
        Some(&json!({"name": "Renamed"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["name"], "Renamed");
    assert_eq!(json["sprint_number"], 1);

    let follow_up = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}"),
        None,
    )
    .await;
    let follow_up_json = json_of(follow_up).await;
    assert_eq!(follow_up_json["name"], "Renamed");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_sprint_from_another_board_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_a, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;
    let board_b = seed_board(&state).await;

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_b}/sprints/{sprint_id}"),
        Some(&json!({"name": "Hijacked"})),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");

    let check = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_a}/sprints/{sprint_id}"),
        None,
    )
    .await;
    let check_json = json_of(check).await;
    assert_eq!(check_json["name"], "Alpha");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_sprint_returns_204_and_removes_it() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_id, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let follow_up = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}"),
        None,
    )
    .await;
    assert_eq!(follow_up.status(), StatusCode::NOT_FOUND);

    let list = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/sprints"),
        None,
    )
    .await;
    assert_eq!(list.status(), StatusCode::OK);
    let arr = json_of(list).await;
    assert_eq!(arr, serde_json::json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_sprint_from_another_board_returns_404_and_keeps_it() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_a, sprint_id) = seed_board_and_sprint(&state, "Alpha").await;
    let board_b = seed_board(&state).await;

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/boards/{board_b}/sprints/{sprint_id}"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");

    let check = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_a}/sprints/{sprint_id}"),
        None,
    )
    .await;
    assert_eq!(check.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_sprint_persists_to_disk() {
    use kanban_persistence_json::{JsonDataStore, JsonFileStore};
    use kanban_service::{resolve_sprint_name, AppConfig, KanbanBackend, KanbanContext};
    use std::sync::Arc;

    let dir = tempdir().unwrap();
    let path = dir.path().join("s.json");

    let state = make_state(&path);
    let (board_id, sprint_id) = seed_board_and_sprint(&state, "Original Name").await;

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{board_id}/sprints/{sprint_id}"),
        Some(&json!({"name": "Renamed"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["name"], "Renamed");

    let independent_backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(&path))));
    let independent_ctx = KanbanContext::open(independent_backend, AppConfig::default())
        .await
        .unwrap();

    let sprint_from_disk = independent_ctx
        .get_sprint(sprint_id)
        .unwrap()
        .expect("sprint should exist on disk");
    let name = resolve_sprint_name(&independent_ctx, &sprint_from_disk).unwrap();
    assert_eq!(
        name,
        Some("Renamed".to_string()),
        "PATCH update must be persisted to disk, not just in-memory state"
    );
}
