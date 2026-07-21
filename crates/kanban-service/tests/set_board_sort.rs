//! Integration tests for `KanbanContext::set_board_sort` (KAN-952 / BSF-R3).
//!
//! `set_board_sort` is the shared entry point CLI (R4) and MCP (R5) call to
//! persist a board-sort preference. It must:
//!   1. persist to disk FIRST (via `kanban_service::config::save`) and only
//!      mutate the held `app_config` in place on save success, and
//!   2. NOT rebuild the context (no `open_deferred`), so the session_id and
//!      per-session undo history survive the change.

use kanban_domain::{BoardSortField, SortOrder};
use kanban_persistence_json::JsonFileStore;
use kanban_service::{
    json_backend::JsonDataStore, AppConfig, KanbanBackend, KanbanContext, KanbanOperations,
};
use std::sync::Arc;
use tempfile::tempdir;

fn make_json_backend(path: &std::path::Path) -> Arc<dyn KanbanBackend> {
    Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))))
}

/// Persisting a board sort must NOT rebuild the context: the session_id and the
/// per-session undo history are preserved, and the in-memory app_config reflects
/// the new (canonical) strings while the on-disk config file is written.
#[tokio::test(flavor = "multi_thread")]
async fn test_set_board_sort_persists_and_preserves_session() {
    let dir = tempdir().unwrap();
    let store_path = dir.path().join("board.json");
    let config_path = dir.path().join("config.toml");

    let config = AppConfig {
        configuration_location: Some(config_path.to_string_lossy().into_owned()),
        ..Default::default()
    };
    let mut ctx = KanbanContext::open(make_json_backend(&store_path), config)
        .await
        .unwrap();

    // Build some undo history so we can prove it survives the config write.
    ctx.create_board("Alpha".into(), None).unwrap();
    let session_before = ctx.session_id();
    let undo_depth_before = ctx.undo_depth();
    assert!(undo_depth_before > 0, "seeded undo history precondition");

    ctx.set_board_sort(BoardSortField::Name, SortOrder::Descending)
        .expect("set_board_sort persists successfully");

    // Session identity and undo history are untouched (no context rebuild).
    assert_eq!(
        ctx.session_id(),
        session_before,
        "session_id preserved across set_board_sort"
    );
    assert_eq!(
        ctx.undo_depth(),
        undo_depth_before,
        "undo history preserved across set_board_sort"
    );

    // In-memory app_config carries the new canonical strings (via Display).
    assert_eq!(
        ctx.app_config().board_sort_field.as_deref(),
        Some("name"),
        "field written canonically"
    );
    assert_eq!(
        ctx.app_config().board_sort_order.as_deref(),
        Some("descending"),
        "order written canonically"
    );

    // The config file was actually written and round-trips the values.
    let written = std::fs::read_to_string(&config_path).unwrap();
    assert!(
        written.contains("name") && written.contains("descending"),
        "config file persisted the new sort: {written}"
    );
}

/// If the disk write fails (unwritable config path), `set_board_sort` returns an
/// Err and leaves the in-memory app_config completely unchanged — persist-first
/// ordering means no half-applied state.
#[tokio::test(flavor = "multi_thread")]
async fn test_set_board_sort_save_failure_leaves_config_unchanged() {
    let dir = tempdir().unwrap();
    let store_path = dir.path().join("board.json");

    // A config path whose parent is a FILE, so create_dir_all + write both fail.
    let blocker = dir.path().join("not_a_dir");
    std::fs::write(&blocker, b"x").unwrap();
    let bad_config_path = blocker.join("child").join("config.toml");

    let config = AppConfig {
        configuration_location: Some(bad_config_path.to_string_lossy().into_owned()),
        board_sort_field: Some("position".into()),
        board_sort_order: Some("ascending".into()),
        ..Default::default()
    };
    let mut ctx = KanbanContext::open(make_json_backend(&store_path), config)
        .await
        .unwrap();

    let field_before = ctx.app_config().board_sort_field.clone();
    let order_before = ctx.app_config().board_sort_order.clone();

    let result = ctx.set_board_sort(BoardSortField::Name, SortOrder::Descending);
    assert!(result.is_err(), "save to unwritable path must error");

    assert_eq!(
        ctx.app_config().board_sort_field,
        field_before,
        "app_config field unchanged after failed save"
    );
    assert_eq!(
        ctx.app_config().board_sort_order,
        order_before,
        "app_config order unchanged after failed save"
    );
}
