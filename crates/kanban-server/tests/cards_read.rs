#![cfg(feature = "test-helpers")]

//! Card read routes (GET /v1/boards/{board_id}/cards, GET /v1/boards/{board_id}/cards/{id}).
//! Read-only, no mutation, no event broadcast. Established via `tower::ServiceExt::oneshot`
//! against the router directly, with no real TCP socket.

use axum::http::StatusCode;
use kanban_domain::{CardPriority, CardStatus, CardUpdate, CreateCardOptions};
use kanban_server::test_helpers::{
    json_of, make_sqlite_state, make_state, send, send_with_headers,
};
use kanban_service::api::CardResponse;
use kanban_service::KanbanOperations;
use std::collections::HashSet;
use tempfile::tempdir;
use uuid::Uuid;

fn etag_of(response: &axum::response::Response) -> String {
    response
        .headers()
        .get("etag")
        .expect("etag header")
        .to_str()
        .unwrap()
        .to_string()
}

fn is_quoted_32_hex(tag: &str) -> bool {
    tag.len() == 34
        && tag.starts_with('"')
        && tag.ends_with('"')
        && tag[1..33]
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

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
async fn test_list_cards_unknown_board_returns_404() {
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

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
    assert_eq!(
        json["message"],
        format!("Board {random_board_id} not found")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_empty_board_returns_200_empty_page() {
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
        &format!("/v1/boards/{}/cards", board_id),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["items"], serde_json::json!([]));
    assert_eq!(json["total"], 0);
    assert_eq!(json["total_pages"], 0);
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

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_returns_the_cards_of_every_column_of_the_board() {
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
        ctx.create_card(board_id, col1_id, "Card 1".to_string(), Default::default())
            .unwrap();
        ctx.create_card(board_id, col2_id, "Card 2".to_string(), Default::default())
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
    let titles: std::collections::HashSet<_> =
        arr.iter().map(|c| c["title"].as_str().unwrap()).collect();
    assert_eq!(titles, ["Card 1", "Card 2"].into_iter().collect());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_archived_include_loads_archived_bodies_through_the_per_id_card_tier() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let archived_id: Uuid;
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
        archived_id = ctx
            .create_card(
                board_id,
                col_id,
                "Archived Card".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        ctx.archive_card(archived_id).unwrap();
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
    let arr = json["items"].as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert!(arr.iter().any(|c| c["id"] == archived_id.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_archived_only_over_a_sqlite_locator_serves_the_archived_body_from_the_model(
) {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("s.sqlite")).await;

    let board_id: Uuid;
    let archived_id: Uuid;
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
        archived_id = ctx
            .create_card(
                board_id,
                col_id,
                "Archived Card".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        ctx.archive_card(archived_id).unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards?archived=archived_only", board_id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let arr = json["items"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], archived_id.to_string());
    assert!(!arr[0]["archived_at"].is_null());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_resolves_through_the_per_id_card_tier_and_still_404s_a_card_of_another_board(
) {
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
            .create_column(board_a_id, "Column".to_string(), None)
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

    let wrong_board_response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards/{}", board_b_id, card_id),
        None,
    )
    .await;
    assert_eq!(wrong_board_response.status(), StatusCode::NOT_FOUND);
    let wrong_board_json = json_of(wrong_board_response).await;
    assert_eq!(wrong_board_json["code"], "NOT_FOUND");

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards/{}", board_a_id, card_id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["id"], card_id.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_with_a_column_from_another_board_returns_an_empty_page_for_every_archived_selector(
) {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_a_id: Uuid;
    let col_b_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_a_id = ctx
            .create_board("Board A".to_string(), Some("BA".to_string()))
            .unwrap()
            .id;
        let board_b_id = ctx
            .create_board("Board B".to_string(), Some("BB".to_string()))
            .unwrap()
            .id;
        let col_a_id = ctx
            .create_column(board_a_id, "Column A".to_string(), None)
            .unwrap()
            .id;
        col_b_id = ctx
            .create_column(board_b_id, "Column B".to_string(), None)
            .unwrap()
            .id;
        ctx.create_card(
            board_a_id,
            col_a_id,
            "Card in A".to_string(),
            Default::default(),
        )
        .unwrap();
        let card_in_b = ctx
            .create_card(
                board_b_id,
                col_b_id,
                "Card in B".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        ctx.archive_card(card_in_b).unwrap();
        ctx.create_card(
            board_b_id,
            col_b_id,
            "Live card in B".to_string(),
            Default::default(),
        )
        .unwrap();
    }

    for query in ["", "&archived=include", "&archived=archived_only"] {
        let response = send(
            &state,
            "GET",
            &format!(
                "/v1/boards/{}/cards?column_id={}{}",
                board_a_id, col_b_id, query
            ),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let json = json_of(response).await;
        assert_eq!(
            json["items"],
            serde_json::json!([]),
            "query {query:?} should return no cards from another board's column"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_sprint_ids_csv_filters_any_of() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let sprint_a: Uuid;
    let sprint_b: Uuid;
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
        sprint_a = ctx.create_sprint(board_id, None, None).unwrap().id;
        sprint_b = ctx.create_sprint(board_id, None, None).unwrap().id;
        let sprint_c = ctx.create_sprint(board_id, None, None).unwrap().id;

        ctx.create_card(
            board_id,
            col_id,
            "Card A".to_string(),
            CreateCardOptions {
                sprint_id: Some(sprint_a),
                ..Default::default()
            },
        )
        .unwrap();
        ctx.create_card(
            board_id,
            col_id,
            "Card B".to_string(),
            CreateCardOptions {
                sprint_id: Some(sprint_b),
                ..Default::default()
            },
        )
        .unwrap();
        ctx.create_card(
            board_id,
            col_id,
            "Card C".to_string(),
            CreateCardOptions {
                sprint_id: Some(sprint_c),
                ..Default::default()
            },
        )
        .unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!(
            "/v1/boards/{}/cards?sprint_ids={},{}",
            board_id, sprint_a, sprint_b
        ),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let items = json["items"].as_array().unwrap();
    let sprint_ids: HashSet<String> = items
        .iter()
        .map(|c| c["sprint_id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(items.len(), 2, "items: {items:?}");
    assert_eq!(
        sprint_ids,
        HashSet::from([sprint_a.to_string(), sprint_b.to_string()])
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_sprint_id_and_sprint_ids_union() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let sprint_a: Uuid;
    let sprint_b: Uuid;
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
        sprint_a = ctx.create_sprint(board_id, None, None).unwrap().id;
        sprint_b = ctx.create_sprint(board_id, None, None).unwrap().id;

        ctx.create_card(
            board_id,
            col_id,
            "Card A".to_string(),
            CreateCardOptions {
                sprint_id: Some(sprint_a),
                ..Default::default()
            },
        )
        .unwrap();
        ctx.create_card(
            board_id,
            col_id,
            "Card B".to_string(),
            CreateCardOptions {
                sprint_id: Some(sprint_b),
                ..Default::default()
            },
        )
        .unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!(
            "/v1/boards/{}/cards?sprint_id={}&sprint_ids={}",
            board_id, sprint_a, sprint_b
        ),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let items = json["items"].as_array().unwrap();
    let sprint_ids: HashSet<String> = items
        .iter()
        .map(|c| c["sprint_id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(items.len(), 2, "items: {items:?}");
    assert_eq!(
        sprint_ids,
        HashSet::from([sprint_a.to_string(), sprint_b.to_string()])
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_filters_by_status() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let done_id: Uuid;
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
        let _todo_id = ctx
            .create_card(
                board_id,
                col_id,
                "Todo Card".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        done_id = ctx
            .create_card(
                board_id,
                col_id,
                "Done Card".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        ctx.update_card(
            done_id,
            CardUpdate {
                status: Some(CardStatus::Done),
                ..Default::default()
            },
        )
        .unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards?status=done", board_id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "items: {items:?}");
    assert_eq!(items[0]["id"], done_id.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_unknown_status_value_returns_400() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Test Board".to_string(), Some("TB".to_string()))
            .unwrap()
            .id
    };

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards?status=bogus", board_id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_search_matches_title_case_insensitive() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let needle_id: Uuid;
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
        needle_id = ctx
            .create_card(
                board_id,
                col_id,
                "needle in title".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        ctx.create_card(
            board_id,
            col_id,
            "unrelated".to_string(),
            Default::default(),
        )
        .unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards?search=NEEdle", board_id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "items: {items:?}");
    assert_eq!(items[0]["id"], needle_id.to_string());
}

async fn search_matches_sprint_branch_segment(sqlite: bool) {
    let dir = tempdir().unwrap();
    let state = if sqlite {
        make_sqlite_state(&dir.path().join("s.sqlite")).await
    } else {
        make_state(&dir.path().join("s.json"))
    };

    let board_id: Uuid;
    let card_x_id: Uuid;
    let query: String;
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
        let sprint = ctx
            .create_sprint(board_id, Some("zed".to_string()), Some("S".to_string()))
            .unwrap();
        query = format!(
            "{}-{}",
            sprint.prefix.as_deref().unwrap(),
            sprint.sprint_number
        );

        card_x_id = ctx
            .create_card(
                board_id,
                col_id,
                "card x".to_string(),
                CreateCardOptions {
                    sprint_id: Some(sprint.id),
                    ..Default::default()
                },
            )
            .unwrap()
            .id;
        ctx.create_card(board_id, col_id, "card y".to_string(), Default::default())
            .unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards?search={}", board_id, query),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "items: {items:?}");
    assert_eq!(items[0]["id"], card_x_id.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_search_matches_the_sprint_segment_of_the_branch_name() {
    search_matches_sprint_branch_segment(false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_search_over_a_sqlite_locator_matches_the_sprint_branch_segment() {
    search_matches_sprint_branch_segment(true).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_hide_assigned_excludes_sprint_members() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let unassigned_id: Uuid;
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
        let sprint_id = ctx.create_sprint(board_id, None, None).unwrap().id;
        ctx.create_card(
            board_id,
            col_id,
            "Assigned".to_string(),
            CreateCardOptions {
                sprint_id: Some(sprint_id),
                ..Default::default()
            },
        )
        .unwrap();
        unassigned_id = ctx
            .create_card(
                board_id,
                col_id,
                "Unassigned".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards?hide_assigned=true", board_id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "items: {items:?}");
    assert_eq!(items[0]["id"], unassigned_id.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_sort_by_priority_descending() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let low_id: Uuid;
    let critical_id: Uuid;
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
        low_id = ctx
            .create_card(
                board_id,
                col_id,
                "Low".to_string(),
                CreateCardOptions {
                    priority: Some(CardPriority::Low),
                    ..Default::default()
                },
            )
            .unwrap()
            .id;
        critical_id = ctx
            .create_card(
                board_id,
                col_id,
                "Critical".to_string(),
                CreateCardOptions {
                    priority: Some(CardPriority::Critical),
                    ..Default::default()
                },
            )
            .unwrap()
            .id;
    }

    let response = send(
        &state,
        "GET",
        &format!(
            "/v1/boards/{}/cards?sort=priority&sort_order=descending",
            board_id
        ),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let items = json["items"].as_array().unwrap();
    let ids: Vec<String> = items
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, vec![critical_id.to_string(), low_id.to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_status_and_column_and_search_compose() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let col1_id: Uuid;
    let target_id: Uuid;
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
        let col2_id = ctx
            .create_column(board_id, "Column 2".to_string(), None)
            .unwrap()
            .id;

        let wrong_status = ctx
            .create_card(
                board_id,
                col1_id,
                "keyword card".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        let wrong_search = ctx
            .create_card(
                board_id,
                col1_id,
                "unrelated card".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        let wrong_column = ctx
            .create_card(
                board_id,
                col2_id,
                "keyword card".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        target_id = ctx
            .create_card(
                board_id,
                col1_id,
                "keyword card".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;

        for id in [wrong_search, wrong_column, target_id] {
            ctx.update_card(
                id,
                CardUpdate {
                    status: Some(CardStatus::Done),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        let _ = wrong_status;
    }

    let response = send(
        &state,
        "GET",
        &format!(
            "/v1/boards/{}/cards?column_id={}&status=done&search=keyword",
            board_id, col1_id
        ),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "items: {items:?}");
    assert_eq!(items[0]["id"], target_id.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_archived_include_with_status_filters_dangling_column_archived_cards() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let todo_id: Uuid;
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
        todo_id = ctx
            .create_card(
                board_id,
                col_id,
                "Todo Archived".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        let done_id = ctx
            .create_card(
                board_id,
                col_id,
                "Done Archived".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        ctx.update_card(
            done_id,
            CardUpdate {
                status: Some(CardStatus::Done),
                ..Default::default()
            },
        )
        .unwrap();
        ctx.archive_card(todo_id).unwrap();
        ctx.archive_card(done_id).unwrap();
        ctx.delete_column(col_id).unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards?archived=include&status=todo", board_id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "items: {items:?}");
    assert_eq!(items[0]["id"], todo_id.to_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_malformed_sprint_ids_returns_400() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Test Board".to_string(), Some("TB".to_string()))
            .unwrap()
            .id
    };

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards?sprint_ids=notauuid", board_id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let valid = Uuid::new_v4();
    let response = send(
        &state,
        "GET",
        &format!(
            "/v1/boards/{}/cards?sprint_ids={},notauuid",
            board_id, valid
        ),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_carries_etag_header() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, card_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col_id = ctx
            .create_column(board_id, "To Do".to_string(), None)
            .unwrap()
            .id;
        let card_id = ctx
            .create_card(board_id, col_id, "Task".to_string(), Default::default())
            .unwrap()
            .id;
        (board_id, card_id)
    };

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/cards/{}", board_id, card_id),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let tag = etag_of(&response);
    assert!(
        is_quoted_32_hex(&tag),
        "expected quoted 32-hex etag, got {tag}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_with_matching_if_none_match_returns_304() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (board_id, card_id) = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col_id = ctx
            .create_column(board_id, "To Do".to_string(), None)
            .unwrap()
            .id;
        let card_id = ctx
            .create_card(board_id, col_id, "Task".to_string(), Default::default())
            .unwrap()
            .id;
        (board_id, card_id)
    };

    let uri = format!("/v1/boards/{}/cards/{}", board_id, card_id);
    let first = send(&state, "GET", &uri, None).await;
    let tag = etag_of(&first);

    let second = send_with_headers(&state, "GET", &uri, None, &[("if-none-match", &tag)]).await;

    assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(etag_of(&second), tag);
    let bytes = axum::body::to_bytes(second.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(bytes.is_empty());
}
