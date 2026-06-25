//! Service-tier integration tests for `KanbanContext::create_board_from_spec`
//! (Board factory epic slice D-core, KAN-791): the rich create funnel that
//! resolves the optional client id, mints `now`, applies server-managed
//! `position`, and dispatches through `Board::create` via the import path.
//!
//! JSON backend only (the create path is backend-agnostic); a single SQLite
//! smoke test guards the relational backend.
use kanban_domain::{SortField, SortOrder, TaskListView};
use kanban_persistence_json::JsonFileStore;
use kanban_service::{
    json_backend::JsonDataStore, AppConfig, KanbanBackend, KanbanContext, KanbanOperations,
    NewBoard,
};
use std::sync::Arc;
use tempfile::tempdir;
use uuid::Uuid;

fn make_json_backend(path: &std::path::Path) -> Arc<dyn KanbanBackend> {
    Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))))
}

fn full_spec(name: &str) -> NewBoard {
    NewBoard {
        name: name.to_string(),
        description: Some("a description".to_string()),
        sprint_prefix: Some("SPR".to_string()),
        card_prefix: Some("KAN".to_string()),
        task_sort_field: Some(SortField::Priority),
        task_sort_order: Some(SortOrder::Descending),
        sprint_duration_days: Some(21),
        task_list_view: Some(TaskListView::GroupedByColumn),
        completion_column_id: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_board_from_spec_applies_all_content_fields() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("spec.json");
    let mut ctx = KanbanContext::open_deferred(make_json_backend(&path), AppConfig::default());

    let board = ctx
        .create_board_from_spec(None, full_spec("Roadmap"))
        .unwrap();

    let fetched = ctx.get_board(board.id).unwrap().unwrap();
    assert_eq!(fetched.name, "Roadmap");
    assert_eq!(fetched.description, Some("a description".to_string()));
    assert_eq!(fetched.sprint_prefix, Some("SPR".to_string()));
    assert_eq!(fetched.card_prefix, Some("KAN".to_string()));
    assert_eq!(fetched.task_sort_field, SortField::Priority);
    assert_eq!(fetched.task_sort_order, SortOrder::Descending);
    assert_eq!(fetched.sprint_duration_days, Some(21));
    assert_eq!(fetched.task_list_view, TaskListView::GroupedByColumn);
    // Server-managed: counters minted, not accepted; position == prior list len (0).
    assert_eq!(fetched.card_counter, 1);
    assert_eq!(fetched.next_sprint_number, 1);
    assert_eq!(fetched.position, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_board_from_spec_position_is_prior_list_len() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("pos.json");
    let mut ctx = KanbanContext::open_deferred(make_json_backend(&path), AppConfig::default());

    let first = ctx
        .create_board_from_spec(None, full_spec("First"))
        .unwrap();
    let second = ctx
        .create_board_from_spec(None, full_spec("Second"))
        .unwrap();

    assert_eq!(first.position, 0);
    assert_eq!(second.position, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_board_from_spec_mints_id_when_absent() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mint.json");
    let mut ctx = KanbanContext::open_deferred(make_json_backend(&path), AppConfig::default());

    let board = ctx
        .create_board_from_spec(None, full_spec("Minted"))
        .unwrap();

    assert_ne!(board.id, Uuid::nil());
    assert!(ctx.get_board(board.id).unwrap().is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_board_from_spec_uses_client_supplied_id() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("client.json");
    let mut ctx = KanbanContext::open_deferred(make_json_backend(&path), AppConfig::default());

    let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let board = ctx
        .create_board_from_spec(Some(id), full_spec("Pinned"))
        .unwrap();

    assert_eq!(board.id, id);
    assert_eq!(ctx.get_board(id).unwrap().unwrap().id, id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_board_with_duplicate_client_id_returns_conflict() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("dup.json");
    let mut ctx = KanbanContext::open_deferred(make_json_backend(&path), AppConfig::default());

    let id = Uuid::new_v4();
    ctx.create_board_from_spec(Some(id), full_spec("Original"))
        .unwrap();

    let err = ctx
        .create_board_from_spec(Some(id), full_spec("Collision"))
        .unwrap_err();

    // KAN-770 dedicated variant → 409, not a 422 validation reuse.
    assert!(
        err.is_already_exists(),
        "expected AlreadyExists conflict, got: {err:?}"
    );
    // The original board is unchanged (collision rejected before any write).
    let existing = ctx.get_board(id).unwrap().unwrap();
    assert_eq!(existing.name, "Original");
    assert_eq!(ctx.list_boards().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_board_shim_delegates_to_spec_path() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("shim.json");
    let mut ctx = KanbanContext::open_deferred(make_json_backend(&path), AppConfig::default());

    // The thin name/card_prefix shim still works (no churn for ~40 callers).
    let board = ctx
        .create_board("Shimmed".to_string(), Some("SHM".to_string()))
        .unwrap();

    let fetched = ctx.get_board(board.id).unwrap().unwrap();
    assert_eq!(fetched.name, "Shimmed");
    assert_eq!(fetched.card_prefix, Some("SHM".to_string()));
    assert_eq!(fetched.card_counter, 1);
    assert_eq!(fetched.position, 0);
}
