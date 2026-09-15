#![cfg(feature = "test-helpers")]

//! POST/DELETE dependency-graph write routes under /v1/cards/{id}/{children,blocks,related}.

use kanban_server::state::AppState;
use kanban_server::test_helpers::{json_of, make_sqlite_state, make_state, send};
use kanban_service::KanbanOperations;
use serde_json::json;
use std::time::Duration;
use tempfile::tempdir;
use uuid::Uuid;

async fn seed_cards(state: &AppState, n: usize) -> Vec<Uuid> {
    let mut ctx = state.ctx.lock().await;
    let board = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap();
    let column = ctx
        .create_column(board.id, "Todo".to_string(), None)
        .unwrap();
    (0..n)
        .map(|i| {
            ctx.create_card(board.id, column.id, format!("Card {i}"), Default::default())
                .unwrap()
                .id
        })
        .collect()
}

async fn test_attach_children_returns_the_updated_graph_json(state: AppState) {
    let cards = seed_cards(&state, 3).await;
    let (parent, c1, c2) = (cards[0], cards[1], cards[2]);

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{parent}/children"),
        Some(&json!({"children": [c1, c2]})),
    )
    .await;
    assert_eq!(response.status(), 200);
    let body = json_of(response).await;
    let children: Vec<Uuid> = serde_json::from_value(body["children"].clone()).unwrap();
    assert!(children.contains(&c1) && children.contains(&c2));

    let response = send(&state, "GET", &format!("/v1/cards/{parent}/graph"), None).await;
    let body = json_of(response).await;
    let children: Vec<Uuid> = serde_json::from_value(body["children"].clone()).unwrap();
    assert!(children.contains(&c1) && children.contains(&c2));

    let response = send(&state, "GET", &format!("/v1/cards/{c1}/graph"), None).await;
    let body = json_of(response).await;
    let parents: Vec<Uuid> = serde_json::from_value(body["parents"].clone()).unwrap();
    assert_eq!(parents, vec![parent]);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_attach_children_returns_the_updated_graph_json_backend() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    test_attach_children_returns_the_updated_graph_json(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_attach_children_returns_the_updated_graph_sqlite_backend() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    test_attach_children_returns_the_updated_graph_json(state).await;
}

async fn test_block_write_then_read_round_trips_severity_over_the_wire(state: AppState) {
    let cards = seed_cards(&state, 2).await;
    let (blocker, blocked) = (cards[0], cards[1]);

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{blocker}/blocks"),
        Some(&json!({"blocked": blocked, "severity": "critical"})),
    )
    .await;
    assert_eq!(response.status(), 200);
    let body = json_of(response).await;
    assert_eq!(
        body["block_edges"],
        json!([{"blocker": blocker, "blocked": blocked, "severity": "critical"}])
    );

    let response = send(&state, "GET", &format!("/v1/cards/{blocked}/graph"), None).await;
    assert_eq!(response.status(), 200);
    let body = json_of(response).await;
    let blocked_by: Vec<Uuid> = serde_json::from_value(body["blocked_by"].clone()).unwrap();
    assert_eq!(blocked_by, vec![blocker]);
    assert_eq!(
        body["block_edges"],
        json!([{"blocker": blocker, "blocked": blocked, "severity": "critical"}])
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_block_write_then_read_round_trips_severity_over_the_wire_json() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    test_block_write_then_read_round_trips_severity_over_the_wire(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_block_write_then_read_round_trips_severity_over_the_wire_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    test_block_write_then_read_round_trips_severity_over_the_wire(state).await;
}

async fn test_related_write_round_trips_kind(state: AppState) {
    let cards = seed_cards(&state, 2).await;
    let (a, b) = (cards[0], cards[1]);

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{a}/related"),
        Some(&json!({"other": b, "kind": "duplicates"})),
    )
    .await;
    assert_eq!(response.status(), 200);

    for id in [a, b] {
        let response = send(&state, "GET", &format!("/v1/cards/{id}/graph"), None).await;
        let body = json_of(response).await;
        let related_edges = body["related_edges"].as_array().unwrap();
        assert_eq!(related_edges.len(), 1);
        let edge = &related_edges[0];
        let source = Uuid::parse_str(edge["source"].as_str().unwrap()).unwrap();
        let target = Uuid::parse_str(edge["target"].as_str().unwrap()).unwrap();
        let endpoints: std::collections::HashSet<Uuid> = [source, target].into_iter().collect();
        assert_eq!(endpoints, [a, b].into_iter().collect());
        assert_eq!(edge["kind"], "duplicates");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_related_write_round_trips_kind_json() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    test_related_write_round_trips_kind(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_related_write_round_trips_kind_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    test_related_write_round_trips_kind(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_related_write_defaults_kind_to_general_json() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let cards = seed_cards(&state, 2).await;
    let (a, b) = (cards[0], cards[1]);

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{a}/related"),
        Some(&json!({"other": b})),
    )
    .await;
    assert_eq!(response.status(), 200);
    let body = json_of(response).await;
    assert_eq!(body["related_edges"][0]["kind"], "general");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_block_write_defaults_severity_to_medium_json() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let cards = seed_cards(&state, 2).await;
    let (blocker, blocked) = (cards[0], cards[1]);

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{blocker}/blocks"),
        Some(&json!({"blocked": blocked})),
    )
    .await;
    assert_eq!(response.status(), 200);
    let body = json_of(response).await;
    assert_eq!(body["block_edges"][0]["severity"], "medium");
}

async fn test_detach_child_returns_204_and_removes_the_spawns_edge(state: AppState) {
    let cards = seed_cards(&state, 2).await;
    let (parent, child) = (cards[0], cards[1]);
    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{parent}/children"),
        Some(&json!({"children": [child]})),
    )
    .await;
    assert_eq!(response.status(), 200);

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/cards/{parent}/children/{child}"),
        None,
    )
    .await;
    assert_eq!(response.status(), 204);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(body.is_empty());

    let response = send(&state, "GET", &format!("/v1/cards/{parent}/graph"), None).await;
    let body = json_of(response).await;
    assert_eq!(body["children"], json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_detach_child_returns_204_and_removes_the_spawns_edge_json() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    test_detach_child_returns_204_and_removes_the_spawns_edge(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_detach_child_returns_204_and_removes_the_spawns_edge_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    test_detach_child_returns_204_and_removes_the_spawns_edge(state).await;
}

async fn test_unblock_returns_204_and_removes_the_blocks_edge(state: AppState) {
    let cards = seed_cards(&state, 2).await;
    let (blocker, blocked) = (cards[0], cards[1]);
    send(
        &state,
        "POST",
        &format!("/v1/cards/{blocker}/blocks"),
        Some(&json!({"blocked": blocked})),
    )
    .await;

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/cards/{blocker}/blocks/{blocked}"),
        None,
    )
    .await;
    assert_eq!(response.status(), 204);

    let response = send(&state, "GET", &format!("/v1/cards/{blocked}/graph"), None).await;
    let body = json_of(response).await;
    assert_eq!(body["blocked_by"], json!([]));
    assert_eq!(body["block_edges"], json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_unblock_returns_204_and_removes_the_blocks_edge_json() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    test_unblock_returns_204_and_removes_the_blocks_edge(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_unblock_returns_204_and_removes_the_blocks_edge_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    test_unblock_returns_204_and_removes_the_blocks_edge(state).await;
}

async fn test_dissociate_returns_204_and_removes_the_relates_edge(state: AppState) {
    let cards = seed_cards(&state, 2).await;
    let (a, b) = (cards[0], cards[1]);
    send(
        &state,
        "POST",
        &format!("/v1/cards/{a}/related"),
        Some(&json!({"other": b})),
    )
    .await;

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/cards/{b}/related/{a}"),
        None,
    )
    .await;
    assert_eq!(response.status(), 204);

    for id in [a, b] {
        let response = send(&state, "GET", &format!("/v1/cards/{id}/graph"), None).await;
        let body = json_of(response).await;
        assert_eq!(body["related"], json!([]));
        assert_eq!(body["related_edges"], json!([]));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_dissociate_returns_204_and_removes_the_relates_edge_json() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    test_dissociate_returns_204_and_removes_the_relates_edge(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_dissociate_returns_204_and_removes_the_relates_edge_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    test_dissociate_returns_204_and_removes_the_relates_edge(state).await;
}

async fn test_spawns_cycle_returns_409_cycle_detected(state: AppState) {
    let cards = seed_cards(&state, 2).await;
    let (a, b) = (cards[0], cards[1]);

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{a}/children"),
        Some(&json!({"children": [b]})),
    )
    .await;
    assert_eq!(response.status(), 200);

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{b}/children"),
        Some(&json!({"children": [a]})),
    )
    .await;
    assert_eq!(response.status(), 409);
    let body = json_of(response).await;
    assert_eq!(body["code"], "CYCLE_DETECTED");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_spawns_cycle_returns_409_cycle_detected_json() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    test_spawns_cycle_returns_409_cycle_detected(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_spawns_cycle_returns_409_cycle_detected_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    test_spawns_cycle_returns_409_cycle_detected(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_self_block_returns_422_self_reference_json() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let cards = seed_cards(&state, 1).await;
    let id = cards[0];

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{id}/blocks"),
        Some(&json!({"blocked": id})),
    )
    .await;
    assert_eq!(response.status(), 422);
    let body = json_of(response).await;
    assert_eq!(body["code"], "SELF_REFERENCE");
}

async fn test_duplicate_block_returns_409_duplicate_edge(state: AppState) {
    let cards = seed_cards(&state, 2).await;
    let (blocker, blocked) = (cards[0], cards[1]);

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{blocker}/blocks"),
        Some(&json!({"blocked": blocked})),
    )
    .await;
    assert_eq!(response.status(), 200);

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{blocker}/blocks"),
        Some(&json!({"blocked": blocked})),
    )
    .await;
    assert_eq!(response.status(), 409);
    let body = json_of(response).await;
    assert_eq!(body["code"], "DUPLICATE_EDGE");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_duplicate_block_returns_409_duplicate_edge_json() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    test_duplicate_block_returns_409_duplicate_edge(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_duplicate_block_returns_409_duplicate_edge_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    test_duplicate_block_returns_409_duplicate_edge(state).await;
}

async fn test_remove_missing_edge_returns_404_edge_not_found(state: AppState) {
    let cards = seed_cards(&state, 2).await;
    let (a, b) = (cards[0], cards[1]);

    let response = send(&state, "DELETE", &format!("/v1/cards/{a}/blocks/{b}"), None).await;
    assert_eq!(response.status(), 404);
    let body = json_of(response).await;
    assert_eq!(body["code"], "EDGE_NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remove_missing_edge_returns_404_edge_not_found_json() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    test_remove_missing_edge_returns_404_edge_not_found(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remove_missing_edge_returns_404_edge_not_found_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    test_remove_missing_edge_returns_404_edge_not_found(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_graph_write_on_unknown_card_returns_404_not_found_json() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let cards = seed_cards(&state, 1).await;
    let real = cards[0];
    let unknown = Uuid::new_v4();

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{unknown}/children"),
        Some(&json!({"children": [real]})),
    )
    .await;
    assert_eq!(response.status(), 404);
    let body = json_of(response).await;
    assert_eq!(body["code"], "NOT_FOUND");

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{real}/children"),
        Some(&json!({"children": [unknown]})),
    )
    .await;
    assert_eq!(response.status(), 404);
    let body = json_of(response).await;
    assert_eq!(body["code"], "NOT_FOUND");
}

async fn test_attach_empty_children_pins_observed_behavior(state: AppState) {
    let cards = seed_cards(&state, 1).await;
    let id = cards[0];

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{id}/children"),
        Some(&json!({"children": []})),
    )
    .await;
    assert_eq!(response.status(), 200);
    let body = json_of(response).await;
    assert_eq!(body["children"], json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_attach_empty_children_pins_observed_behavior_json() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    test_attach_empty_children_pins_observed_behavior(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_attach_empty_children_pins_observed_behavior_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    test_attach_empty_children_pins_observed_behavior(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_graph_writes_emit_card_updated_frames() {
    use kanban_service::api::{ChangeKind, EntityType};

    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let cards = seed_cards(&state, 6).await;

    let mut rx = state.event_tx.subscribe();

    async fn next_frame(
        rx: &mut tokio::sync::broadcast::Receiver<kanban_service::api::ChangeEventFrame>,
    ) -> kanban_service::api::ChangeEventFrame {
        tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("timed out waiting for a change event frame")
            .expect("broadcast channel closed unexpectedly")
    }

    let (c0, c1, c2, c3, c4, c5) = (cards[0], cards[1], cards[2], cards[3], cards[4], cards[5]);

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{c0}/children"),
        Some(&json!({"children": [c1]})),
    )
    .await;
    assert_eq!(response.status(), 200);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_type, Some(EntityType::Card));
    assert_eq!(frame.entity_id, Some(c0));
    assert_eq!(frame.kind, Some(ChangeKind::Updated));
    assert_eq!(frame.writer_instance_id, state.instance_id);

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{c2}/blocks"),
        Some(&json!({"blocked": c3})),
    )
    .await;
    assert_eq!(response.status(), 200);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_id, Some(c2));

    let response = send(
        &state,
        "POST",
        &format!("/v1/cards/{c4}/related"),
        Some(&json!({"other": c5})),
    )
    .await;
    assert_eq!(response.status(), 200);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_id, Some(c4));

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/cards/{c0}/children/{c1}"),
        None,
    )
    .await;
    assert_eq!(response.status(), 204);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_id, Some(c0));

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/cards/{c2}/blocks/{c3}"),
        None,
    )
    .await;
    assert_eq!(response.status(), 204);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_id, Some(c2));

    let response = send(
        &state,
        "DELETE",
        &format!("/v1/cards/{c4}/related/{c5}"),
        None,
    )
    .await;
    assert_eq!(response.status(), 204);
    let frame = next_frame(&mut rx).await;
    assert_eq!(frame.entity_id, Some(c4));
}
