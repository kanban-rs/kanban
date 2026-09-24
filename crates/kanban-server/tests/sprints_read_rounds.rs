#![cfg(feature = "test-helpers")]

//! Pins the round-chaining the sprint read routes need: `list_sprints` and
//! `get_sprint` resolve a board-scoped sprint tier alongside the owning
//! board head, and the flat `get_sprint_flat` route chains a second round
//! once it learns the sprint's board id from the first.

use axum::http::StatusCode;
use kanban_server::test_helpers::{json_of, make_sqlite_state, make_state, send};
use kanban_service::KanbanOperations;
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread")]
async fn test_list_sprints_returns_the_boards_sprints() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board A".to_string(), Some("BA".to_string()))
            .unwrap()
            .id;
        ctx.create_sprint(board_id, Some("SPR".to_string()), Some("Alpha".to_string()))
            .unwrap();
        ctx.create_sprint(board_id, Some("SPR".to_string()), Some("Beta".to_string()))
            .unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/sprints", board_id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let arr = json["items"].as_array().expect("items should be an array");
    let names: std::collections::HashSet<_> = arr
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        names,
        ["Alpha".to_string(), "Beta".to_string()]
            .into_iter()
            .collect()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_sprint_returns_the_sprint_and_its_board_id() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let sprint_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board A".to_string(), Some("BA".to_string()))
            .unwrap()
            .id;
        sprint_id = ctx
            .create_sprint(board_id, Some("SPR".to_string()), Some("Alpha".to_string()))
            .unwrap()
            .id;
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/sprints/{}", board_id, sprint_id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["name"].as_str().unwrap(), "Alpha");
    assert_eq!(json["board_id"].as_str().unwrap(), board_id.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_the_flat_sprint_route_chains_a_second_round_for_its_owning_board_head() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let sprint_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board A".to_string(), Some("BA".to_string()))
            .unwrap()
            .id;
        sprint_id = ctx
            .create_sprint(board_id, Some("SPR".to_string()), Some("Alpha".to_string()))
            .unwrap()
            .id;
    }

    let response = send(&state, "GET", &format!("/v1/sprints/{}", sprint_id), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["name"].as_str().unwrap(), "Alpha");
    assert_eq!(json["board_id"].as_str().unwrap(), board_id.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sprint_reads_of_an_archived_board_resolve_the_name_and_load_its_head() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let sprint_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board A".to_string(), Some("BA".to_string()))
            .unwrap()
            .id;
        sprint_id = ctx
            .create_sprint(board_id, Some("SPR".to_string()), Some("Alpha".to_string()))
            .unwrap()
            .id;
        ctx.archive_board(board_id).unwrap();
    }

    let list_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/sprints", board_id),
        None,
    )
    .await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_json = json_of(list_response).await;
    let arr = list_json["items"]
        .as_array()
        .expect("items should be an array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"].as_str().unwrap(), "Alpha");

    let get_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/sprints/{}", board_id, sprint_id),
        None,
    )
    .await;
    assert_eq!(get_response.status(), StatusCode::OK);
    let get_json = json_of(get_response).await;
    assert_eq!(get_json["name"].as_str().unwrap(), "Alpha");

    let flat_response = send(&state, "GET", &format!("/v1/sprints/{}", sprint_id), None).await;
    assert_eq!(flat_response.status(), StatusCode::OK);
    let flat_json = json_of(flat_response).await;
    assert_eq!(flat_json["name"].as_str().unwrap(), "Alpha");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sequential_sprint_reads_on_a_sqlite_locator_each_serve_the_current_row() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("s.sqlite")).await;

    let board_id: Uuid;
    let sprint_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board A".to_string(), Some("BA".to_string()))
            .unwrap()
            .id;
        sprint_id = ctx
            .create_sprint(board_id, Some("SPR".to_string()), Some("Alpha".to_string()))
            .unwrap()
            .id;
    }

    let get_response = send(&state, "GET", &format!("/v1/sprints/{}", sprint_id), None).await;
    assert_eq!(get_response.status(), StatusCode::OK);
    let get_json = json_of(get_response).await;
    assert_eq!(get_json["name"].as_str().unwrap(), "Alpha");

    let list_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/sprints", board_id),
        None,
    )
    .await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_json = json_of(list_response).await;
    let arr = list_json["items"]
        .as_array()
        .expect("items should be an array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"].as_str().unwrap(), "Alpha");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sprint_reads_of_an_archived_board_resolve_on_a_sqlite_locator() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("s.sqlite")).await;

    let board_id: Uuid;
    let sprint_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board A".to_string(), Some("BA".to_string()))
            .unwrap()
            .id;
        sprint_id = ctx
            .create_sprint(board_id, Some("SPR".to_string()), Some("Alpha".to_string()))
            .unwrap()
            .id;
        ctx.archive_board(board_id).unwrap();
    }

    let list_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/sprints", board_id),
        None,
    )
    .await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_json = json_of(list_response).await;
    let arr = list_json["items"]
        .as_array()
        .expect("items should be an array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"].as_str().unwrap(), "Alpha");

    let flat_response = send(&state, "GET", &format!("/v1/sprints/{}", sprint_id), None).await;
    assert_eq!(flat_response.status(), StatusCode::OK);
    let flat_json = json_of(flat_response).await;
    assert_eq!(flat_json["name"].as_str().unwrap(), "Alpha");
}
