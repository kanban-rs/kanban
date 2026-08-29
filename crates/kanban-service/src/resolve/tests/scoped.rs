use crate::fetch_plan::FetchRound;

use super::{seed_board_with_column, seed_card, store, FixedPlan, OneShotPlan, StubLoaded};
use crate::read_recorder::{assert_ops, ReadOp};
use crate::resolve::resolve;

#[test]
fn test_a_scoped_round_fetches_the_children_of_the_named_parent() {
    let store = store();
    let (board, col_a) = seed_board_with_column(&store);
    let col_b = super::seed_column(&store, &board, "other");
    let card1 = seed_card(&store, &board, &col_a, "1");
    let card2 = seed_card(&store, &board, &col_a, "2");
    let _ = col_b;
    let loaded = StubLoaded::default();
    let plan = OneShotPlan::new(FetchRound {
        cards_by_column: vec![col_a.id],
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "list_cards_by_column",
            ids: vec![col_a.id],
        }],
    );
    let scoped = resolved.cards.by_parent[&col_a.id].loaded().unwrap();
    let mut ids: Vec<_> = scoped.iter().map(|c| c.id).collect();
    ids.sort();
    let mut expected = vec![card1.id, card2.id];
    expected.sort();
    assert_eq!(ids, expected);
    assert!(resolved.cards.all.is_not_loaded());
    assert!(resolved.cards.by_id.is_empty());
}

#[test]
fn test_a_scope_with_no_children_resolves_to_loaded_and_empty() {
    let store = store();
    let (_board, column) = seed_board_with_column(&store);
    let loaded = StubLoaded::default();
    let plan = OneShotPlan::new(FetchRound {
        cards_by_column: vec![column.id],
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    let state = &resolved.cards.by_parent[&column.id];
    assert!(state.is_loaded());
    assert!(state.loaded().unwrap().is_empty());
    assert!(!state.is_missing());
}

#[test]
fn test_every_scoped_tier_resolves_in_one_round() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let loaded = StubLoaded::default();
    let plan = OneShotPlan::new(FetchRound {
        columns_by_board: vec![board.id],
        cards_by_column: vec![column.id],
        sprints_by_board: vec![board.id],
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    assert!(resolved.columns.by_parent[&board.id].is_loaded());
    assert!(resolved.cards.by_parent[&column.id].is_loaded());
    assert!(resolved.sprints.by_parent[&board.id].is_loaded());
    assert!(resolved.boards.by_parent.is_empty());
    assert_ops(
        &store.ops(),
        &[
            ReadOp {
                method: "list_columns_by_board",
                ids: vec![board.id],
            },
            ReadOp {
                method: "list_cards_by_column",
                ids: vec![column.id],
            },
            ReadOp {
                method: "list_sprints_by_board",
                ids: vec![board.id],
            },
        ],
    );
}

#[test]
fn test_a_plan_that_repeats_a_scope_fetches_it_once_and_halts() {
    let store = store();
    let (_board, column) = seed_board_with_column(&store);
    let loaded = StubLoaded::default();
    let plan = FixedPlan(FetchRound {
        cards_by_column: vec![column.id],
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    assert!(resolved.cards.by_parent[&column.id].is_loaded());
    let ops = store.ops();
    let ops = ops.lock().unwrap();
    assert_eq!(
        ops.iter()
            .filter(|op| op.method == "list_cards_by_column")
            .count(),
        1
    );
}

#[test]
fn test_a_failed_scoped_read_maps_to_failed_and_never_to_missing() {
    let store = store();
    let (board, _column) = seed_board_with_column(&store);
    store.fail_method("list_columns_by_board");
    let loaded = StubLoaded::default();
    let plan = FixedPlan(FetchRound {
        columns_by_board: vec![board.id],
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    let state = &resolved.columns.by_parent[&board.id];
    assert!(state.is_failed());
    assert!(!state.is_missing());
    assert!(!state.is_loaded());
}

#[test]
fn test_a_failed_scope_is_retried_by_a_later_resolve_call() {
    let store = store();
    let (_board, column) = seed_board_with_column(&store);
    store.fail_method("list_cards_by_column");
    let mut loaded = StubLoaded::default();
    let plan = FixedPlan(FetchRound {
        cards_by_column: vec![column.id],
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);
    loaded.apply(resolved);

    store.clear_failures();
    store.clear_log();

    let resolved = resolve(&plan, &loaded, &store);

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "list_cards_by_column",
            ids: vec![column.id],
        }],
    );
    assert!(resolved.cards.by_parent[&column.id].is_loaded());
}

#[test]
fn test_a_scoped_fetch_leaves_the_list_and_id_tiers_untouched() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    seed_card(&store, &board, &column, "a");
    seed_card(&store, &board, &column, "b");
    let loaded = StubLoaded::default();
    let plan = OneShotPlan::new(FetchRound {
        cards_by_column: vec![column.id],
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    assert!(resolved.cards.by_parent[&column.id].is_loaded());
    assert_eq!(
        resolved.cards.by_parent[&column.id].loaded().unwrap().len(),
        2
    );
    assert!(resolved.cards.all.is_not_loaded());
    assert!(resolved.cards.by_id.is_empty());
}

#[test]
fn test_a_whole_list_fetch_leaves_the_scoped_tier_untouched() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    seed_card(&store, &board, &column, "a");
    let loaded = StubLoaded::default();
    let plan = FixedPlan(FetchRound {
        card_list: true,
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    assert!(resolved.cards.all.is_loaded());
    assert!(resolved.cards.by_parent.is_empty());
}
