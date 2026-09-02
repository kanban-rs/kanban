use crate::fetch_plan::FetchRound;

use super::{seed_archived_board, seed_board, store, FixedPlan, StubLoaded};
use crate::read_recorder::{assert_ops, ReadOp};
use crate::resolve::resolve;

#[test]
fn test_a_flat_archived_board_round_resolves_every_marker() {
    let store = store();
    let live = seed_board(&store, "live");
    let archived_a = seed_board(&store, "a");
    seed_archived_board(&store, &archived_a);
    let archived_b = seed_board(&store, "b");
    seed_archived_board(&store, &archived_b);

    let loaded = StubLoaded::default();
    let plan = FixedPlan(FetchRound {
        archived_board_list: true,
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "list_archived_boards",
            ids: vec![],
        }],
    );
    let all = resolved.archived_boards.all.loaded().unwrap();
    assert_eq!(all.len(), 2);
    assert!(resolved.archived_boards.by_parent.is_empty());
    assert!(resolved.archived_boards.by_id.is_empty());
    let _ = live;
}

#[test]
fn test_an_unrequested_archived_board_tier_stays_not_loaded() {
    let store = store();
    let board = seed_board(&store, "b");
    seed_archived_board(&store, &board);

    let loaded = StubLoaded::default();
    let plan = FixedPlan(FetchRound {
        board_list: true,
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    assert!(resolved.boards.all.is_loaded());
    assert!(resolved.archived_boards.is_untouched());
}

#[test]
fn test_an_archived_board_read_error_resolves_failed_not_empty() {
    let store = store();
    let board = seed_board(&store, "b");
    seed_archived_board(&store, &board);
    store.fail_method("list_archived_boards");

    let loaded = StubLoaded::default();
    let plan = FixedPlan(FetchRound {
        archived_board_list: true,
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    let state = &resolved.archived_boards.all;
    assert!(state.is_failed());
    assert!(!state.is_loaded());
}

#[test]
fn test_a_plan_that_repeats_the_archived_board_list_fetches_it_once_and_halts() {
    let store = store();
    let board = seed_board(&store, "b");
    seed_archived_board(&store, &board);
    let loaded = StubLoaded::default();
    let plan = FixedPlan(FetchRound {
        archived_board_list: true,
        ..Default::default()
    });

    let resolved = resolve(&plan, &loaded, &store);

    assert!(resolved.archived_boards.all.is_loaded());
    let ops = store.ops();
    let ops = ops.lock().unwrap();
    assert_eq!(
        ops.iter()
            .filter(|op| op.method == "list_archived_boards")
            .count(),
        1
    );
}
