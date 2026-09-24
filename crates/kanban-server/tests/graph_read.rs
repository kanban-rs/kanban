#![cfg(feature = "test-helpers")]

//! GET /v1/cards/{id}/graph. Read-only, no mutation, no event broadcast.
//! Established via `tower::ServiceExt::oneshot` against the router directly,
//! with no real TCP socket.

use kanban_domain::{DependencyGraph, GraphOperations, RelatesKind, Severity};
use kanban_server::test_helpers::{json_of, make_sqlite_state, make_state, send};
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
        let column = ctx
            .create_column(board.id, "Todo".to_string(), None)
            .unwrap();
        subject = ctx
            .create_card(
                board.id,
                column.id,
                "Subject".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        child = ctx
            .create_card(board.id, column.id, "Child".to_string(), Default::default())
            .unwrap()
            .id;
        blocker = ctx
            .create_card(
                board.id,
                column.id,
                "Blocker".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        rel = ctx
            .create_card(
                board.id,
                column.id,
                "Related".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        let outsider_a = ctx
            .create_card(
                board.id,
                column.id,
                "Outsider A".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        outsider_b = ctx
            .create_card(
                board.id,
                column.id,
                "Outsider B".to_string(),
                Default::default(),
            )
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
        ctx.create_column(board.id, "Todo".to_string(), None)
            .unwrap();
    }

    let unknown_id = Uuid::new_v4();
    let response = send(
        &state,
        "GET",
        &format!("/v1/cards/{unknown_id}/graph"),
        None,
    )
    .await;

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
        let column = ctx
            .create_column(board.id, "Todo".to_string(), None)
            .unwrap();
        card_id = ctx
            .create_card(
                board.id,
                column.id,
                "Lonely".to_string(),
                Default::default(),
            )
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
            "block_edges": [],
            "related_edges": [],
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_graph_returns_the_children() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let subject: Uuid;
    let child: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        let board = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap();
        let column = ctx
            .create_column(board.id, "Todo".to_string(), None)
            .unwrap();
        subject = ctx
            .create_card(
                board.id,
                column.id,
                "Subject".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        child = ctx
            .create_card(board.id, column.id, "Child".to_string(), Default::default())
            .unwrap()
            .id;
        ctx.attach_children(subject, vec![child]).unwrap();
    }

    let response = send(&state, "GET", &format!("/v1/cards/{subject}/graph"), None).await;
    assert_eq!(response.status(), 200);
    let response_json = json_of(response).await;
    assert_eq!(response_json["children"], json!([child]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_graph_on_a_sqlite_locator_returns_the_children() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;

    let subject: Uuid;
    let child: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        let board = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap();
        let column = ctx
            .create_column(board.id, "Todo".to_string(), None)
            .unwrap();
        subject = ctx
            .create_card(
                board.id,
                column.id,
                "Subject".to_string(),
                Default::default(),
            )
            .unwrap()
            .id;
        child = ctx
            .create_card(board.id, column.id, "Child".to_string(), Default::default())
            .unwrap()
            .id;
        ctx.attach_children(subject, vec![child]).unwrap();
    }

    let response = send(&state, "GET", &format!("/v1/cards/{subject}/graph"), None).await;
    assert_eq!(response.status(), 200);
    let response_json = json_of(response).await;
    assert_eq!(response_json["children"], json!([child]));
}

fn seed_whole_graph(
    ctx: &mut (impl KanbanOperations + GraphOperations),
) -> (Uuid, Uuid, Uuid, Uuid, Uuid, Uuid) {
    let board = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap();
    let column = ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    let parent = ctx
        .create_card(
            board.id,
            column.id,
            "Parent".to_string(),
            Default::default(),
        )
        .unwrap()
        .id;
    let child = ctx
        .create_card(board.id, column.id, "Child".to_string(), Default::default())
        .unwrap()
        .id;
    let blocker = ctx
        .create_card(
            board.id,
            column.id,
            "Blocker".to_string(),
            Default::default(),
        )
        .unwrap()
        .id;
    let rel = ctx
        .create_card(
            board.id,
            column.id,
            "Related".to_string(),
            Default::default(),
        )
        .unwrap()
        .id;
    let doomed = ctx
        .create_card(
            board.id,
            column.id,
            "Doomed".to_string(),
            Default::default(),
        )
        .unwrap()
        .id;

    ctx.attach_children(parent, vec![child]).unwrap();
    ctx.block(blocker, parent, Severity::High).unwrap();
    ctx.relate(parent, rel, RelatesKind::Duplicates).unwrap();
    ctx.attach_children(parent, vec![doomed]).unwrap();
    ctx.archive_card(doomed).unwrap();

    (parent, child, blocker, rel, doomed, board.id)
}

fn assert_whole_graph_shape(graph: &DependencyGraph, parent: Uuid, child: Uuid, doomed: Uuid) {
    let live_spawn = graph
        .spawns_edges()
        .iter()
        .find(|e| e.base.source == parent && e.base.target == child)
        .expect("expected the live parent->child spawns edge");
    assert!(live_spawn.base.archived_at.is_none());

    let archived_spawn = graph
        .spawns_edges()
        .iter()
        .find(|e| e.base.source == parent && e.base.target == doomed)
        .expect("expected the archived parent->doomed spawns edge");
    assert!(archived_spawn.base.archived_at.is_some());

    assert_eq!(graph.blocks_edges()[0].severity, Severity::High);
    assert_eq!(graph.relates_edges()[0].kind, RelatesKind::Duplicates);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_graph_returns_the_whole_graph_including_archived_edges() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let (parent, child, _blocker, _rel, doomed, _board_id) = {
        let mut ctx = state.ctx.lock().await;
        seed_whole_graph(&mut *ctx)
    };

    let stored_graph = state.ctx.lock().await.data_store().get_graph().unwrap();

    let response = send(&state, "GET", "/v1/graph", None).await;
    assert_eq!(response.status(), 200);
    let response_json = json_of(response).await;
    let deserialized: DependencyGraph = serde_json::from_value(response_json).unwrap();

    assert_eq!(deserialized, stored_graph);
    assert_whole_graph_shape(&deserialized, parent, child, doomed);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_graph_on_a_sqlite_locator_returns_the_whole_graph_including_archived_edges() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;

    let (parent, child, _blocker, _rel, doomed, _board_id) = {
        let mut ctx = state.ctx.lock().await;
        seed_whole_graph(&mut *ctx)
    };

    let stored_graph = state.ctx.lock().await.data_store().get_graph().unwrap();

    let response = send(&state, "GET", "/v1/graph", None).await;
    assert_eq!(response.status(), 200);
    let response_json = json_of(response).await;
    let deserialized: DependencyGraph = serde_json::from_value(response_json).unwrap();

    assert_eq!(deserialized, stored_graph);
    assert_whole_graph_shape(&deserialized, parent, child, doomed);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_graph_on_an_empty_store_returns_an_empty_graph_not_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let response = send(&state, "GET", "/v1/graph", None).await;
    assert_eq!(response.status(), 200);
    let response_json = json_of(response).await;
    let deserialized: DependencyGraph = serde_json::from_value(response_json).unwrap();

    assert!(deserialized.is_empty());
    assert_eq!(deserialized.len(), 0);
}
