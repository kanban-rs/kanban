#![cfg(feature = "test-helpers")]

use axum::http::StatusCode;
use kanban_domain::{CreateCardOptions, GraphOperations, KanbanOperations};
use kanban_server::state::AppState;
use kanban_server::test_helpers::{json_of, make_sqlite_state, make_state, send};
use kanban_service::api::{ChangeKind, EntityType, InvalidationDto};
use serde_json::json;
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::broadcast;
use uuid::Uuid;

async fn next_frame(
    rx: &mut broadcast::Receiver<kanban_service::api::ChangeEventFrame>,
) -> kanban_service::api::ChangeEventFrame {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out waiting for a change event frame")
        .expect("broadcast channel closed unexpectedly")
}

struct SeededGraph {
    board_id: Uuid,
    column_id: Uuid,
    card_a: Uuid,
    card_b: Uuid,
    sprint_id: Uuid,
}

async fn seed_graph(state: &AppState) -> SeededGraph {
    let mut ctx = state.ctx.lock().await;
    let board_id = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id;
    let column_id = ctx
        .create_column(board_id, "To Do".to_string(), None)
        .unwrap()
        .id;
    let card_a = ctx
        .create_card(
            board_id,
            column_id,
            "Card A".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap()
        .id;
    let card_b = ctx
        .create_card(
            board_id,
            column_id,
            "Card B".to_string(),
            CreateCardOptions::default(),
        )
        .unwrap()
        .id;
    let sprint_id = ctx.create_sprint(board_id, None, None).unwrap().id;
    ctx.attach_children(card_a, vec![card_b]).unwrap();
    SeededGraph {
        board_id,
        column_id,
        card_a,
        card_b,
        sprint_id,
    }
}

fn assert_cascading_board_delete_invalidation(invalidation: &InvalidationDto, graph: &SeededGraph) {
    match invalidation {
        InvalidationDto::Entities(ids) => {
            assert!(ids.boards.contains(&graph.board_id));
            assert!(ids.columns.contains(&graph.column_id));
            assert!(ids.cards.contains(&graph.card_a));
            assert!(ids.cards.contains(&graph.card_b));
            assert!(ids.sprints.contains(&graph.sprint_id));
            assert!(ids.graph);
        }
        InvalidationDto::All => panic!("expected an Entities invalidation naming the graph"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_cascading_board_delete_frame_names_every_touched_entity_kind() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let graph = seed_graph(&state).await;
    let mut rx = state.event_tx.subscribe();

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/boards/{}", graph.board_id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Board));
    assert_eq!(frame.entity_id, Some(graph.board_id));
    assert_eq!(frame.kind, Some(ChangeKind::Deleted));
    let invalidation = frame
        .invalidation
        .expect("frame must carry an invalidation");
    assert_cascading_board_delete_invalidation(&invalidation, &graph);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_cascading_board_delete_frame_names_every_touched_entity_kind_on_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("s.sqlite")).await;
    let graph = seed_graph(&state).await;
    let mut rx = state.event_tx.subscribe();

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/boards/{}", graph.board_id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Board));
    assert_eq!(frame.entity_id, Some(graph.board_id));
    assert_eq!(frame.kind, Some(ChangeKind::Deleted));
    let invalidation = frame
        .invalidation
        .expect("frame must carry an invalidation");
    assert_cascading_board_delete_invalidation(&invalidation, &graph);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_board_returns_200_with_the_cascade_invalidation_naming_the_subtree() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let graph = seed_graph(&state).await;

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/boards/{}", graph.board_id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = json_of(response).await;
    let invalidation: InvalidationDto = serde_json::from_value(body["invalidation"].clone())
        .expect("body must carry a parseable invalidation");
    assert_cascading_board_delete_invalidation(&invalidation, &graph);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_board_returns_200_with_the_cascade_invalidation_naming_the_subtree_on_sqlite()
{
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("s.sqlite")).await;
    let graph = seed_graph(&state).await;

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/boards/{}", graph.board_id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = json_of(response).await;
    let invalidation: InvalidationDto = serde_json::from_value(body["invalidation"].clone())
        .expect("body must carry a parseable invalidation");
    assert_cascading_board_delete_invalidation(&invalidation, &graph);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mutation_body_invalidation_equals_the_sse_frame_invalidation() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let graph = seed_graph(&state).await;
    let mut rx = state.event_tx.subscribe();

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/boards/{}", graph.board_id),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    let body_invalidation: InvalidationDto =
        serde_json::from_value(body["invalidation"].clone()).unwrap();

    let frame = next_frame(&mut rx).await;
    let frame_invalidation = frame
        .invalidation
        .expect("frame must carry an invalidation");

    assert_eq!(frame_invalidation, body_invalidation);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_write_frame_does_not_carry_a_previous_requests_invalidation() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let graph = seed_graph(&state).await;
    let mut rx = state.event_tx.subscribe();

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/cards/{}", graph.card_a),
        Some(&json!({"title": "Renamed"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let frame_a = next_frame(&mut rx).await;
    match frame_a
        .invalidation
        .expect("frame must carry an invalidation")
    {
        InvalidationDto::Entities(ids) => assert!(ids.cards.contains(&graph.card_a)),
        InvalidationDto::All => panic!("expected an Entities invalidation naming the card"),
    }

    let response = send(
        &state,
        "PATCH",
        &format!("/v1/boards/{}", graph.board_id),
        Some(&json!({"name": "Renamed board"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let frame_b = next_frame(&mut rx).await;
    match frame_b
        .invalidation
        .expect("frame must carry an invalidation")
    {
        InvalidationDto::Entities(ids) => {
            assert!(ids.boards.contains(&graph.board_id));
            assert!(!ids.cards.contains(&graph.card_a));
        }
        InvalidationDto::All => panic!("expected an Entities invalidation naming the board"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_an_external_change_frame_invalidates_everything() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let mut rx = state.event_tx.subscribe();

    state.broadcast_unscoped_change();

    let frame = rx.try_recv().unwrap();
    assert!(frame.entity_type.is_none());
    assert!(frame.entity_id.is_none());
    assert!(frame.kind.is_none());
    assert_eq!(frame.invalidation, Some(InvalidationDto::All));
}
