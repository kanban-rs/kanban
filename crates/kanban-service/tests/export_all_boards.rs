//! `KanbanContext::export_all_boards` must compose the same `AllBoardsExport`
//! as converting a full backend snapshot, atomically, without exposing the
//! dependency graph and while carrying dangling-column archived cards.

use kanban_domain::export::BoardImporter;
use kanban_domain::{CreateCardOptions, GraphOperations, KanbanOperations, Severity};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{read_full_snapshot, AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;

async fn open_json(path: &std::path::Path) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_all_boards_matches_the_snapshot_based_composition() {
    let dir = tempdir().unwrap();
    let mut ctx = open_json(&dir.path().join("test.json")).await;

    let board = ctx.create_board("Board".into(), None).unwrap();
    let column = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();
    let live = ctx
        .create_card(
            board.id,
            column.id,
            "Live".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let arch = ctx
        .create_card(
            board.id,
            column.id,
            "Arch".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.assign_card_to_sprint(live.id, sprint.id).unwrap();
    ctx.block(live.id, arch.id, Severity::High).unwrap();
    ctx.archive_card(arch.id).unwrap();

    let other_board = ctx.create_board("Other".into(), None).unwrap();
    let other_column = ctx
        .create_column(other_board.id, "Todo".into(), None)
        .unwrap();
    ctx.create_sprint(other_board.id, None, None).unwrap();
    ctx.create_card(
        other_board.id,
        other_column.id,
        "Other card".into(),
        CreateCardOptions::default(),
    )
    .unwrap();
    ctx.archive_board(other_board.id).unwrap();

    let composed = ctx.export_all_boards().unwrap();
    let expected =
        BoardImporter::convert_snapshot_to_export(read_full_snapshot(ctx.data_store()).unwrap());

    assert_eq!(
        serde_json::to_value(&composed).unwrap(),
        serde_json::to_value(&expected).unwrap(),
        "export_all_boards must match converting the whole-store snapshot"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_all_boards_drops_the_dependency_graph() {
    let dir = tempdir().unwrap();
    let mut ctx = open_json(&dir.path().join("test.json")).await;

    let board = ctx.create_board("Board".into(), None).unwrap();
    let column = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let a = ctx
        .create_card(
            board.id,
            column.id,
            "A".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let b = ctx
        .create_card(
            board.id,
            column.id,
            "B".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.block(a.id, b.id, Severity::High).unwrap();

    let export = ctx.export_all_boards().unwrap();
    let value = serde_json::to_value(&export).unwrap();

    assert!(
        value.get("graph").is_none(),
        "AllBoardsExport carries no graph field by construction (the inverse of \
         transfer_state_to's contract); this pins that shape rather than \
         asserting an edge was filtered out"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_all_boards_carries_a_dangling_column_archived_card() {
    let dir = tempdir().unwrap();
    let mut ctx = open_json(&dir.path().join("test.json")).await;

    let board = ctx.create_board("Board".into(), None).unwrap();
    let column = ctx.create_column(board.id, "Todo".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.archive_card(card.id).unwrap();
    ctx.delete_column(column.id).unwrap();

    let export = ctx.export_all_boards().unwrap();
    let board_export = export
        .boards
        .iter()
        .find(|b| b.board.id == board.id)
        .expect("board must be present");

    assert!(
        board_export.cards.iter().any(|c| c.id == card.id),
        "an archived card whose column was deleted must still carry its live row"
    );
}
