//! Single-board export must scope the dependency-graph edges it carries to
//! the exported board's own cards, not the whole workspace graph.

use kanban_domain::{CreateCardOptions, GraphOperations, KanbanOperations, KanbanResult};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;

async fn open_json(path: &std::path::Path) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

fn new_board_with_two_cards(
    ctx: &mut KanbanContext,
    name: &str,
) -> KanbanResult<(uuid::Uuid, uuid::Uuid, uuid::Uuid)> {
    let board = ctx.create_board(name.into(), None)?;
    let column = ctx.create_column(board.id, "TODO".into(), None)?;
    let a = ctx.create_card(
        board.id,
        column.id,
        "A".into(),
        CreateCardOptions::default(),
    )?;
    let b = ctx.create_card(
        board.id,
        column.id,
        "B".into(),
        CreateCardOptions::default(),
    )?;
    Ok((board.id, a.id, b.id))
}

fn spawns_edges(export: &str) -> Vec<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(export).unwrap();
    value["graph"]["spawns"]["edges"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn has_edge(edges: &[serde_json::Value], source: uuid::Uuid, target: uuid::Uuid) -> bool {
    edges.iter().any(|e| {
        e["source"] == serde_json::Value::String(source.to_string())
            && e["target"] == serde_json::Value::String(target.to_string())
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn test_single_board_export_omits_other_boards_edges() {
    let dir = tempdir().unwrap();
    let mut ctx = open_json(&dir.path().join("test.json")).await;

    let (board_a, a1, a2) = new_board_with_two_cards(&mut ctx, "Board A").unwrap();
    let (_board_b, b1, b2) = new_board_with_two_cards(&mut ctx, "Board B").unwrap();

    ctx.attach_child(a1, a2).unwrap();
    ctx.attach_child(b1, b2).unwrap();

    let exported = ctx.export_board(Some(board_a)).unwrap();
    let edges = spawns_edges(&exported);

    assert!(
        has_edge(&edges, a1, a2),
        "board A's own edge must be present"
    );
    assert!(
        !has_edge(&edges, b1, b2),
        "board B's edge must not leak into board A's export"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_single_board_export_drops_cross_board_edge() {
    let dir = tempdir().unwrap();
    let mut ctx = open_json(&dir.path().join("test.json")).await;

    let (board_a, a1, _a2) = new_board_with_two_cards(&mut ctx, "Board A").unwrap();
    let (_board_b, _b1, b2) = new_board_with_two_cards(&mut ctx, "Board B").unwrap();

    ctx.attach_child(a1, b2).unwrap();

    let exported = ctx.export_board(Some(board_a)).unwrap();
    let edges = spawns_edges(&exported);

    assert!(
        !has_edge(&edges, a1, b2),
        "an edge with only one endpoint on the exported board must be dropped"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_single_board_export_keeps_its_own_archived_edges() {
    let dir = tempdir().unwrap();
    let mut ctx = open_json(&dir.path().join("test.json")).await;

    let (board_a, a1, a2) = new_board_with_two_cards(&mut ctx, "Board A").unwrap();
    ctx.attach_child(a1, a2).unwrap();
    ctx.archive_card(a1).unwrap();
    ctx.archive_card(a2).unwrap();

    let exported = ctx.export_board(Some(board_a)).unwrap();
    let edges = spawns_edges(&exported);

    assert!(
        has_edge(&edges, a1, a2),
        "an archived edge between two archived cards on the exported board must survive"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_full_workspace_export_carries_every_edge() {
    let dir = tempdir().unwrap();
    let mut ctx = open_json(&dir.path().join("test.json")).await;

    let (_board_a, a1, a2) = new_board_with_two_cards(&mut ctx, "Board A").unwrap();
    let (_board_b, b1, b2) = new_board_with_two_cards(&mut ctx, "Board B").unwrap();

    ctx.attach_child(a1, a2).unwrap();
    ctx.attach_child(b1, b2).unwrap();

    let exported = ctx.export_board(None).unwrap();
    let edges = spawns_edges(&exported);

    assert!(has_edge(&edges, a1, a2), "board A's edge must be present");
    assert!(has_edge(&edges, b1, b2), "board B's edge must be present");
}
