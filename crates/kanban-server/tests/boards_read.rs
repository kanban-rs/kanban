#![cfg(feature = "test-helpers")]

//! Board read routes (GET /v1/boards, GET /v1/boards/{id}).
//! Read-only, no mutation, no event broadcast. Established via `tower::ServiceExt::oneshot`
//! against the router directly, with no real TCP socket.

use axum::http::StatusCode;
use kanban_server::test_helpers::{
    json_of, make_sqlite_state, make_state, send, send_with_headers,
};
use kanban_service::KanbanOperations;
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
async fn test_list_boards_empty_returns_200_empty_page() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let response = send(&state, "GET", "/v1/boards", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["items"], serde_json::json!([]));
    assert_eq!(json["total"], 0);
    assert_eq!(json["total_pages"], 0);
    assert_eq!(json["page"], 1);
    assert_eq!(json["page_size"], 50);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_boards_returns_all_seeded_boards() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board1_id: Uuid;
    let board2_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board1_id = ctx
            .create_board("Board 1".to_string(), Some("B1".to_string()))
            .unwrap()
            .id;
        board2_id = ctx
            .create_board("Board 2".to_string(), Some("B2".to_string()))
            .unwrap()
            .id;
    }

    let response = send(&state, "GET", "/v1/boards", None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;

    let arr = json["items"].as_array().expect("items should be an array");
    assert_eq!(arr.len(), 2, "should have 2 boards");

    let returned_ids: std::collections::HashSet<_> = arr
        .iter()
        .map(|b| Uuid::parse_str(b["id"].as_str().unwrap()).expect("id should be a valid UUID"))
        .collect();

    let expected_ids: std::collections::HashSet<_> =
        vec![board1_id, board2_id].into_iter().collect();
    assert_eq!(
        returned_ids, expected_ids,
        "returned ids should match seeded ids"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_returns_board_response_for_existing_id() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("My Board".to_string(), Some("MB".to_string()))
            .unwrap()
            .id;
    }

    let response = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;

    assert_eq!(json["id"], board_id.to_string());
    assert_eq!(json["name"], "My Board");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_response_omits_internal_allocation_state() {
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

    let response = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;

    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;

    for hidden in [
        "card_counter",
        "next_sprint_number",
        "sprint_counters",
        "sprint_names",
        "sprint_name_used_count",
    ] {
        assert!(
            json.get(hidden).is_none(),
            "field {} should be absent from response",
            hidden
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_unknown_id_returns_404_not_found() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let random_id = Uuid::new_v4();

    let response = send(&state, "GET", &format!("/v1/boards/{}", random_id), None).await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;

    assert_eq!(json["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_archived_board_response_has_archived_at_stamped() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Archived Board".to_string(), Some("AB".to_string()))
            .unwrap()
            .id;
        ctx.archive_board(board_id).unwrap();
    }

    let response = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;

    assert_eq!(
        response.status(),
        StatusCode::OK,
        "get_board is unfiltered and must still resolve an archived board"
    );
    let json = json_of(response).await;

    assert!(
        json["archived_at"].is_string(),
        "an archived board's response must be stamped with archived_at, not look live: {json}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_boards_returns_the_created_board() {
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
    let arr = json["items"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["name"], "Board 1");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_returns_the_board_by_id() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("My Board".to_string(), Some("MB".to_string()))
            .unwrap()
            .id;
    }

    let response = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert_eq!(json["id"], board_id.to_string());
    assert_eq!(json["name"], "My Board");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_archived_board_resolves_through_the_per_id_tier_and_stamps_archived_at() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Archived Board".to_string(), Some("AB".to_string()))
            .unwrap()
            .id;
        ctx.archive_board(board_id).unwrap();
    }

    let response = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert!(json["archived_at"].is_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_unknown_id_records_missing_on_the_per_id_tier_and_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let random_id = Uuid::new_v4();
    let response = send(&state, "GET", &format!("/v1/boards/{}", random_id), None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_archived_board_on_a_sqlite_locator_stamps_archived_at_and_syncs_the_per_id_tier()
{
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Archived Board".to_string(), Some("AB".to_string()))
            .unwrap()
            .id;
        ctx.archive_board(board_id).unwrap();
    }

    let response = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert!(json["archived_at"].is_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_boards_after_reading_a_single_archived_board_still_omits_it() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Archived Board".to_string(), Some("AB".to_string()))
            .unwrap()
            .id;
        ctx.archive_board(board_id).unwrap();
    }

    let first = json_of(send(&state, "GET", "/v1/boards", None).await).await;
    assert_eq!(first["items"], serde_json::json!([]));

    let get_resp = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;
    assert_eq!(get_resp.status(), StatusCode::OK);

    let second = json_of(send(&state, "GET", "/v1/boards", None).await).await;
    assert_eq!(
        second["items"],
        serde_json::json!([]),
        "a re-list after reading the archived board by id must not leak it back in"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_board_then_get_board_returns_the_updated_name_across_requests() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Original".to_string(), Some("OG".to_string()))
            .unwrap()
            .id;
    }

    let get1 = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;
    assert_eq!(get1.status(), StatusCode::OK);

    let patch_body = serde_json::json!({ "name": "Renamed" });
    let patch_resp = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{}", board_id),
        Some(&patch_body),
    )
    .await;
    assert_eq!(patch_resp.status(), StatusCode::OK);

    let get2 = json_of(send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await).await;
    assert_eq!(get2["name"], "Renamed");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_carries_etag_header() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    let response = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("application/json")
    );
    let tag = etag_of(&response);
    assert!(
        is_quoted_32_hex(&tag),
        "expected quoted 32-hex etag, got {tag}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_with_matching_if_none_match_returns_304_empty_body_with_etag() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    let first = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;
    let tag = etag_of(&first);

    let second = send_with_headers(
        &state,
        "GET",
        &format!("/v1/boards/{}", board_id),
        None,
        &[("if-none-match", &tag)],
    )
    .await;

    assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
    let response_tag = etag_of(&second);
    assert_eq!(response_tag, tag);
    let bytes = axum::body::to_bytes(second.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(bytes.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_with_stale_if_none_match_returns_200_and_body() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    let response = send_with_headers(
        &state,
        "GET",
        &format!("/v1/boards/{}", board_id),
        None,
        &[("if-none-match", "\"00000000000000000000000000000000\"")],
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let tag = etag_of(&response);
    assert_ne!(tag, "\"00000000000000000000000000000000\"");
    let json = json_of(response).await;
    assert_eq!(json["name"], "Board");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_with_weak_if_none_match_returns_304_not_modified() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    let first = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;
    let tag = etag_of(&first);
    let weak = format!("W/{tag}");

    let second = send_with_headers(
        &state,
        "GET",
        &format!("/v1/boards/{}", board_id),
        None,
        &[("if-none-match", &weak)],
    )
    .await;

    assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_etag_changes_after_update() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    let first = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;
    let etag1 = etag_of(&first);

    let patch_body = serde_json::json!({ "name": "Renamed" });
    let patch_resp = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{}", board_id),
        Some(&patch_body),
    )
    .await;
    assert_eq!(patch_resp.status(), StatusCode::OK);

    let second = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;
    let etag2 = etag_of(&second);

    assert_ne!(etag1, etag2);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_archival_changes_etag() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        ctx.create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id
    };

    let first = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;
    let etag1 = etag_of(&first);

    {
        let mut ctx = state.ctx.lock().await;
        ctx.archive_board(board_id).unwrap();
    }

    let second = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;
    let etag2 = etag_of(&second);

    assert_ne!(etag1, etag2);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_etag_is_stable_across_repeated_gets_on_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;

    let board_id = {
        let mut ctx = state.ctx.lock().await;
        let board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let col_id = ctx
            .create_column(board_id, "To Do".to_string(), None)
            .unwrap()
            .id;
        ctx.create_card(board_id, col_id, "Task".to_string(), Default::default())
            .unwrap();
        board_id
    };

    let first = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;
    let etag1 = etag_of(&first);
    let second = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;
    let etag2 = etag_of(&second);
    let third = send(&state, "GET", &format!("/v1/boards/{}", board_id), None).await;
    let etag3 = etag_of(&third);

    assert_eq!(etag1, etag2);
    assert_eq!(etag2, etag3);

    let fourth = send_with_headers(
        &state,
        "GET",
        &format!("/v1/boards/{}", board_id),
        None,
        &[("if-none-match", &etag1)],
    )
    .await;
    assert_eq!(fourth.status(), StatusCode::NOT_MODIFIED);
}
