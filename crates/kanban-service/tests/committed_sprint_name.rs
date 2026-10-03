#![cfg(feature = "test-helpers")]

use std::sync::Arc;

use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::test_helpers::FaultInjectingBackend;
use kanban_service::{
    resolve_committed_sprint_name, resolve_sprint_name, AppConfig, KanbanBackend, KanbanContext,
    KanbanOperations, Sprint,
};
use tempfile::tempdir;

fn seeded_ctx(path: &std::path::Path) -> (KanbanContext, Arc<FaultInjectingBackend>, Sprint) {
    let inner: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    let wrapper = Arc::new(FaultInjectingBackend::new(inner));
    let mut ctx = KanbanContext::open_deferred(
        Arc::clone(&wrapper) as Arc<dyn KanbanBackend>,
        AppConfig::default(),
    );
    let board_id = ctx
        .create_board("Roadmap".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id;
    let (sprint, _) = ctx
        .create_sprint_from_spec(board_id, None, Some("Alpha".to_string()), None, false)
        .unwrap();
    wrapper.clear_ops();
    (ctx, wrapper, sprint)
}

fn degraded(sprint: &Sprint) -> Sprint {
    Sprint {
        name_index: None,
        ..sprint.clone()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_committed_sprint_name_for_a_named_sprint_returns_the_pool_name_without_a_sprint_reread(
) {
    let dir = tempdir().unwrap();
    let (ctx, wrapper, sprint) = seeded_ctx(&dir.path().join("named.json"));

    let name = resolve_committed_sprint_name(&ctx, &sprint);

    assert_eq!(name, Some("Alpha".to_string()));
    assert_eq!(wrapper.op_count("get_sprint"), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_committed_sprint_name_for_a_sprint_returned_unnamed_rereads_it_and_returns_the_pool_name(
) {
    let dir = tempdir().unwrap();
    let (ctx, wrapper, sprint) = seeded_ctx(&dir.path().join("degraded.json"));
    let degraded = degraded(&sprint);

    let name = resolve_committed_sprint_name(&ctx, &degraded);

    assert_eq!(name, Some("Alpha".to_string()));
    assert_eq!(wrapper.op_count("get_sprint"), 1);
    assert_eq!(
        resolve_sprint_name(&ctx, &degraded).unwrap(),
        None,
        "the strict path alone cannot recover a sprint with no name_index"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_committed_sprint_name_when_the_sprint_reread_fails_returns_none() {
    let dir = tempdir().unwrap();
    let (ctx, wrapper, sprint) = seeded_ctx(&dir.path().join("reread_fails.json"));
    wrapper.fail("get_sprint");
    let degraded = degraded(&sprint);

    let name = resolve_committed_sprint_name(&ctx, &degraded);

    assert_eq!(name, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_committed_sprint_name_when_the_board_read_fails_returns_none() {
    let dir = tempdir().unwrap();
    let (ctx, wrapper, sprint) = seeded_ctx(&dir.path().join("board_fails.json"));
    wrapper.fail("get_board");
    let degraded = degraded(&sprint);

    assert_eq!(resolve_committed_sprint_name(&ctx, &sprint), None);
    assert_eq!(resolve_committed_sprint_name(&ctx, &degraded), None);
    assert!(resolve_sprint_name(&ctx, &sprint).is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_committed_sprint_name_for_an_unnamed_sprint_returns_none_without_a_board_read(
) {
    let dir = tempdir().unwrap();
    let (mut ctx, wrapper, _sprint) = seeded_ctx(&dir.path().join("unnamed.json"));
    let board_id = ctx.list_boards().unwrap()[0].id;
    let (unnamed, _) = ctx
        .create_sprint_from_spec(board_id, None, None, None, false)
        .unwrap();
    wrapper.clear_ops();

    let name = resolve_committed_sprint_name(&ctx, &unnamed);

    assert_eq!(name, None);
    assert_eq!(wrapper.op_count("get_sprint"), 1);
    assert_eq!(wrapper.op_count("get_board"), 0);
}
