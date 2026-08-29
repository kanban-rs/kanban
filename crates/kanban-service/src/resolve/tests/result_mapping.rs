use crate::fetch_plan::FetchRound;
use uuid::Uuid;

use super::{seed_board_with_column, seed_sprint, store, FixedPlan, StubLoaded};
use crate::resolve::resolve;

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
        let loaded = StubLoaded::default();

        let resolved = resolve(&FixedPlan(round), &loaded, &store);

        let returned = match method {
            "list_boards" => resolved.boards.all.is_failed(),
            "list_all_columns" => resolved.columns.all.is_failed(),
            "list_all_cards" => resolved.cards.all.is_failed(),
            _ => resolved.sprints.all.is_failed(),
        };

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
