use crate::fetch_plan::FetchRound;
use uuid::Uuid;

use super::{seed_board_with_column, seed_sprint, store, FixedPlan, StubLoaded};
use crate::resolve::resolve;

#[test]
fn test_a_failing_list_read_is_failed_not_a_loaded_empty_collection() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let loaded = StubLoaded::default();

    store.fail_method("list_boards");
    let resolved = resolve(
        &FixedPlan(FetchRound {
            board_list: true,
            ..Default::default()
        }),
        &loaded,
        &store,
    );
    assert!(
        resolved.boards.all.is_failed(),
        "list_boards: returned tier must be Failed, not Loaded(empty)"
    );
    store.clear_failures();

    store.fail_method("list_columns_by_board");
    let resolved = resolve(
        &FixedPlan(FetchRound {
            columns_by_board: vec![board.id],
            ..Default::default()
        }),
        &loaded,
        &store,
    );
    assert!(
        resolved.columns.by_parent[&board.id].is_failed(),
        "list_columns_by_board: returned tier must be Failed, not Loaded(empty)"
    );
    store.clear_failures();

    store.fail_method("list_cards_by_column");
    let resolved = resolve(
        &FixedPlan(FetchRound {
            cards_by_column: vec![column.id],
            ..Default::default()
        }),
        &loaded,
        &store,
    );
    assert!(
        resolved.cards.by_parent[&column.id].is_failed(),
        "list_cards_by_column: returned tier must be Failed, not Loaded(empty)"
    );
    store.clear_failures();

    store.fail_method("list_sprints_by_board");
    let resolved = resolve(
        &FixedPlan(FetchRound {
            sprints_by_board: vec![board.id],
            ..Default::default()
        }),
        &loaded,
        &store,
    );
    assert!(
        resolved.sprints.by_parent[&board.id].is_failed(),
        "list_sprints_by_board: returned tier must be Failed, not Loaded(empty)"
    );
}

#[test]
fn test_a_failing_graph_read_is_failed_not_an_empty_graph() {
    let store = store();
    store.fail_method("get_graph");
    let loaded = StubLoaded::default();

    let resolved = resolve(
        &FixedPlan(FetchRound {
            graph: true,
            ..Default::default()
        }),
        &loaded,
        &store,
    );

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
    let loaded = StubLoaded::default();

    let resolved = resolve(
        &FixedPlan(FetchRound {
            columns: vec![absent_column, column.id],
            sprints: vec![absent_sprint, sprint.id],
            ..Default::default()
        }),
        &loaded,
        &store,
    );

    assert!(resolved.columns.by_id[&absent_column].is_missing());
    assert!(resolved.sprints.by_id[&absent_sprint].is_missing());
    assert!(resolved.columns.by_id[&column.id].is_failed());
    assert!(resolved.sprints.by_id[&sprint.id].is_failed());
}
