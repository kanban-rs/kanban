#![cfg(feature = "test-helpers")]

//! GET /v1/cards/{id}/graph. Read-only, no mutation, no event broadcast.
//! Established via `tower::ServiceExt::oneshot` against the router directly,
//! with no real TCP socket.

use kanban_domain::{GraphOperations, RelatesKind, Severity};
use kanban_server::test_helpers::{json_of, make_state, send};
use kanban_service::KanbanOperations;
use serde_json::json;
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_graph_returns_only_the_requested_cards_edges() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let subject: Uuid;
    let child: Uuid;
    let blocker: Uuid;
    let rel: Uuid;
    let outsider_b: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        let board = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap();
        let column = ctx.create_column(board.id, "Todo".to_string(), None).unwrap();
        subject = ctx
            .create_card(board.id, column.id, "Subject".to_string(), Default::default())
            .unwrap()
            .id;
        child = ctx
            .create_card(board.id, column.id, "Child".to_string(), Default::default())
            .unwrap()
            .id;
        blocker = ctx
            .create_card(board.id, column.id, "Blocker".to_string(), Default::default())
            .unwrap()
            .id;
        rel = ctx
            .create_card(board.id, column.id, "Related".to_string(), Default::default())
            .unwrap()
            .id;
        let outsider_a = ctx
            .create_card(board.id, column.id, "Outsider A".to_string(), Default::default())
            .unwrap()
            .id;
        outsider_b = ctx
            .create_card(board.id, column.id, "Outsider B".to_string(), Default::default())
            .unwrap()
            .id;

        ctx.attach_children(subject, vec![child]).unwrap();
        ctx.block(blocker, subject, Severity::Medium).unwrap();
        ctx.relate(subject, rel, RelatesKind::General).unwrap();
        ctx.attach_children(outsider_a, vec![outsider_b]).unwrap();
    }

    let response = send(&state, "GET", &format!("/v1/cards/{subject}/graph"), None).await;

    assert_eq!(response.status(), 200);
    let response_json = json_of(response).await;
    assert_eq!(response_json["card_id"], subject.to_string());
    assert_eq!(response_json["children"], json!([child]));
    assert_eq!(response_json["blocked_by"], json!([blocker]));
    assert_eq!(response_json["related"], json!([rel]));
    assert_eq!(response_json["parents"], json!([]));
    assert_eq!(response_json["blocks"], json!([]));
    assert!(
        !response_json.to_string().contains(&outsider_b.to_string()),
        "response leaked an edge unrelated to the requested card: {response_json}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_graph_unknown_card_returns_404_not_empty_arrays() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    {
        let mut ctx = state.ctx.lock().await;
        let board = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap();
        ctx.create_column(board.id, "Todo".to_string(), None).unwrap();
    }

    let unknown_id = Uuid::new_v4();
    let response = send(&state, "GET", &format!("/v1/cards/{unknown_id}/graph"), None).await;

    assert_eq!(response.status(), 404);
    let response_json = json_of(response).await;
    assert_eq!(response_json["code"], "NOT_FOUND");
    assert!(response_json.get("children").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_graph_existing_card_with_no_edges_returns_200_empty_arrays() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let card_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        let board = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap();
        let column = ctx.create_column(board.id, "Todo".to_string(), None).unwrap();
        card_id = ctx
            .create_card(board.id, column.id, "Lonely".to_string(), Default::default())
            .unwrap()
            .id;
    }

    let response = send(&state, "GET", &format!("/v1/cards/{card_id}/graph"), None).await;

    assert_eq!(response.status(), 200);
    let response_json = json_of(response).await;
    assert_eq!(
        response_json,
        json!({
            "card_id": card_id.to_string(),
            "parents": [],
            "children": [],
            "blocked_by": [],
            "blocks": [],
            "related": [],
        })
    );
}
