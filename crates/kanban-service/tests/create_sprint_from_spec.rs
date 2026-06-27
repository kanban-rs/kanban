//! Service-tier integration tests for `KanbanContext::create_sprint_from_spec`
//! (Sprint factory epic slice D-core, KAN-797): the create funnel that resolves
//! the optional client id, validates the `board_id` FK, mints `sprint_number` +
//! `name_index` from the owning Board, and dispatches through the frozen
//! `CreateSprint` command whose execute now builds via `Sprint::create`.
//!
//! JSON backend only (the create path is backend-agnostic); a single SQLite
//! smoke test guards the relational backend.
use kanban_domain::SprintStatus;
use kanban_persistence_json::JsonFileStore;
use kanban_service::{
    json_backend::JsonDataStore, AppConfig, KanbanBackend, KanbanContext, KanbanOperations,
};
use std::sync::Arc;
use tempfile::tempdir;
use uuid::Uuid;

fn make_json_backend(path: &std::path::Path) -> Arc<dyn KanbanBackend> {
    Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))))
}

fn ctx_with_board(path: &std::path::Path) -> (KanbanContext, Uuid) {
    let mut ctx = KanbanContext::open_deferred(make_json_backend(path), AppConfig::default());
    let board = ctx
        .create_board("Roadmap".to_string(), Some("KAN".to_string()))
        .unwrap();
    (ctx, board.id)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_sprint_mints_sprint_number_from_board_counter() {
    let dir = tempdir().unwrap();
    let (mut ctx, board_id) = ctx_with_board(&dir.path().join("num.json"));

    let first = ctx
        .create_sprint_from_spec(board_id, None, None, Some("SPR".to_string()), false)
        .unwrap();
    let second = ctx
        .create_sprint_from_spec(board_id, None, None, Some("SPR".to_string()), false)
        .unwrap();

    assert_eq!(first.sprint_number, 1);
    assert_eq!(second.sprint_number, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_sprint_with_explicit_name_allocates_name_index_from_board_pool() {
    let dir = tempdir().unwrap();
    let (mut ctx, board_id) = ctx_with_board(&dir.path().join("name.json"));

    let sprint = ctx
        .create_sprint_from_spec(
            board_id,
            None,
            Some("Alpha".to_string()),
            Some("SPR".to_string()),
            false,
        )
        .unwrap();

    let idx = sprint
        .name_index
        .expect("explicit name allocates a name_index");
    let board = ctx.get_board(board_id).unwrap().unwrap();
    assert_eq!(
        board.sprint_names.get(idx).map(String::as_str),
        Some("Alpha")
    );
    assert_eq!(sprint.get_name(&board), Some("Alpha"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_sprint_starts_in_planning_with_no_dates() {
    let dir = tempdir().unwrap();
    let (mut ctx, board_id) = ctx_with_board(&dir.path().join("planning.json"));

    let sprint = ctx
        .create_sprint_from_spec(board_id, None, None, None, false)
        .unwrap();

    assert_eq!(sprint.status, SprintStatus::Planning);
    assert_eq!(sprint.start_date, None);
    assert_eq!(sprint.end_date, None);
    assert_eq!(
        sprint.created_at, sprint.updated_at,
        "factory uses a single clock for both timestamps"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_sprint_with_nonexistent_board_returns_not_found() {
    let dir = tempdir().unwrap();
    let mut ctx = KanbanContext::open_deferred(
        make_json_backend(&dir.path().join("fk.json")),
        AppConfig::default(),
    );

    let err = ctx
        .create_sprint_from_spec(Uuid::new_v4(), None, None, None, false)
        .unwrap_err();

    assert!(err.is_not_found(), "expected NotFound, got: {err:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_sprint_with_client_supplied_id_uses_that_id() {
    let dir = tempdir().unwrap();
    let (mut ctx, board_id) = ctx_with_board(&dir.path().join("clientid.json"));

    let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let sprint = ctx
        .create_sprint_from_spec(board_id, Some(id), None, None, false)
        .unwrap();

    assert_eq!(sprint.id, id);
    assert_eq!(ctx.get_sprint(id).unwrap().unwrap().id, id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_sprint_with_colliding_client_id_returns_conflict() {
    let dir = tempdir().unwrap();
    let (mut ctx, board_id) = ctx_with_board(&dir.path().join("collide.json"));

    let id = Uuid::new_v4();
    ctx.create_sprint_from_spec(board_id, Some(id), None, None, false)
        .unwrap();

    let err = ctx
        .create_sprint_from_spec(board_id, Some(id), None, None, false)
        .unwrap_err();

    assert!(
        err.is_already_exists(),
        "expected AlreadyExists conflict, got: {err:?}"
    );
    // The collision was rejected before any second write.
    assert_eq!(ctx.list_sprints(board_id).unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_create_sprint_is_idempotent() {
    let dir = tempdir().unwrap();
    let (mut ctx, board_id) = ctx_with_board(&dir.path().join("put.json"));

    let id = Uuid::new_v4();
    let created = ctx
        .create_or_replace_sprint(board_id, id, None, Some("SPR".to_string()), false)
        .unwrap();
    assert!(created.created, "absent id must report created");
    assert_eq!(created.sprint.id, id);

    let replaced = ctx
        .create_or_replace_sprint(board_id, id, None, Some("SPR".to_string()), false)
        .unwrap();
    assert!(
        !replaced.created,
        "present id must report replace, not a duplicate create"
    );
    assert_eq!(replaced.sprint.id, id, "id is stable across replace");
    // Exactly one sprint: the second PUT replaced, it did not duplicate.
    assert_eq!(ctx.list_sprints(board_id).unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_sprint_bumps_board_counter_persisted_before_sprint() {
    let dir = tempdir().unwrap();
    let (mut ctx, board_id) = ctx_with_board(&dir.path().join("counter.json"));

    ctx.create_sprint_from_spec(board_id, None, None, Some("SPR".to_string()), false)
        .unwrap();

    // Board counter advanced (the dual-mint side effect persisted via upsert_board).
    let board = ctx.get_board(board_id).unwrap().unwrap();
    let next = ctx
        .create_sprint_from_spec(board_id, None, None, Some("SPR".to_string()), false)
        .unwrap();
    assert_eq!(
        next.sprint_number, 2,
        "second sprint sees the bumped counter; board={board:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_carry_over_still_requires_completed_source_and_planning_target() {
    let dir = tempdir().unwrap();
    let (mut ctx, board_id) = ctx_with_board(&dir.path().join("carry.json"));

    let source = ctx
        .create_sprint_from_spec(board_id, None, None, None, false)
        .unwrap();
    let target = ctx
        .create_sprint_from_spec(board_id, None, None, None, false)
        .unwrap();

    // Source is still Planning (not Completed/Cancelled) -> carry-over rejected.
    let err = ctx
        .carry_over_sprint_cards(source.id, target.id)
        .unwrap_err();
    assert!(
        err.is_validation(),
        "expected validation error, got: {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_sprint_from_spec_sqlite_smoke() {
    use kanban_service::sqlite_backend::SqliteBackend;

    let dir = tempdir().unwrap();
    let path = dir.path().join("smoke.db");
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(SqliteBackend::open(path.to_str().unwrap()).await.unwrap());
    let mut ctx = KanbanContext::open_deferred(backend, AppConfig::default());
    let board = ctx
        .create_board("B".to_string(), Some("KAN".to_string()))
        .unwrap();

    let sprint = ctx
        .create_sprint_from_spec(
            board.id,
            None,
            Some("Alpha".to_string()),
            Some("SPR".to_string()),
            false,
        )
        .unwrap();

    assert_eq!(sprint.sprint_number, 1);
    assert_eq!(sprint.status, SprintStatus::Planning);
    let fetched = ctx.get_sprint(sprint.id).unwrap().unwrap();
    assert_eq!(fetched.id, sprint.id);
}
