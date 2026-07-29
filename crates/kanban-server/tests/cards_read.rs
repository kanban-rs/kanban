#![cfg(feature = "test-helpers")]

//! Card read routes (GET /v1/boards/{board_id}/cards, GET /v1/boards/{board_id}/cards/{id}).
//! Read-only, no mutation, no event broadcast. Established via `tower::ServiceExt::oneshot`
//! against the router directly, with no real TCP socket.

use axum::http::StatusCode;
use kanban_server::test_helpers::{json_of, make_state, send};
use kanban_service::KanbanOperations;
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_returns_all_board_cards_by_default() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Test Board".to_string(), Some("TB".to_string()))
            .unwrap()
            .id;

        let col1_id = ctx
            .create_column(board_id, "Column 1".to_string(), None)
            .unwrap()
            .id;
        let col2_id = ctx
            .create_column(board_id, "Column 2".to_string(), None)
            .unwrap()
            .id;

        let _ = ctx
            .create_card(board_id, col1_id, "Card 1".to_string(), Default::default())
            .unwrap()
            .id;
        let _ = ctx
            .create_card(board_id, col2_id, "Card 2".to_string(), Default::default())
            .unwrap()
            .id;
        let archived_card_id = ctx
            .create_card(
                board_id,
                col1_id,
                "Card 3 (archived)".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        ctx.archive_card(archived_card_id).unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards", board_id),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;

    assert!(json.is_array(), "response should be an array");
    let arr = json.as_array().unwrap();
    assert_eq!(
        arr.len(),
        2,
        "should have 2 live cards (archived one excluded by default)"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_column_filter_returns_only_that_column() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let col1_id: Uuid;
    let col2_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Test Board".to_string(), Some("TB".to_string()))
            .unwrap()
            .id;

        col1_id = ctx
            .create_column(board_id, "Column 1".to_string(), None)
            .unwrap()
            .id;
        col2_id = ctx
            .create_column(board_id, "Column 2".to_string(), None)
            .unwrap()
            .id;

        let _ = ctx
            .create_card(
                board_id,
                col1_id,
                "Card in Col 1".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        let _ = ctx
            .create_card(
                board_id,
                col1_id,
                "Another Card in Col 1".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        let _ = ctx
            .create_card(
                board_id,
                col2_id,
                "Card in Col 2".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards?column_id={}", board_id, col1_id),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;

    assert!(json.is_array(), "response should be an array");
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2, "should have 2 cards in column 1");

    for card in arr {
        assert_eq!(
            card["column_id"],
            col1_id.to_string(),
            "all cards should be in column 1"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_sprint_filter_returns_only_that_sprint() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let sprint_id: Uuid;
    let col_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Test Board".to_string(), Some("TB".to_string()))
            .unwrap()
            .id;

        col_id = ctx
            .create_column(board_id, "Column".to_string(), None)
            .unwrap()
            .id;

        sprint_id = ctx
            .create_sprint(board_id, None, Some("Sprint 1".to_string()))
            .unwrap()
            .id;

        let card1_id = ctx
            .create_card(
                board_id,
                col_id,
                "Card in Sprint".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        let _ = ctx
            .create_card(
                board_id,
                col_id,
                "Card not in Sprint".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;

        ctx.assign_card_to_sprint(card1_id, sprint_id).unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards?sprint_id={}", board_id, sprint_id),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;

    assert!(json.is_array(), "response should be an array");
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1, "should have 1 card in the sprint");
    assert_eq!(arr[0]["sprint_id"], sprint_id.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_archived_include_returns_live_and_archived_with_archived_at_stamped() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Test Board".to_string(), Some("TB".to_string()))
            .unwrap()
            .id;

        let col_id = ctx
            .create_column(board_id, "Column".to_string(), None)
            .unwrap()
            .id;

        let _ = ctx
            .create_card(
                board_id,
                col_id,
                "Live Card".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        let archived_card_id = ctx
            .create_card(
                board_id,
                col_id,
                "Archived Card".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;

        ctx.archive_card(archived_card_id).unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards?archived=include", board_id),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;

    assert!(json.is_array(), "response should be an array");
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2, "should have 2 cards (1 live, 1 archived)");

    let mut archived_found = false;
    for card in arr {
        if card["title"] == "Archived Card" {
            archived_found = true;
            assert!(
                !card["archived_at"].is_null(),
                "archived card should have archived_at stamped"
            );
        } else if card["title"] == "Live Card" {
            assert!(
                card["archived_at"].is_null(),
                "live card should have null archived_at on wire"
            );
        }
    }
    assert!(archived_found, "archived card should be in response");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_unknown_board_id_returns_200_empty_array() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let random_board_id = Uuid::new_v4();

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards", random_board_id),
        None,
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "unknown board_id should return 200, not 404"
    );
    let json = json_of(response).await;
    assert_eq!(json, serde_json::json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_returns_card_response_for_existing_id() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let card_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Test Board".to_string(), Some("TB".to_string()))
            .unwrap()
            .id;

        let col_id = ctx
            .create_column(board_id, "Column".to_string(), None)
            .unwrap()
            .id;

        card_id = ctx
            .create_card(
                board_id,
                col_id,
                "Test Card".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards/{}", board_id, card_id),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;

    assert_eq!(json["id"], card_id.to_string());
    assert_eq!(json["title"], "Test Card");
    assert!(!json["id"].is_null(), "card response should have an id");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_unknown_id_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Test Board".to_string(), Some("TB".to_string()))
            .unwrap()
            .id;
    }

    let random_card_id = Uuid::new_v4();

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards/{}", board_id, random_card_id),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_wrong_board_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_a_id: Uuid;
    let board_b_id: Uuid;
    let card_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_a_id = ctx
            .create_board("Board A".to_string(), Some("BA".to_string()))
            .unwrap()
            .id;
        board_b_id = ctx
            .create_board("Board B".to_string(), Some("BB".to_string()))
            .unwrap()
            .id;

        let col_a_id = ctx
            .create_column(board_a_id, "Column in A".to_string(), None)
            .unwrap()
            .id;

        card_id = ctx
            .create_card(
                board_a_id,
                col_a_id,
                "Card in A".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards/{}", board_b_id, card_id),
        None,
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "card from a different board should 404"
    );
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
}
