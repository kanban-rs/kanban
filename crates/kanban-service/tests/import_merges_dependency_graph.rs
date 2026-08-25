//! Import must MERGE dependency edges into the destination workspace, not
//! replace the graph wholesale. `ImportEntities::execute` used to call
//! `set_graph`, which deletes every `spawns`/`blocks`/`relates` edge in the
//! destination before reinserting the imported ones, silently destroying any
//! edge among the destination's own cards.

use kanban_core::graph::Edge;
use kanban_domain::{CreateCardOptions, GraphOperations, KanbanOperations, RelatesKind, Severity};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;
use uuid::Uuid;

async fn open_json(path: &std::path::Path) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

async fn open_sqlite(path: &std::path::Path) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(SqliteBackend::open(path.to_str().unwrap()).await.unwrap());
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

/// A board with two cards linked by a spawns edge (parent -> child), a blocks
/// edge, and a relates edge among themselves.
fn seed_board_with_all_three_kinds(ctx: &mut KanbanContext) -> (Uuid, Uuid, Uuid, Uuid) {
    let board = ctx.create_board("Board".into(), None).unwrap();
    let column = ctx.create_column(board.id, "TODO".into(), None).unwrap();
    let a = ctx
        .create_card(
            board.id,
            column.id,
            "a".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let b = ctx
        .create_card(
            board.id,
            column.id,
            "b".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    let c = ctx
        .create_card(
            board.id,
            column.id,
            "c".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.attach_child(a.id, b.id).unwrap();
    ctx.block(a.id, c.id, Severity::High).unwrap();
    ctx.relate(b.id, c.id, RelatesKind::General).unwrap();
    (board.id, a.id, b.id, c.id)
}

fn seed_and_export(ctx: &mut KanbanContext) -> (Uuid, Uuid, Uuid, Uuid, String) {
    let (board_id, a, b, c) = seed_board_with_all_three_kinds(ctx);
    let exported = ctx.export_board(Some(board_id)).unwrap();
    (board_id, a, b, c, exported)
}

async fn assert_import_merges_all_three_edge_kinds(
    mut src: KanbanContext,
    mut dest: KanbanContext,
) {
    let (_src_board, src_a, src_b, src_c, exported) = seed_and_export(&mut src);

    let (dest_board_id, dest_a, dest_b, dest_c) = seed_board_with_all_three_kinds(&mut dest);

    dest.import_board(&exported).unwrap();

    let dest_relations = dest.list_relations_for_board(dest_board_id).unwrap();
    assert!(
        dest_relations
            .spawns
            .iter()
            .any(|e| e.source() == dest_a && e.target() == dest_b),
        "the destination's own spawns edge did not survive import"
    );
    assert!(
        dest_relations
            .blocks
            .iter()
            .any(|e| e.source() == dest_a && e.target() == dest_c),
        "the destination's own blocks edge did not survive import"
    );
    assert!(
        dest_relations
            .relates
            .iter()
            .any(|e| (e.source() == dest_b && e.target() == dest_c)
                || (e.source() == dest_c && e.target() == dest_b)),
        "the destination's own relates edge did not survive import"
    );

    let imported_board = dest
        .list_boards()
        .unwrap()
        .into_iter()
        .find(|b| b.id != dest_board_id)
        .expect("the imported board");
    let imported_relations = dest.list_relations_for_board(imported_board.id).unwrap();
    assert!(
        imported_relations
            .spawns
            .iter()
            .any(|e| e.source() == src_a && e.target() == src_b),
        "the imported board's own spawns edge did not arrive"
    );
    assert!(
        imported_relations
            .blocks
            .iter()
            .any(|e| e.source() == src_a && e.target() == src_c),
        "the imported board's own blocks edge did not arrive"
    );
    assert!(
        imported_relations
            .relates
            .iter()
            .any(|e| (e.source() == src_b && e.target() == src_c)
                || (e.source() == src_c && e.target() == src_b)),
        "the imported board's own relates edge did not arrive"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_json_import_merges_all_three_edge_kinds_without_discarding_destination_edges() {
    let dir = tempdir().unwrap();
    let src = open_json(&dir.path().join("src.json")).await;
    let dest = open_json(&dir.path().join("dest.json")).await;
    assert_import_merges_all_three_edge_kinds(src, dest).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_import_merges_all_three_edge_kinds_without_discarding_destination_edges() {
    let dir = tempdir().unwrap();
    let src = open_sqlite(&dir.path().join("src.db")).await;
    let dest = open_sqlite(&dir.path().join("dest.db")).await;
    assert_import_merges_all_three_edge_kinds(src, dest).await;
}

/// A legacy export written before graphs were scoped per board carries the
/// whole source workspace's graph. That must still be safe to import: the
/// destination's own edges must survive, and the legacy payload's edges among
/// its own cards must still land.
async fn assert_legacy_unscoped_export_is_safe_to_import(
    mut src: KanbanContext,
    mut dest: KanbanContext,
) {
    let (_board, src_a, src_b, _src_c) = seed_board_with_all_three_kinds(&mut src);
    // An export carrying every board's edges, not just one board's — the
    // shape a whole-workspace export or a pre-scoping legacy file has.
    let exported = src.export_board(None).unwrap();

    let (dest_board_id, dest_a, dest_b, _dest_c) = seed_board_with_all_three_kinds(&mut dest);

    dest.import_board(&exported).unwrap();

    let dest_relations = dest.list_relations_for_board(dest_board_id).unwrap();
    assert!(
        dest_relations
            .spawns
            .iter()
            .any(|e| e.source() == dest_a && e.target() == dest_b),
        "an unscoped import destroyed the destination's own edge"
    );

    let imported_board = dest
        .list_boards()
        .unwrap()
        .into_iter()
        .find(|b| b.id != dest_board_id)
        .expect("the imported board");
    let imported_relations = dest.list_relations_for_board(imported_board.id).unwrap();
    assert!(
        imported_relations
            .spawns
            .iter()
            .any(|e| e.source() == src_a && e.target() == src_b),
        "an unscoped import's own edges must still land"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_json_legacy_unscoped_export_is_safe_to_import() {
    let dir = tempdir().unwrap();
    let src = open_json(&dir.path().join("src.json")).await;
    let dest = open_json(&dir.path().join("dest.json")).await;
    assert_legacy_unscoped_export_is_safe_to_import(src, dest).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_legacy_unscoped_export_is_safe_to_import() {
    let dir = tempdir().unwrap();
    let src = open_sqlite(&dir.path().join("src.db")).await;
    let dest = open_sqlite(&dir.path().join("dest.db")).await;
    assert_legacy_unscoped_export_is_safe_to_import(src, dest).await;
}

/// `import_board` (the TUI/CLI entry point) intentionally clears undo history
/// rather than being undoable — `test_import_board_clears_history` in
/// `undo_redo.rs` pins that. `ImportEntities` is still undoable through its
/// other callers, which replay it via `KanbanContext::execute` directly (see
/// `test_import_entities_is_undoable`), so that is the path this test drives.
///
/// Undo must restore the destination's pre-import graph exactly: the
/// imported edges gone, the destination's own edges untouched.
async fn assert_undo_restores_pre_import_graph(mut src: KanbanContext, mut dest: KanbanContext) {
    use kanban_domain::commands::{BoardCommand, Command, ImportEntities};

    let (_src_board, src_a, src_b, src_c, exported) = seed_and_export(&mut src);
    let imported: kanban_domain::Snapshot = serde_json::from_str(&exported).unwrap();
    let (dest_board_id, dest_a, dest_b, dest_c) = seed_board_with_all_three_kinds(&mut dest);

    dest.execute(vec![Command::Board(BoardCommand::Import(ImportEntities {
        boards: imported.boards,
        columns: imported.columns,
        cards: imported.cards,
        archived_cards: imported.archived_cards,
        archived_boards: imported.archived_boards,
        sprints: imported.sprints,
        graph: Some(imported.graph),
        prefixes: imported.prefixes,
        default_sprint_prefix: None,
    }))])
    .unwrap();
    dest.undo().unwrap();

    let dest_relations = dest.list_relations_for_board(dest_board_id).unwrap();
    assert!(
        dest_relations
            .spawns
            .iter()
            .any(|e| e.source() == dest_a && e.target() == dest_b),
        "undo must leave the destination's own spawns edge in place"
    );
    assert!(
        dest_relations
            .blocks
            .iter()
            .any(|e| e.source() == dest_a && e.target() == dest_c),
        "undo must leave the destination's own blocks edge in place"
    );
    assert!(
        dest_relations
            .relates
            .iter()
            .any(|e| (e.source() == dest_b && e.target() == dest_c)
                || (e.source() == dest_c && e.target() == dest_b)),
        "undo must leave the destination's own relates edge in place"
    );

    let graph = dest.backend().get_graph().unwrap();
    assert!(
        !graph
            .spawns_edges()
            .iter()
            .any(|e| e.source() == src_a && e.target() == src_b),
        "undo must remove the imported spawns edge"
    );
    assert!(
        !graph
            .blocks_edges()
            .iter()
            .any(|e| e.source() == src_a && e.target() == src_c),
        "undo must remove the imported blocks edge"
    );
    assert!(
        !graph
            .relates_edges()
            .iter()
            .any(|e| (e.source() == src_b && e.target() == src_c)
                || (e.source() == src_c && e.target() == src_b)),
        "undo must remove the imported relates edge"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_json_undo_restores_pre_import_graph() {
    let dir = tempdir().unwrap();
    let src = open_json(&dir.path().join("src.json")).await;
    let dest = open_json(&dir.path().join("dest.json")).await;
    assert_undo_restores_pre_import_graph(src, dest).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_undo_restores_pre_import_graph() {
    let dir = tempdir().unwrap();
    let src = open_sqlite(&dir.path().join("src.db")).await;
    let dest = open_sqlite(&dir.path().join("dest.db")).await;
    assert_undo_restores_pre_import_graph(src, dest).await;
}

/// The merge and its undo must both survive a save/reload cycle, not just
/// hold in memory before the store is written.
async fn assert_merge_and_undo_survive_reload(backend_kind: BackendKind) {
    let dir = tempdir().unwrap();
    let src_path = path_for(&backend_kind, dir.path(), "src");
    let dest_path = path_for(&backend_kind, dir.path(), "dest");

    let mut src = open(&backend_kind, &src_path).await;
    let (_board, src_a, src_b, _src_c, exported) = seed_and_export(&mut src);

    let mut dest = open(&backend_kind, &dest_path).await;
    let (dest_board_id, dest_a, dest_b, _dest_c) = seed_board_with_all_three_kinds(&mut dest);

    dest.import_board(&exported).unwrap();
    dest.save().await.unwrap();
    drop(dest);

    let reopened = open(&backend_kind, &dest_path).await;
    let dest_relations = reopened.list_relations_for_board(dest_board_id).unwrap();
    assert!(
        dest_relations
            .spawns
            .iter()
            .any(|e| e.source() == dest_a && e.target() == dest_b),
        "the destination's own edge did not survive the merge and reload"
    );
    let imported_board = reopened
        .list_boards()
        .unwrap()
        .into_iter()
        .find(|b| b.id != dest_board_id)
        .expect("the imported board");
    let imported_relations = reopened
        .list_relations_for_board(imported_board.id)
        .unwrap();
    assert!(
        imported_relations
            .spawns
            .iter()
            .any(|e| e.source() == src_a && e.target() == src_b),
        "the imported edge did not survive the merge and reload"
    );
}

enum BackendKind {
    Json,
    Sqlite,
}

async fn open(backend: &BackendKind, path: &std::path::Path) -> KanbanContext {
    match backend {
        BackendKind::Json => open_json(path).await,
        BackendKind::Sqlite => open_sqlite(path).await,
    }
}

fn path_for(backend: &BackendKind, dir: &std::path::Path, stem: &str) -> std::path::PathBuf {
    match backend {
        BackendKind::Json => dir.join(format!("{stem}.json")),
        BackendKind::Sqlite => dir.join(format!("{stem}.sqlite")),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_json_merge_survives_reload() {
    assert_merge_and_undo_survive_reload(BackendKind::Json).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_merge_survives_reload() {
    assert_merge_and_undo_survive_reload(BackendKind::Sqlite).await;
}
