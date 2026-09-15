//! Service-tier integration tests for the free-function sprint-name
//! resolvers (`kanban_service::resolve_sprint_name` /
//! `resolve_sprint_names`): the `Board` lookup + `Sprint::get_name`
//! denormalisation happen once inside kanban-service, so callers get the
//! resolved name back without touching `Board` themselves.
use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{
    resolve_sprint_name, resolve_sprint_names, AppConfig, KanbanBackend, KanbanContext,
    KanbanOperations, Sprint,
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
async fn test_resolve_sprint_name_returns_the_owning_boards_pool_name() {
    let dir = tempdir().unwrap();
    let (mut ctx, board_id) = ctx_with_board(&dir.path().join("get.json"));

    let (sprint, _inv) = ctx
        .create_sprint_from_spec(
            board_id,
            None,
            Some("Alpha".to_string()),
            Some("SPR".to_string()),
            false,
        )
        .unwrap();

    let name = resolve_sprint_name(&ctx, &sprint).unwrap();
    assert_eq!(name, Some("Alpha".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_sprint_name_returns_none_when_sprint_has_no_name_index() {
    let dir = tempdir().unwrap();
    let (mut ctx, board_id) = ctx_with_board(&dir.path().join("no_name.json"));

    let (sprint, _inv) = ctx
        .create_sprint_from_spec(board_id, None, None, Some("SPR".to_string()), false)
        .unwrap();

    let name = resolve_sprint_name(&ctx, &sprint).unwrap();
    assert_eq!(name, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_sprint_name_missing_board_returns_not_found() {
    let dir = tempdir().unwrap();
    let (ctx, _board_id) = ctx_with_board(&dir.path().join("missing.json"));

    let missing_board_id = Uuid::new_v4();
    let sprint = Sprint::new(missing_board_id, 1, Some(0), None::<String>);

    let err = resolve_sprint_name(&ctx, &sprint).expect_err("board does not exist");
    assert!(err.is_not_found());
    assert!(err.to_string().contains(&missing_board_id.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_sprint_names_maps_each_sprint_to_its_own_pool_name() {
    let dir = tempdir().unwrap();
    let (mut ctx, board_id) = ctx_with_board(&dir.path().join("list.json"));

    let (_sprint, _inv) = ctx
        .create_sprint_from_spec(
            board_id,
            None,
            Some("Alpha".to_string()),
            Some("SPR".to_string()),
            false,
        )
        .unwrap();
    let (_sprint, _inv) = ctx
        .create_sprint_from_spec(board_id, None, None, Some("SPR".to_string()), false)
        .unwrap();
    let (_sprint, _inv) = ctx
        .create_sprint_from_spec(
            board_id,
            None,
            Some("Gamma".to_string()),
            Some("SPR".to_string()),
            false,
        )
        .unwrap();

    let sprints = ctx.list_sprints(board_id).unwrap();
    let names = resolve_sprint_names(&ctx, board_id, &sprints).unwrap();

    assert_eq!(
        names,
        vec![Some("Alpha".to_string()), None, Some("Gamma".to_string())]
    );
}
