//! Service-tier integration tests for `KanbanContext::create_column_from_spec`
//! (Column factory epic slice D-core, KAN-793): the rich create funnel that
//! validates the `board_id` FK, resolves the optional client id (idempotent
//! PUT-create), enforces id uniqueness, applies the server-assigned append
//! `position`, and dispatches through `Column::create`.
//!
//! JSON backend only (the create path is backend-agnostic); a single SQLite
//! smoke test guards the relational backend.
use kanban_persistence_json::JsonFileStore;
use kanban_service::{
    json_backend::JsonDataStore, AppConfig, ColumnCreateOutcome, KanbanBackend, KanbanContext,
    KanbanOperations, NewColumn,
};
use std::sync::Arc;
use tempfile::tempdir;
use uuid::Uuid;

fn make_json_backend(path: &std::path::Path) -> Arc<dyn KanbanBackend> {
    Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))))
}

fn ctx_for(name: &str) -> (tempfile::TempDir, KanbanContext) {
    let dir = tempdir().unwrap();
    let path = dir.path().join(format!("{name}.json"));
    let ctx = KanbanContext::open_deferred(make_json_backend(&path), AppConfig::default());
    (dir, ctx)
}

fn board_id(ctx: &mut KanbanContext) -> Uuid {
    ctx.create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id
}

fn spec(board_id: Uuid, name: &str, wip_limit: Option<i32>) -> NewColumn {
    NewColumn {
        board_id,
        name: name.to_string(),
        wip_limit,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_column_missing_board_returns_not_found() {
    let (_dir, mut ctx) = ctx_for("missing_board");
    let missing = Uuid::new_v4();

    let err = ctx
        .create_column_from_spec(None, spec(missing, "To Do", None))
        .unwrap_err();

    assert!(err.is_not_found(), "expected NotFound, got: {err:?}");
    assert_eq!(ctx.list_columns(missing).unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_column_with_client_id_uses_it() {
    let (_dir, mut ctx) = ctx_for("client_id");
    let bid = board_id(&mut ctx);
    let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();

    let column = ctx
        .create_column_from_spec(Some(id), spec(bid, "To Do", None))
        .unwrap();

    assert_eq!(column.id, id);
    assert_eq!(ctx.get_column(id).unwrap().unwrap().id, id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_column_carries_wip_limit_from_spec() {
    let (_dir, mut ctx) = ctx_for("wip");
    let bid = board_id(&mut ctx);

    let column = ctx
        .create_column_from_spec(None, spec(bid, "WIP", Some(2)))
        .unwrap();

    // The spec wip_limit survives the funnel atomically (no follow-up read).
    assert_eq!(column.wip_limit, Some(2));
    assert_eq!(
        ctx.get_column(column.id).unwrap().unwrap().wip_limit,
        Some(2)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_column_mints_id_when_absent() {
    let (_dir, mut ctx) = ctx_for("mint");
    let bid = board_id(&mut ctx);

    let column = ctx
        .create_column_from_spec(None, spec(bid, "Minted", None))
        .unwrap();

    assert_ne!(column.id, Uuid::nil());
    assert!(ctx.get_column(column.id).unwrap().is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_column_duplicate_id_returns_conflict() {
    let (_dir, mut ctx) = ctx_for("dup");
    let bid = board_id(&mut ctx);
    let id = Uuid::new_v4();

    ctx.create_column_from_spec(Some(id), spec(bid, "Original", None))
        .unwrap();

    let err = ctx
        .create_column_from_spec(Some(id), spec(bid, "Collision", None))
        .unwrap_err();

    // KAN-770 dedicated variant → 409, not a 422 validation reuse.
    assert!(
        err.is_already_exists(),
        "expected AlreadyExists conflict, got: {err:?}"
    );
    // The original column is unchanged (collision rejected before any write).
    let existing = ctx.get_column(id).unwrap().unwrap();
    assert_eq!(existing.name, "Original");
    assert_eq!(ctx.list_columns(bid).unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_column_appends_position() {
    let (_dir, mut ctx) = ctx_for("append");
    let bid = board_id(&mut ctx);

    let first = ctx
        .create_column_from_spec(None, spec(bid, "First", None))
        .unwrap();
    let second = ctx
        .create_column_from_spec(None, spec(bid, "Second", None))
        .unwrap();

    // Server-assigned append: position is the prior column count, ignoring any
    // client wish (the spec carries no position field).
    assert_eq!(first.position, 0);
    assert_eq!(second.position, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_column_create_or_replace_is_idempotent() {
    let (_dir, mut ctx) = ctx_for("idempotent");
    let bid = board_id(&mut ctx);
    let id = Uuid::new_v4();

    let first = ctx
        .create_column_from_spec(Some(id), spec(bid, "Stable", Some(3)))
        .unwrap();

    // A second create with the SAME id is rejected as a conflict (idempotent
    // PUT-create at this tier means "no duplicate"); the state is unchanged.
    let err = ctx
        .create_column_from_spec(Some(id), spec(bid, "Stable", Some(3)))
        .unwrap_err();
    assert!(err.is_already_exists(), "got: {err:?}");

    let columns = ctx.list_columns(bid).unwrap();
    assert_eq!(columns.len(), 1);
    let only = &columns[0];
    assert_eq!(only.id, first.id);
    assert_eq!(only.name, "Stable");
    assert_eq!(only.wip_limit, Some(3));
    assert_eq!(only.position, first.position);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_column_shim_delegates_to_spec_path() {
    let (_dir, mut ctx) = ctx_for("shim");
    let bid = board_id(&mut ctx);

    // The thin board_id/name/position shim still works (no churn for callers).
    let column = ctx.create_column(bid, "Shimmed".to_string(), None).unwrap();

    let fetched = ctx.get_column(column.id).unwrap().unwrap();
    assert_eq!(fetched.name, "Shimmed");
    assert_eq!(fetched.board_id, bid);
    assert_eq!(fetched.position, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_reorder_column_still_updates_position() {
    let (_dir, mut ctx) = ctx_for("reorder");
    let bid = board_id(&mut ctx);
    let a = ctx
        .create_column_from_spec(None, spec(bid, "A", None))
        .unwrap();
    let b = ctx
        .create_column_from_spec(None, spec(bid, "B", None))
        .unwrap();
    assert_eq!(a.position, 0);
    assert_eq!(b.position, 1);

    let reordered = ctx.reorder_column(a.id, 5).unwrap();
    assert_eq!(reordered.position, 5);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_or_replace_column_creates_when_absent() {
    let (_dir, mut ctx) = ctx_for("cor_create");
    let bid = board_id(&mut ctx);
    let id = Uuid::new_v4();

    let ColumnCreateOutcome { column, created } = ctx
        .create_or_replace_column(id, spec(bid, "To Do", Some(2)))
        .unwrap();

    assert!(created, "absent id reports created");
    assert_eq!(column.id, id);
    assert_eq!(column.name, "To Do");
    assert_eq!(column.wip_limit, Some(2));
    assert_eq!(column.position, 0, "server-assigned append position");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_or_replace_column_replaces_when_present() {
    let (_dir, mut ctx) = ctx_for("cor_replace");
    let bid = board_id(&mut ctx);
    let id = Uuid::new_v4();

    ctx.create_or_replace_column(id, spec(bid, "Original", Some(5)))
        .unwrap();
    let position_before = ctx.get_column(id).unwrap().unwrap().position;

    let ColumnCreateOutcome { column, created } = ctx
        .create_or_replace_column(id, spec(bid, "Renamed", None))
        .unwrap();

    assert!(!created, "present id reports replace");
    assert_eq!(column.id, id, "id stable across replace");
    assert_eq!(column.name, "Renamed", "content replaced");
    assert_eq!(column.wip_limit, None, "omitted wip_limit cleared");
    assert_eq!(
        column.position, position_before,
        "server-managed position preserved across replace"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_or_replace_column_is_idempotent() {
    let (_dir, mut ctx) = ctx_for("cor_idempotent");
    let bid = board_id(&mut ctx);
    let id = Uuid::new_v4();

    ctx.create_or_replace_column(id, spec(bid, "X", None))
        .unwrap();
    ctx.create_or_replace_column(id, spec(bid, "X", None))
        .unwrap();

    assert_eq!(
        ctx.list_columns(bid).unwrap().len(),
        1,
        "PUT twice must not duplicate"
    );
}
