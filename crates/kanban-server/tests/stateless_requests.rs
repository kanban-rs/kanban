#![cfg(feature = "test-helpers")]

use kanban_domain::BoardUpdate;
use kanban_server::test_helpers::{json_of, make_sqlite_state, make_state, send};
use kanban_service::KanbanOperations;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread")]
async fn test_a_board_created_between_two_requests_appears_in_the_second_list() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("First".to_string(), None).unwrap();
    }
    let first = json_of(send(&state, "GET", "/v1/boards", None).await).await;
    assert_eq!(first["items"].as_array().unwrap().len(), 1);

    {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Second".to_string(), None).unwrap();
    }
    let second = json_of(send(&state, "GET", "/v1/boards", None).await).await;
    let arr = second["items"].as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let names: std::collections::HashSet<_> =
        arr.iter().map(|b| b["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["First", "Second"].into_iter().collect());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_column_created_between_two_requests_appears_in_the_second_list() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx.create_board("Board".to_string(), None).unwrap().id;
        ctx.create_column(board_id, "First".to_string(), None)
            .unwrap();
    }
    let first = json_of(
        send(
            &state,
            "GET",
            &format!("/v1/boards/{board_id}/columns"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(first["items"].as_array().unwrap().len(), 1);

    {
        let mut ctx = state.ctx.lock().await;
        ctx.create_column(board_id, "Second".to_string(), None)
            .unwrap();
    }
    let second = json_of(
        send(
            &state,
            "GET",
            &format!("/v1/boards/{board_id}/columns"),
            None,
        )
        .await,
    )
    .await;
    let arr = second["items"].as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let names: std::collections::HashSet<_> =
        arr.iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["First", "Second"].into_iter().collect());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_board_renamed_between_two_requests_is_served_fresh_by_id() {
    let dir = tempfile::tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx.create_board("Original".to_string(), None).unwrap().id;
    }
    let first = json_of(send(&state, "GET", &format!("/v1/boards/{board_id}"), None).await).await;
    assert_eq!(first["name"].as_str().unwrap(), "Original");

    {
        let mut ctx = state.ctx.lock().await;
        ctx.update_board(
            board_id,
            BoardUpdate {
                name: Some("Renamed".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
    }
    let second = json_of(send(&state, "GET", &format!("/v1/boards/{board_id}"), None).await).await;
    assert_eq!(second["name"].as_str().unwrap(), "Renamed");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_board_created_between_two_requests_appears_in_the_second_list_on_a_sqlite_locator()
{
    let dir = tempfile::tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("s.sqlite")).await;

    {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("First".to_string(), None).unwrap();
    }
    let first = json_of(send(&state, "GET", "/v1/boards", None).await).await;
    assert_eq!(first["items"].as_array().unwrap().len(), 1);

    {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Second".to_string(), None).unwrap();
    }
    let second = json_of(send(&state, "GET", "/v1/boards", None).await).await;
    let arr = second["items"].as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let names: std::collections::HashSet<_> =
        arr.iter().map(|b| b["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["First", "Second"].into_iter().collect());
}
