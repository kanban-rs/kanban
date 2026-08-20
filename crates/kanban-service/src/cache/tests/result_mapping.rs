use kanban_domain::FetchRound;
use uuid::Uuid;

use super::{seed_board_with_column, seed_sprint, store, FixedPlan};
use crate::cache::EntityCache;

#[test]
fn test_a_failing_list_read_is_failed_not_a_loaded_empty_collection() {
    for (method, round) in [
        (
            "list_boards",
            FetchRound {
                board_list: true,
                ..Default::default()
            },
        ),
        (
            "list_all_columns",
            FetchRound {
                column_list: true,
                ..Default::default()
            },
        ),
        (
            "list_all_cards",
            FetchRound {
                card_list: true,
                ..Default::default()
            },
        ),
        (
            "list_all_sprints",
            FetchRound {
                sprint_list: true,
                ..Default::default()
            },
        ),
    ] {
        let store = store();
        seed_board_with_column(&store);
        store.fail_method(method);
        let mut cache = EntityCache::new();

        let resolved = cache.resolve(&FixedPlan(round), &store).unwrap();

        let (cached, returned) = match method {
            "list_boards" => (
                cache.board_list().is_failed(),
                resolved.boards.all.is_failed(),
            ),
            "list_all_columns" => (
                cache.column_list().is_failed(),
                resolved.columns.all.is_failed(),
            ),
            "list_all_cards" => (
                cache.card_list().is_failed(),
                resolved.cards.all.is_failed(),
            ),
            _ => (
                cache.sprint_list().is_failed(),
                resolved.sprints.all.is_failed(),
            ),
        };

        assert!(
            cached,
            "{method}: cached tier must be Failed, not Loaded(empty)"
        );
        assert!(
            returned,
            "{method}: returned tier must be Failed, not Loaded(empty)"
        );
    }
}

#[test]
fn test_a_failing_graph_read_is_failed_not_an_empty_graph() {
    let store = store();
    store.fail_method("get_graph");
    let mut cache = EntityCache::new();

    let resolved = cache
        .resolve(
            &FixedPlan(FetchRound {
                graph: true,
                ..Default::default()
            }),
            &store,
        )
        .unwrap();

    assert!(cache.graph().is_failed());
    assert!(resolved.graph.is_failed());
}

#[test]
fn test_an_absent_column_or_sprint_is_missing_and_a_failing_one_is_failed() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let sprint = seed_sprint(&store, &board);
    let absent_column = Uuid::new_v4();
    let absent_sprint = Uuid::new_v4();
    store.fail_column(column.id);
    store.fail_sprint(sprint.id);
    let mut cache = EntityCache::new();

    let resolved = cache
        .resolve(
            &FixedPlan(FetchRound {
                columns: vec![absent_column, column.id],
                sprints: vec![absent_sprint, sprint.id],
                ..Default::default()
            }),
            &store,
        )
        .unwrap();

    assert!(cache.column(absent_column).is_missing());
    assert!(cache.sprint(absent_sprint).is_missing());
    assert!(resolved.columns.by_id[&absent_column].is_missing());
    assert!(resolved.sprints.by_id[&absent_sprint].is_missing());

    assert!(cache.column(column.id).is_failed());
    assert!(cache.sprint(sprint.id).is_failed());
    assert!(resolved.columns.by_id[&column.id].is_failed());
    assert!(resolved.sprints.by_id[&sprint.id].is_failed());
}
