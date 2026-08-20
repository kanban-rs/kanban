#![cfg(feature = "test-helpers")]

//! Card read routes (GET /v1/boards/{board_id}/cards, GET /v1/boards/{board_id}/cards/{id}).
//! Read-only, no mutation, no event broadcast. Established via `tower::ServiceExt::oneshot`
//! against the router directly, with no real TCP socket.

use axum::http::StatusCode;
use kanban_domain::{CardPriority, CardUpdate, CreateCardOptions};
use kanban_server::test_helpers::{json_of, make_state, send};
use kanban_service::api::CardResponse;
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

    let arr = json["items"].as_array().expect("items should be an array");
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

    let arr = json["items"].as_array().expect("items should be an array");
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

    let arr = json["items"].as_array().expect("items should be an array");
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

    let arr = json["items"].as_array().expect("items should be an array");
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
async fn test_list_cards_unknown_board_id_returns_200_empty_page() {
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
    assert_eq!(json["items"], serde_json::json!([]));
    assert_eq!(json["total"], 0);
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

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_route_body_deserializes_as_page_of_card_response() {
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
        ctx.create_card(board_id, col_id, "Card 1".to_string(), Default::default())
            .unwrap();
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
    let parsed: kanban_service::api::Page<CardResponse> =
        serde_json::from_value(json).expect("list body should deserialize as Page<CardResponse>");
    assert_eq!(parsed.items.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_route_exposes_description_and_board_id_keys() {
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
        ctx.create_card(
            board_id,
            col_id,
            "Card 1".to_string(),
            CreateCardOptions {
                description: Some("A description".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
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
    let arr = json["items"].as_array().unwrap();
    let item = &arr[0];

    assert!(
        item.get("description").is_some(),
        "description key should be present"
    );
    assert_eq!(item["description"], "A description");
    assert_eq!(item["board_id"], board_id.to_string());
    assert!(item.get("prefix").is_some(), "prefix key should be present");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_route_serializes_priority_and_status_snake_case() {
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
        let card_id = ctx
            .create_card(
                board_id,
                col_id,
                "Card 1".to_string(),
                CreateCardOptions {
                    priority: Some(CardPriority::High),
                    ..Default::default()
                },
            )
            .unwrap()
            .id;
        ctx.update_card(
            card_id,
            CardUpdate {
                status: Some(kanban_domain::CardStatus::InProgress),
                ..Default::default()
            },
        )
        .unwrap();
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
    let arr = json["items"].as_array().unwrap();
    let item = &arr[0];

    assert_eq!(item["priority"], "high");
    assert_eq!(item["status"], "in_progress");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_route_stamps_archived_at_on_the_card_response() {
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
        ctx.create_card(
            board_id,
            col_id,
            "Live Card".to_string(),
            Default::default(),
        )
        .unwrap();
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
    let parsed: kanban_service::api::Page<CardResponse> =
        serde_json::from_value(json).expect("list body should deserialize as Page<CardResponse>");

    let archived = parsed
        .items
        .iter()
        .find(|c| c.title == "Archived Card")
        .unwrap();
    let live = parsed
        .items
        .iter()
        .find(|c| c.title == "Live Card")
        .unwrap();

    assert!(
        archived.archived_at.is_some(),
        "archived card should have archived_at stamped"
    );
    assert!(
        live.archived_at.is_none(),
        "live card should have no archived_at"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_and_get_card_agree_for_a_live_card() {
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
                "Live Card".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
    }

    let list_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards", board_id),
        None,
    )
    .await;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_json = json_of(list_response).await;
    let list: kanban_service::api::Page<CardResponse> = serde_json::from_value(list_json)
        .expect("list body should deserialize as Page<CardResponse>");

    let get_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards/{}", board_id, card_id),
        None,
    )
    .await;
    assert_eq!(get_response.status(), StatusCode::OK);
    let get_json = json_of(get_response).await;
    let single: CardResponse =
        serde_json::from_value(get_json).expect("get body should deserialize as CardResponse");

    assert_eq!(list.items.len(), 1);
    assert_eq!(list.items[0], single);
}
