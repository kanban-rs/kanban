//! Service-tier integration tests for the name-resolving sprint accessors
//! (`KanbanContext::get_sprint_with_name` / `list_sprints_with_names`): the
//! `Board` lookup + `Sprint::get_name` denormalisation happen once inside
//! kanban-service, so callers get the resolved name back without touching
//! `Board` themselves.
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
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
async fn test_get_sprint_with_name_resolves_denormalised_name() {
    let dir = tempdir().unwrap();
    let (mut ctx, board_id) = ctx_with_board(&dir.path().join("get.json"));

    let sprint = ctx
        .create_sprint_from_spec(
            board_id,
            None,
            Some("Alpha".to_string()),
            Some("SPR".to_string()),
            false,
        )
        .unwrap();

    let (resolved, name) = ctx.get_sprint_with_name(sprint.id).unwrap().unwrap();
    assert_eq!(resolved.id, sprint.id);
    assert_eq!(name, Some("Alpha".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_sprint_with_name_returns_none_for_missing_sprint() {
    let dir = tempdir().unwrap();
    let (ctx, _board_id) = ctx_with_board(&dir.path().join("missing.json"));

    let result = ctx.get_sprint_with_name(Uuid::new_v4()).unwrap();
    assert!(result.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_sprints_with_names_resolves_every_sprint_in_board() {
    let dir = tempdir().unwrap();
    let (mut ctx, board_id) = ctx_with_board(&dir.path().join("list.json"));

    ctx.create_sprint_from_spec(
        board_id,
        None,
        Some("Alpha".to_string()),
        Some("SPR".to_string()),
        false,
    )
    .unwrap();
    ctx.create_sprint_from_spec(
        board_id,
        None,
        Some("Beta".to_string()),
        Some("SPR".to_string()),
        false,
    )
    .unwrap();

    let mut names: Vec<Option<String>> = ctx
        .list_sprints_with_names(board_id)
        .unwrap()
        .into_iter()
        .map(|(_, name)| name)
        .collect();
    names.sort();

    assert_eq!(
        names,
        vec![Some("Alpha".to_string()), Some("Beta".to_string())]
    );
}
