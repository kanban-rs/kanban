#![cfg(feature = "test-helpers")]

//! POST /v1/boards/{id}/archive, POST /v1/boards/{id}/restore and
//! GET /v1/archived-boards. Archive/restore is proven the identity over the
//! full entity graph on both the JSON and SQLite backends.

use axum::http::StatusCode;
use kanban_domain::{FieldUpdate, GraphOperations, Severity};
use kanban_server::state::AppState;
use kanban_server::test_helpers::{json_of, make_sqlite_state, make_state, send};
use kanban_service::{ColumnUpdate, KanbanOperations};
use tempfile::tempdir;
use uuid::Uuid;

async fn archive_returns_200_with_archived_at_stamped_case(state: AppState) {
    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
    }

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/archive"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let json = json_of(response).await;
    assert!(json["archived_at"].is_string());

    let list_response = send(&state, "GET", "/v1/boards", None).await;
    let list_json = json_of(list_response).await;
    let items = list_json["items"].as_array().unwrap();
    assert!(
        items.iter().all(|b| b["id"] != board_id.to_string()),
        "archived board must not appear in the live list"
    );

    let get_response = send(&state, "GET", &format!("/v1/boards/{board_id}"), None).await;
    let get_json = json_of(get_response).await;
    assert_eq!(get_json["id"], board_id.to_string());
    assert!(get_json["archived_at"].is_string());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_board_returns_200_with_archived_at_stamped() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    archive_returns_200_with_archived_at_stamped_case(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_board_returns_200_with_archived_at_stamped_on_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    archive_returns_200_with_archived_at_stamped_case(state).await;
}

struct SeededGraph {
    board_id: Uuid,
    columns: (Uuid, Uuid),
    cards: (Uuid, Uuid),
    sprint_id: Uuid,
}

async fn seed_nontrivial_graph(state: &AppState) -> SeededGraph {
    let mut ctx = state.ctx.lock().await;
    let board_id = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id;
    let col1 = ctx
        .create_column(board_id, "Todo".to_string(), Some(0))
        .unwrap();
    let col2 = ctx
        .create_column(board_id, "Doing".to_string(), Some(1))
        .unwrap();
    ctx.update_column(
        col1.id,
        ColumnUpdate {
            wip_limit: FieldUpdate::Set(3),
            ..Default::default()
        },
    )
    .unwrap();
    let card1 = ctx
        .create_card(board_id, col1.id, "Card 1".to_string(), Default::default())
        .unwrap();
    let card2 = ctx
        .create_card(board_id, col2.id, "Card 2".to_string(), Default::default())
        .unwrap();
    let sprint_id = ctx
        .create_sprint(board_id, Some("SPR".to_string()), Some("Alpha".to_string()))
        .unwrap()
        .id;
    ctx.attach_children(card1.id, vec![card2.id]).unwrap();
    ctx.block(card1.id, card2.id, Severity::Medium).unwrap();

    SeededGraph {
        board_id,
        columns: (col1.id, col2.id),
        cards: (card1.id, card2.id),
        sprint_id,
    }
}

async fn archive_board_keeps_subtree_reachable_case(state: AppState) {
    let seeded = seed_nontrivial_graph(&state).await;
    let board_id = seeded.board_id;

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/archive"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let columns_json = json_of(
        send(
            &state,
            "GET",
            &format!("/v1/boards/{board_id}/columns"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(columns_json["items"].as_array().unwrap().len(), 2);

    let cards_json =
        json_of(send(&state, "GET", &format!("/v1/boards/{board_id}/cards"), None).await).await;
    assert_eq!(cards_json["items"].as_array().unwrap().len(), 2);

    let sprints_json = json_of(
        send(
            &state,
            "GET",
            &format!("/v1/boards/{board_id}/sprints"),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(sprints_json["items"].as_array().unwrap().len(), 1);

    let graph_json = json_of(
        send(
            &state,
            "GET",
            &format!("/v1/cards/{}/graph", seeded.cards.0),
            None,
        )
        .await,
    )
    .await;
    assert_eq!(
        graph_json["children"].as_array().unwrap().len(),
        1,
        "the spawns edge must survive the board archive"
    );
    assert_eq!(
        graph_json["blocks"].as_array().unwrap().len(),
        1,
        "the blocks edge must survive the board archive"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_board_keeps_subtree_reachable() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    archive_board_keeps_subtree_reachable_case(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_board_keeps_subtree_reachable_on_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    archive_board_keeps_subtree_reachable_case(state).await;
}

async fn restore_board_returns_the_full_subtree_over_the_wire_case(state: AppState) {
    let seeded = seed_nontrivial_graph(&state).await;
    let board_id = seeded.board_id;

    let columns_before = json_of(
        send(
            &state,
            "GET",
            &format!("/v1/boards/{board_id}/columns"),
            None,
        )
        .await,
    )
    .await;
    let cards_before =
        json_of(send(&state, "GET", &format!("/v1/boards/{board_id}/cards"), None).await).await;
    let sprints_before = json_of(
        send(
            &state,
            "GET",
            &format!("/v1/boards/{board_id}/sprints"),
            None,
        )
        .await,
    )
    .await;
    let graph_before = json_of(
        send(
            &state,
            "GET",
            &format!("/v1/cards/{}/graph", seeded.cards.0),
            None,
        )
        .await,
    )
    .await;

    let archive_response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/archive"),
        None,
    )
    .await;
    assert_eq!(archive_response.status(), StatusCode::OK);

    let restore_response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/restore"),
        None,
    )
    .await;
    assert_eq!(restore_response.status(), StatusCode::OK);
    let restore_json = json_of(restore_response).await;
    assert!(
        restore_json.get("archived_at").is_none(),
        "a restored board must omit the archived_at key entirely"
    );

    let columns_after = json_of(
        send(
            &state,
            "GET",
            &format!("/v1/boards/{board_id}/columns"),
            None,
        )
        .await,
    )
    .await;
    let cards_after =
        json_of(send(&state, "GET", &format!("/v1/boards/{board_id}/cards"), None).await).await;
    let sprints_after = json_of(
        send(
            &state,
            "GET",
            &format!("/v1/boards/{board_id}/sprints"),
            None,
        )
        .await,
    )
    .await;
    let graph_after = json_of(
        send(
            &state,
            "GET",
            &format!("/v1/cards/{}/graph", seeded.cards.0),
            None,
        )
        .await,
    )
    .await;

    assert_eq!(columns_before, columns_after);
    assert_eq!(cards_before, cards_after);
    assert_eq!(sprints_before, sprints_after);
    assert_eq!(graph_before, graph_after);

    let _ = seeded.columns;
    let _ = seeded.sprint_id;

    let archived_list_json = json_of(send(&state, "GET", "/v1/archived-boards", None).await).await;
    assert!(archived_list_json["items"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_board_returns_the_full_subtree_over_the_wire() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    restore_board_returns_the_full_subtree_over_the_wire_case(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_board_returns_the_full_subtree_over_the_wire_on_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    restore_board_returns_the_full_subtree_over_the_wire_case(state).await;
}

async fn archive_missing_board_returns_404_case(state: AppState) {
    let missing = Uuid::new_v4();
    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{missing}/archive"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_missing_board_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    archive_missing_board_returns_404_case(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_missing_board_returns_404_on_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    archive_missing_board_returns_404_case(state).await;
}

async fn restore_live_board_returns_404_case(state: AppState) {
    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
    }
    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/restore"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let json = json_of(response).await;
    assert_eq!(json["code"], "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_live_board_returns_404() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    restore_live_board_returns_404_case(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_live_board_returns_404_on_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    restore_live_board_returns_404_case(state).await;
}

async fn rearchive_board_succeeds_and_refreshes_archived_at_case(state: AppState) {
    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
    }

    let first = json_of(
        send(
            &state,
            "POST",
            &format!("/v1/boards/{board_id}/archive"),
            None,
        )
        .await,
    )
    .await;
    let second = json_of(
        send(
            &state,
            "POST",
            &format!("/v1/boards/{board_id}/archive"),
            None,
        )
        .await,
    )
    .await;

    let first_at: chrono::DateTime<chrono::Utc> =
        first["archived_at"].as_str().unwrap().parse().unwrap();
    let second_at: chrono::DateTime<chrono::Utc> =
        second["archived_at"].as_str().unwrap().parse().unwrap();
    assert!(second_at >= first_at);

    let list_json = json_of(send(&state, "GET", "/v1/archived-boards", None).await).await;
    let items = list_json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rearchive_board_succeeds_and_refreshes_archived_at() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    rearchive_board_succeeds_and_refreshes_archived_at_case(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rearchive_board_succeeds_and_refreshes_archived_at_on_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    rearchive_board_succeeds_and_refreshes_archived_at_case(state).await;
}

async fn list_archived_boards_route_reflects_archive_and_restore_case(state: AppState) {
    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
    }

    let empty = json_of(send(&state, "GET", "/v1/archived-boards", None).await).await;
    assert_eq!(empty["items"].as_array().unwrap().len(), 0);
    assert_eq!(empty["total"], 0);
    assert_eq!(empty["page"], 1);
    assert_eq!(empty["page_size"], 50);

    send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/archive"),
        None,
    )
    .await;

    let archived = json_of(send(&state, "GET", "/v1/archived-boards", None).await).await;
    let items = archived["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["entity_id"], board_id.to_string());
    assert!(items[0]["archived_at"].is_string());
    let keys: std::collections::BTreeSet<&str> = items[0]
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    assert_eq!(keys, ["entity_id", "archived_at"].into_iter().collect());

    send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/restore"),
        None,
    )
    .await;

    let restored = json_of(send(&state, "GET", "/v1/archived-boards", None).await).await;
    assert_eq!(restored["items"].as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_archived_boards_route_reflects_archive_and_restore() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    list_archived_boards_route_reflects_archive_and_restore_case(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_archived_boards_route_reflects_archive_and_restore_on_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    list_archived_boards_route_reflects_archive_and_restore_case(state).await;
}

async fn archive_and_restore_emit_board_updated_frames_case(state: AppState) {
    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
    }

    let mut rx = state.event_tx.subscribe();

    send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/archive"),
        None,
    )
    .await;
    let archive_frame = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out waiting for the archive frame")
        .expect("broadcast channel closed unexpectedly");
    assert_eq!(
        archive_frame.entity_type,
        Some(kanban_service::api::EntityType::Board)
    );
    assert_eq!(archive_frame.entity_id, Some(board_id));
    assert_eq!(
        archive_frame.kind,
        Some(kanban_service::api::ChangeKind::Updated)
    );
    assert_eq!(archive_frame.writer_instance_id, state.instance_id);

    send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/restore"),
        None,
    )
    .await;
    let restore_frame = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out waiting for the restore frame")
        .expect("broadcast channel closed unexpectedly");
    assert_eq!(
        restore_frame.entity_type,
        Some(kanban_service::api::EntityType::Board)
    );
    assert_eq!(restore_frame.entity_id, Some(board_id));
    assert_eq!(
        restore_frame.kind,
        Some(kanban_service::api::ChangeKind::Updated)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_and_restore_emit_board_updated_frames() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    archive_and_restore_emit_board_updated_frames_case(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_and_restore_emit_board_updated_frames_on_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    archive_and_restore_emit_board_updated_frames_case(state).await;
}

async fn archive_is_visible_to_subsequent_reads_case(state: AppState) {
    let board_id: Uuid;
    {
        let mut ctx = state.ctx.lock().await;
        board_id = ctx
            .create_board("Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
    }

    let before = json_of(send(&state, "GET", "/v1/boards", None).await).await;
    let before_items = before["items"].as_array().unwrap();
    assert!(before_items.iter().any(|b| b["id"] == board_id.to_string()));

    let archive_response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/archive"),
        None,
    )
    .await;
    assert_eq!(archive_response.status(), StatusCode::OK);

    let after = json_of(send(&state, "GET", "/v1/boards", None).await).await;
    let after_items = after["items"].as_array().unwrap();
    assert!(after_items.iter().all(|b| b["id"] != board_id.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_is_visible_to_subsequent_reads() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    archive_is_visible_to_subsequent_reads_case(state).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_is_visible_to_subsequent_reads_on_sqlite() {
    let dir = tempdir().unwrap();
    let state = make_sqlite_state(&dir.path().join("b.sqlite")).await;
    archive_is_visible_to_subsequent_reads_case(state).await;
}
