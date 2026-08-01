//! Export/import round-trip tests for archival data (KAN-909).
//!
//! Proves that `BoardImporter::convert_snapshot_to_export` correctly carries
//! `archived_boards` markers and archived-card live rows (incl. dangling-column
//! cards) through a real file round-trip on both JSON and SQLite backends.
//!
//! The dependency graph is NOT part of `AllBoardsExport`; tests assert edges
//! are ABSENT after import to encode the known export limitation.

use kanban_domain::{
    archival::ArchivedEntity,
    export::{BoardExporter, BoardImporter},
    DependencyGraph, GraphOperations, KanbanOperations, KanbanResult, Severity, Snapshot,
};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;

// ── helpers ──────────────────────────────────────────────────────────────────

async fn open_json_ctx(path: &std::path::Path) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

async fn open_sqlite_ctx(path: &std::path::Path) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(SqliteBackend::open(path.to_str().unwrap()).await.unwrap());
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

#[derive(Clone, Copy)]
struct SeedIds {
    live_board_id: uuid::Uuid,
    archived_board_id: uuid::Uuid,
    col_id: uuid::Uuid,
    live_card_id: uuid::Uuid,
    archived_card_id: uuid::Uuid,
    sprint_id: uuid::Uuid,
}

/// Seed a non-trivial archival graph. Returns ids for later assertions.
/// Seeds:
/// - board B1 (live) with 1 column, 1 live card, 1 archived card, 1 sprint
/// - board B2 (archived) so `archived_boards` is non-empty
/// - a `blocks` edge between two live cards on B1
async fn seed_archival_graph(ctx: &mut KanbanContext) -> SeedIds {
    let b1 = ctx.create_board("Live Board".into(), None).unwrap();
    let col = ctx.create_column(b1.id, "Todo".into(), None).unwrap();
    let live_card = ctx
        .create_card(b1.id, col.id, "Live Card".into(), Default::default())
        .unwrap();
    let arch_card = ctx
        .create_card(b1.id, col.id, "Archived Card".into(), Default::default())
        .unwrap();
    let sprint = ctx.create_sprint(b1.id, Some("S".into()), None).unwrap();
    ctx.archive_card(arch_card.id).unwrap();
    // blocks edge between live cards (one of which is now archived)
    ctx.block(live_card.id, arch_card.id, Severity::default())
        .unwrap();

    let b2 = ctx.create_board("Archived Board".into(), None).unwrap();
    ctx.archive_board(b2.id).unwrap();

    SeedIds {
        live_board_id: b1.id,
        archived_board_id: b2.id,
        col_id: col.id,
        live_card_id: live_card.id,
        archived_card_id: arch_card.id,
        sprint_id: sprint.id,
    }
}

/// Assert the full archival graph is present in `ctx` after import.
fn assert_full_graph(ctx: &KanbanContext, ids: &SeedIds) -> KanbanResult<()> {
    // Live board present in live list
    let live_boards = ctx.boards()?;
    assert!(
        live_boards.iter().any(|b| b.id == ids.live_board_id),
        "live board must appear in ctx.boards()"
    );

    // archived_boards marker present
    let archived_boards = ctx.list_archived_boards()?;
    assert_eq!(
        archived_boards.len(),
        1,
        "exactly one archived_boards marker"
    );
    assert_eq!(
        archived_boards[0].entity_id(),
        ids.archived_board_id,
        "archived_boards marker entity_id"
    );

    // Archived board HEAD reachable from raw snapshot (apply_snapshot stores it)
    let snap = ctx.snapshot()?;
    assert!(
        snap.boards.iter().any(|b| b.id == ids.archived_board_id),
        "archived board head must be in snapshot.boards"
    );

    // Column present in live view
    let columns = ctx.list_all_columns()?;
    assert!(
        columns.iter().any(|c| c.id == ids.col_id),
        "column must be present"
    );

    // Live card present
    let live_cards = ctx.list_all_cards()?;
    assert!(
        live_cards.iter().any(|c| c.id == ids.live_card_id),
        "live card must be present in list_all_cards"
    );

    // Archived card live row reachable by id
    // (list_all_cards is live-scoped and excludes archived cards)
    let arch_row = ctx.get_card(ids.archived_card_id)?;
    assert!(
        arch_row.is_some(),
        "archived card live row must be reachable via get_card (unfiltered)"
    );

    // archived_cards marker present with correct board_id
    let archived_cards = ctx.list_archived_cards_by_board(ids.live_board_id)?;
    assert_eq!(archived_cards.len(), 1, "archived_cards marker count");
    assert_eq!(
        archived_cards[0].entity_id(),
        ids.archived_card_id,
        "archived_cards marker entity_id"
    );
    assert_eq!(
        archived_cards[0].context.board_id, ids.live_board_id,
        "archived_cards marker board_id"
    );

    // Sprint present
    let sprints = ctx.list_all_sprints()?;
    assert!(
        sprints.iter().any(|s| s.id == ids.sprint_id),
        "sprint must be present"
    );

    // Dependency graph NOT exported — blocks edge must be absent
    let graph = ctx.graph()?;
    assert!(
        graph.blocks_edges().is_empty(),
        "blocks edge must be absent after import (graph is not part of AllBoardsExport)"
    );

    Ok(())
}

/// Export the full snapshot via `convert_snapshot_to_export` + `export_to_file`.
/// This includes archived board heads (snapshot.boards has all board heads) and
/// archived_boards markers, and correctly handles dangling-column archived cards.
fn export_snapshot_to_json(ctx: &KanbanContext, export_path: &str) -> KanbanResult<()> {
    let snapshot = ctx.snapshot()?;
    let export = BoardImporter::convert_snapshot_to_export(snapshot);
    BoardExporter::export_to_file(&export, export_path)
        .map_err(|e| kanban_domain::KanbanError::Internal(format!("export_to_file failed: {e}")))
}

/// Import from `export_path` and apply to `ctx` via `apply_snapshot`.
fn import_into_context(ctx: &KanbanContext, export_path: &str) -> KanbanResult<()> {
    let import = BoardImporter::import_from_file(export_path).map_err(|e| {
        kanban_domain::KanbanError::Internal(format!("import_from_file failed: {e}"))
    })?;
    let entities = BoardImporter::extract_entities(import);
    let snapshot = Snapshot {
        archived_boards: entities.archived_boards,
        boards: entities.boards,
        columns: entities.columns,
        cards: entities.cards,
        archived_cards: entities.archived_cards,
        sprints: entities.sprints,
        graph: DependencyGraph::default(),
    };
    ctx.apply_snapshot(snapshot)
}

// ── JSON backend test ─────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn test_export_import_round_trip_full_archival_graph_json() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let source_path = dir.path().join("source.json");
    let export_path = dir.path().join("export.json").to_string_lossy().to_string();
    let dest_path = dir.path().join("dest.json");

    // Seed
    let ids = {
        let mut ctx = open_json_ctx(&source_path).await;
        let ids = seed_archival_graph(&mut ctx).await;
        ctx.save().await?;
        ids
    };

    // Export from source (fresh context to test reload-from-disk)
    {
        let ctx = open_json_ctx(&source_path).await;
        export_snapshot_to_json(&ctx, &export_path)?;
    }

    // Import into fresh destination and assert
    {
        let ctx = open_json_ctx(&dest_path).await;
        import_into_context(&ctx, &export_path)?;
        ctx.save().await?;
        assert_full_graph(&ctx, &ids)?;
    }

    // Reload from disk and re-assert durability
    {
        let ctx = open_json_ctx(&dest_path).await;
        assert_full_graph(&ctx, &ids)?;
    }

    Ok(())
}

// ── SQLite backend test ───────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn test_export_import_round_trip_full_archival_graph_sqlite() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let source_path = dir.path().join("source.sqlite");
    let export_path = dir.path().join("export.json").to_string_lossy().to_string();
    let dest_path = dir.path().join("dest.sqlite");

    // Seed
    let ids = {
        let mut ctx = open_sqlite_ctx(&source_path).await;
        let ids = seed_archival_graph(&mut ctx).await;
        ctx.save().await?;
        ids
    };

    // Export from source (fresh context to test reload-from-disk)
    {
        let ctx = open_sqlite_ctx(&source_path).await;
        export_snapshot_to_json(&ctx, &export_path)?;
    }

    // Import into fresh destination and assert
    {
        let ctx = open_sqlite_ctx(&dest_path).await;
        import_into_context(&ctx, &export_path)?;
        ctx.save().await?;
        assert_full_graph(&ctx, &ids)?;
    }

    // Reload from disk and re-assert durability
    {
        let ctx = open_sqlite_ctx(&dest_path).await;
        assert_full_graph(&ctx, &ids)?;
    }

    Ok(())
}
