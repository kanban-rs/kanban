//! `apply_snapshot` is a trusted seam, but a hand-crafted snapshot whose board
//! carries completion ids that resolve to no column in the same snapshot must
//! not diverge per backend: JSON would happily store the dangling id while
//! SQLite's foreign key rejects the whole import. The seam prunes such ids so
//! both backends accept the snapshot and agree on the stored configuration.

use std::path::Path;
use std::sync::Arc;

use kanban_core::AppConfig;
use kanban_domain::{Board, Column, DependencyGraph, KanbanOperations, Snapshot};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::{KanbanBackend, KanbanContext};
use tempfile::TempDir;
use uuid::Uuid;

async fn open_sqlite(path: &Path) -> KanbanContext {
    let backend = SqliteBackend::open(path.to_str().unwrap()).await.unwrap();
    KanbanContext::open(
        Arc::new(backend) as Arc<dyn KanbanBackend>,
        AppConfig::default(),
    )
    .await
    .unwrap()
}

fn open_json(path: &Path) -> KanbanContext {
    let backend =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path)))) as Arc<dyn KanbanBackend>;
    KanbanContext::open_deferred(backend, AppConfig::default())
}

fn snapshot_with_dangling_completion_id() -> (Snapshot, Uuid, Uuid) {
    let mut board = Board::new("B", None::<String>);
    let live = Column::new(board.id, "Done", 0);
    let dangling = Uuid::new_v4();
    board.update_completion_column_ids(vec![dangling, live.id]);
    let live_id = live.id;
    let board_id = board.id;
    let snapshot = Snapshot::from_data(
        vec![board],
        vec![live],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        DependencyGraph::default(),
    );
    (snapshot, board_id, live_id)
}

async fn assert_sanitized(ctx: KanbanContext, label: &str) {
    let (snapshot, board_id, live_id) = snapshot_with_dangling_completion_id();

    ctx.apply_snapshot(snapshot)
        .unwrap_or_else(|e| panic!("{label}: apply_snapshot must accept and sanitize, got: {e}"));

    let board = ctx.get_board(board_id).unwrap().expect("board imported");
    assert_eq!(
        board.completion_column_ids,
        vec![live_id],
        "{label}: dangling completion ids must be pruned, live ones kept in order"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_apply_snapshot_prunes_dangling_completion_ids_json() {
    let dir = TempDir::new().unwrap();
    assert_sanitized(open_json(&dir.path().join("s.json")), "json").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_apply_snapshot_prunes_dangling_completion_ids_sqlite() {
    let dir = TempDir::new().unwrap();
    assert_sanitized(open_sqlite(&dir.path().join("s.db")).await, "sqlite").await;
}
