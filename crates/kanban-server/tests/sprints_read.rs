#![cfg(feature = "test-helpers")]

//! Sprint read routes (GET /v1/boards/{board_id}/sprints, GET /v1/boards/{board_id}/sprints/{id}).
//! Read-only, no mutation, no event broadcast. Established via `tower::ServiceExt::oneshot`
//! against the router directly, with no real TCP socket.

use axum::http::StatusCode;
use kanban_server::test_helpers::{json_of, make_state, send};
use kanban_service::KanbanOperations;
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread")]
async fn test_list_sprints_returns_only_the_boards_sprints() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_a: Uuid;
    let board_b: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_a = ctx
            .create_board("Board A".to_string(), Some("BA".to_string()))
            .unwrap()
            .id;
        board_b = ctx
            .create_board("Board B".to_string(), Some("BB".to_string()))
            .unwrap()
            .id;
        ctx.create_sprint(board_a, Some("SPR".to_string()), Some("Alpha".to_string()))
            .unwrap();
        ctx.create_sprint(board_a, Some("SPR".to_string()), Some("Beta".to_string()))
            .unwrap();
        ctx.create_sprint(board_b, Some("SPR".to_string()), Some("Gamma".to_string()))
            .unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/sprints", board_a),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let arr = json.as_array().expect("response should be an array");
    assert_eq!(arr.len(), 2, "should have exactly the board's own 2 sprints");

    let names: std::collections::HashSet<_> = arr
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        names,
        std::collections::HashSet::from(["Alpha".to_string(), "Beta".to_string()])
    );
    assert!(!names.contains("Gamma"), "sprint from another board leaked in");

    for sprint in arr {
        assert_eq!(sprint["board_id"], board_a.to_string());
        assert!(!sprint["sprint_number"].is_null());
        assert!(
            sprint.get("name_index").is_none(),
            "name_index must not be on the wire"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_sprints_empty_board_returns_200_empty_array() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Empty Board".to_string(), Some("EB".to_string()))
            .unwrap()
            .id;
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
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_sprints_unknown_board_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let random_board_id = Uuid::new_v4();

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/sprints", random_board_id),
        None,
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "unknown board_id must 404, not collapse into an empty list"
    );
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_sprint_returns_the_sprint_for_its_own_board() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let sprint_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
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
    assert_eq!(json["id"], sprint_id.to_string());
    assert_eq!(json["board_id"], board_id.to_string());
    assert_eq!(json["sprint_number"], 1);
    assert_eq!(json["name"], "Alpha");
    assert!(
        json.get("name_index").is_none(),
        "name_index must not be on the wire"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_sprint_unknown_id_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
    }

    let random_sprint_id = Uuid::new_v4();

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/sprints/{}", board_id, random_sprint_id),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_sprint_from_another_board_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_a: Uuid;
    let board_b: Uuid;
    let sprint_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_a = ctx
            .create_board("Board A".to_string(), Some("BA".to_string()))
            .unwrap()
            .id;
        board_b = ctx
            .create_board("Board B".to_string(), Some("BB".to_string()))
            .unwrap()
            .id;
        sprint_id = ctx
            .create_sprint(board_a, Some("SPR".to_string()), Some("Alpha".to_string()))
            .unwrap()
            .id;
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/sprints/{}", board_b, sprint_id),
        None,
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "sprint from a different board should 404"
    );
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");

    let follow_up = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/sprints/{}", board_a, sprint_id),
        None,
    )
    .await;
    assert_eq!(
        follow_up.status(),
        StatusCode::OK,
        "the guard must reject, not delete, the sprint"
    );
}
