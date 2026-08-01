//! The archived-boards view has its own default sort (recency:
//! `ArchivedAt` descending), independent of whatever the live view's saved
//! `board_sort_field`/`board_sort_order` preference is set to.

use kanban_domain::{ArchivedFilter, BoardListFilter, BoardSortField, SortOrder};
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
use std::sync::Arc;
use tempfile::tempdir;

fn make_json_backend(path: &std::path::Path) -> Arc<dyn KanbanBackend> {
    Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))))
}

/// Setting the live view's saved sort preference to Name must not change the
/// archived view's default: it should still resolve to recency (`ArchivedAt`
/// descending), not `Name`.
#[tokio::test(flavor = "multi_thread")]
async fn test_archived_board_default_sort_ignores_live_sort_preference() {
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

    // Names chosen so name-ascending and recency-descending disagree: Apple
    // archived first (older), Zebra archived second (more recent).
    let apple = ctx.create_board("Apple".into(), None).unwrap();
    let zebra = ctx.create_board("Zebra".into(), None).unwrap();
    ctx.archive_board(apple.id).unwrap();
    ctx.archive_board(zebra.id).unwrap();

    // Setting the live sort to Name would, before the fix, also become the
    // archived view's default (since board_sort_default consulted the same
    // global app_config field for every archived selector).
    ctx.set_board_sort(BoardSortField::Name, SortOrder::Ascending)
        .unwrap();

    let archived = ctx
        .list_boards_filtered(BoardListFilter {
            archived: ArchivedFilter::ArchivedOnly,
            ..Default::default()
        })
        .unwrap();

    // Recency order: Zebra was archived after Apple, so it sorts first under
    // the archived-view's recency default — the opposite of name-ascending.
    assert_eq!(
        archived.iter().map(|b| b.name.clone()).collect::<Vec<_>>(),
        vec!["Zebra", "Apple"],
        "archived view must stay on its own recency default, unaffected by the live sort preference"
    );
}
