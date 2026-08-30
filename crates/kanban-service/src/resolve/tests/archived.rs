use crate::fetch_plan::FetchRound;

use super::{
    seed_archived_card, seed_board, seed_board_with_column, seed_card, seed_column, store,
    FixedPlan, OneShotPlan, StubLoaded,
};
use crate::read_recorder::{assert_ops, ReadOp};
use crate::resolve::resolve;

#[test]
fn test_a_board_scoped_archived_card_round_resolves_the_boards_markers() {
    let store = store();
    let (board_a, col_a) = seed_board_with_column(&store);
    let card_a = seed_card(&store, &board_a, &col_a, "a");
    let marker_a = seed_archived_card(&store, &board_a, &card_a);

    let board_b = seed_board(&store, "b");
    let col_b = seed_column(&store, &board_b, "c");
    let card_b = seed_card(&store, &board_b, &col_b, "b");
    let _marker_b = seed_archived_card(&store, &board_b, &card_b);

    let loaded = StubLoaded::default();
    let plan = OneShotPlan::new(FetchRound {
        archived_cards_by_board: vec![board_a.id],
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "list_archived_cards_by_board",
            ids: vec![board_a.id],
        }],
    );
    let scoped = resolved.archived_cards.by_parent[&board_a.id]
        .loaded()
        .unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].entity_id, marker_a.entity_id);
    assert!(resolved.archived_cards.all.is_not_loaded());
    assert!(resolved.archived_cards.by_id.is_empty());
}

#[test]
fn test_an_archived_card_read_error_resolves_failed_not_empty() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let card = seed_card(&store, &board, &column, "a");
    let _marker = seed_archived_card(&store, &board, &card);
    store.fail_method("list_archived_cards_by_board");

    let loaded = StubLoaded::default();
    let plan = FixedPlan(FetchRound {
        archived_cards_by_board: vec![board.id],
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    let state = &resolved.archived_cards.by_parent[&board.id];
    assert!(state.is_failed());
    assert!(!state.is_loaded());
    assert!(!state.is_missing());
}

#[test]
fn test_a_flat_archived_card_round_resolves_every_marker() {
    let store = store();
    let (board_a, col_a) = seed_board_with_column(&store);
    let card_a = seed_card(&store, &board_a, &col_a, "a");
    seed_archived_card(&store, &board_a, &card_a);

    let board_b = seed_board(&store, "b");
    let col_b = seed_column(&store, &board_b, "c");
    let card_b = seed_card(&store, &board_b, &col_b, "b");
    seed_archived_card(&store, &board_b, &card_b);

    let loaded = StubLoaded::default();
    let plan = FixedPlan(FetchRound {
        archived_card_list: true,
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "list_archived_cards",
            ids: vec![],
        }],
    );
    let all = resolved.archived_cards.all.loaded().unwrap();
    assert_eq!(all.len(), 2);
    assert!(resolved.archived_cards.by_parent.is_empty());
    assert!(resolved.archived_cards.by_id.is_empty());
}

#[test]
fn test_an_unrequested_archived_tier_stays_not_loaded() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let card = seed_card(&store, &board, &column, "a");
    seed_archived_card(&store, &board, &card);

    let loaded = StubLoaded::default();
    let plan = FixedPlan(FetchRound {
        card_list: true,
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    assert!(resolved.cards.all.is_loaded());
    assert!(resolved.archived_cards.is_untouched());
    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "list_all_cards",
            ids: vec![],
        }],
    );
}

#[test]
fn test_a_plan_that_repeats_an_archived_scope_fetches_it_once_and_halts() {
    let store = store();
    let (board, _column) = seed_board_with_column(&store);
    let loaded = StubLoaded::default();
    let plan = FixedPlan(FetchRound {
        archived_cards_by_board: vec![board.id],
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    assert!(resolved.archived_cards.by_parent[&board.id].is_loaded());
    let ops = store.ops();
    let ops = ops.lock().unwrap();
    assert_eq!(
        ops.iter()
            .filter(|op| op.method == "list_archived_cards_by_board")
            .count(),
        1
    );
}
