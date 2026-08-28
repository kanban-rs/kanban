#![cfg(feature = "test-helpers")]

//! Pagination (`?page=&page_size=`) across the four collection list routes:
//! `GET /v1/boards`, `GET /v1/boards/{b}/columns`, `GET /v1/boards/{b}/cards`,
//! `GET /v1/boards/{b}/sprints`. Established via `tower::ServiceExt::oneshot`
//! against the router directly, with no real TCP socket.

use axum::http::StatusCode;
use kanban_server::test_helpers::{json_of, make_state, send};
use kanban_service::KanbanOperations;
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread")]
async fn test_list_boards_paginates_with_explicit_params() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board 1".to_string(), Some("B1".to_string()))
            .unwrap();
        ctx.create_board("Board 2".to_string(), Some("B2".to_string()))
            .unwrap();
        ctx.create_board("Board 3".to_string(), Some("B3".to_string()))
            .unwrap();
    }

    let response = send(&state, "GET", "/v1/boards?page=1&page_size=2", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;

    assert!(
        json.get("items").is_some(),
        "body should be a page envelope: {json}"
    );
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(json["total"], 3);
    assert_eq!(json["page"], 1);
    assert_eq!(json["page_size"], 2);
    assert_eq!(json["total_pages"], 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_boards_second_page_returns_the_remainder() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board 1".to_string(), Some("B1".to_string()))
            .unwrap();
        ctx.create_board("Board 2".to_string(), Some("B2".to_string()))
            .unwrap();
        ctx.create_board("Board 3".to_string(), Some("B3".to_string()))
            .unwrap();
    }

    let first_page = send(&state, "GET", "/v1/boards?page=1&page_size=2", None).await;
    let first_json = json_of(first_page).await;
    let first_ids: std::collections::HashSet<String> = first_json["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["id"].as_str().unwrap().to_string())
        .collect();

    let response = send(&state, "GET", "/v1/boards?page=2&page_size=2", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(json["total"], 3);
    assert_eq!(json["page"], 2);
    assert_eq!(json["total_pages"], 2);

    let second_id = items[0]["id"].as_str().unwrap().to_string();
    assert!(
        !first_ids.contains(&second_id),
        "second page must not repeat an id from the first page"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_boards_defaults_to_page_1_size_50_when_params_omitted() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board 1".to_string(), Some("B1".to_string()))
            .unwrap();
        ctx.create_board("Board 2".to_string(), Some("B2".to_string()))
            .unwrap();
    }

    let response = send(&state, "GET", "/v1/boards", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["items"].as_array().unwrap().len(), 2);
    assert_eq!(json["total"], 2);
    assert_eq!(json["page"], 1);
    assert_eq!(json["page_size"], 50);
    assert_eq!(json["total_pages"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_boards_body_deserializes_as_page_of_board_response() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board 1".to_string(), Some("B1".to_string()))
            .unwrap();
    }

    let response = send(&state, "GET", "/v1/boards", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let parsed: kanban_service::api::Page<kanban_service::api::BoardResponse> =
        serde_json::from_value(json).expect("body should deserialize as Page<BoardResponse>");
    assert_eq!(parsed.items.len(), 1);
    assert_eq!(parsed.total, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_columns_paginates() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board".to_string(), Some("B".to_string()))
            .unwrap()
            .id;
        ctx.create_column(board_id, "Column 1".to_string(), Some(0))
            .unwrap();
        ctx.create_column(board_id, "Column 2".to_string(), Some(1))
            .unwrap();
        ctx.create_column(board_id, "Column 3".to_string(), Some(2))
            .unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/columns?page=2&page_size=2"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(json["total"], 3);
    assert_eq!(json["total_pages"], 2);
    assert_eq!(items[0]["position"], 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_sprints_paginates() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board".to_string(), Some("B".to_string()))
            .unwrap()
            .id;
        ctx.create_sprint(board_id, Some("SPR".to_string()), Some("Alpha".to_string()))
            .unwrap();
        ctx.create_sprint(board_id, Some("SPR".to_string()), Some("Beta".to_string()))
            .unwrap();
        ctx.create_sprint(board_id, Some("SPR".to_string()), Some("Gamma".to_string()))
            .unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/sprints?page=1&page_size=2"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(json["total"], 3);
    assert_eq!(json["total_pages"], 2);
    for item in items {
        assert!(
            !item["name"].is_null(),
            "sprint name must survive pagination"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_paginates_and_reports_correct_total() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board".to_string(), Some("B".to_string()))
            .unwrap()
            .id;
        let col_id = ctx
            .create_column(board_id, "Column".to_string(), None)
            .unwrap()
            .id;
        ctx.create_card(board_id, col_id, "Card 1".to_string(), Default::default())
            .unwrap();
        ctx.create_card(board_id, col_id, "Card 2".to_string(), Default::default())
            .unwrap();
        ctx.create_card(board_id, col_id, "Card 3".to_string(), Default::default())
            .unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/cards?page=1&page_size=2"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(json["total"], 3);
    assert_eq!(json["page_size"], 2);
    assert_eq!(json["total_pages"], 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_column_filter_and_archived_still_apply_alongside_paging() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    let col1_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board".to_string(), Some("B".to_string()))
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

        ctx.create_card(
            board_id,
            col1_id,
            "Col1 Live 1".to_string(),
            Default::default(),
        )
        .unwrap();
        ctx.create_card(
            board_id,
            col1_id,
            "Col1 Live 2".to_string(),
            Default::default(),
        )
        .unwrap();
        let archived_id = ctx
            .create_card(
                board_id,
                col1_id,
                "Col1 Archived".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        ctx.archive_card(archived_id).unwrap();

        ctx.create_card(
            board_id,
            col2_id,
            "Col2 Live".to_string(),
            Default::default(),
        )
        .unwrap();
    }

    let response = send(
        &state,
        "GET",
        &format!(
            "/v1/boards/{board_id}/cards?column_id={col1_id}&archived=include&page=2&page_size=2"
        ),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(
        json["total"], 3,
        "total should count only col1's live+archived cards"
    );
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    for item in items {
        assert_eq!(item["column_id"], col1_id.to_string());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_pagination_rejects_page_zero_with_422() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board 1".to_string(), Some("B1".to_string()))
            .unwrap();
        ctx.create_board("Board 2".to_string(), Some("B2".to_string()))
            .unwrap();
    }

    let response = send(&state, "GET", "/v1/boards?page=0", None).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = json_of(response).await;
    assert_eq!(json["code"], "VALIDATION_FAILED");
    assert!(
        json.get("items").is_none(),
        "invalid page must not render as a page envelope"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_pagination_rejects_page_size_over_max_with_422() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let response = send(&state, "GET", "/v1/boards?page_size=501", None).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = json_of(response).await;
    assert_eq!(json["code"], "VALIDATION_FAILED");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_pagination_out_of_range_page_returns_empty_items_not_error() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board 1".to_string(), Some("B1".to_string()))
            .unwrap();
        ctx.create_board("Board 2".to_string(), Some("B2".to_string()))
            .unwrap();
    }

    let response = send(&state, "GET", "/v1/boards?page=5&page_size=2", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["items"], serde_json::json!([]));
    assert_eq!(json["total"], 2);
    assert_eq!(json["page"], 5);
    assert_eq!(json["total_pages"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_sprints_unknown_board_with_paging_still_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let random_board_id = Uuid::new_v4();

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{random_board_id}/sprints?page=1&page_size=2"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
    assert!(
        json.get("items").is_none(),
        "a missing board must not render as an empty page"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_columns_unknown_board_with_paging_still_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let random_board_id = Uuid::new_v4();

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{random_board_id}/columns?page=1&page_size=2"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
    assert!(
        json.get("items").is_none(),
        "a missing board must not render as an empty page"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_unknown_board_with_paging_still_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let random_board_id = Uuid::new_v4();

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{random_board_id}/cards?page=1&page_size=2"),
        None,
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
    assert!(
        json.get("items").is_none(),
        "a missing board must not render as an empty page"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_boards_page_envelope_is_absent_on_the_single_entity_get() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board".to_string(), Some("B".to_string()))
            .unwrap()
            .id;
    }

    let response = send(&state, "GET", &format!("/v1/boards/{board_id}"), None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["id"], board_id.to_string());
    assert!(
        json.get("items").is_none(),
        "single-entity GET must stay a bare object"
    );
}
