use crate::fetch_plan::FetchRound;

use super::{
    seed_board, seed_board_with_column, seed_card, store, BoardListPlan, CardsByIdPlan, FixedPlan,
    StubLoaded,
};
use crate::read_recorder::{assert_ops, ReadOp};
use crate::resolve::resolve;

#[test]
fn test_a_failed_card_is_refetched_and_recovers_once_the_backend_does() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let card = seed_card(&store, &board, &column, "a");
    store.fail_card(card.id);
    let mut loaded = StubLoaded::default();
    let plan = CardsByIdPlan { ids: vec![card.id] };

    let resolved = resolve(&plan, &loaded, &store);
    loaded.apply(resolved);
    assert!(loaded.card_state(card.id).is_failed());

    store.clear_failures();
    store.clear_log();

    let resolved = resolve(&plan, &loaded, &store);
    loaded.apply(resolved);

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "get_card",
            ids: vec![card.id],
        }],
    );
    assert_eq!(
        loaded.card_state(card.id).loaded().map(|c| c.id),
        Some(card.id)
    );
}

#[test]
fn test_a_failed_collection_is_refetched_and_recovers_once_the_backend_does() {
    let store = store();
    seed_board(&store, "a");
    store.fail_method("list_boards");
    let mut loaded = StubLoaded::default();

    let resolved = resolve(&BoardListPlan, &loaded, &store);
    loaded.apply(resolved);
    assert!(loaded.board_list_state().is_failed());

    store.clear_failures();
    store.clear_log();

    let resolved = resolve(&BoardListPlan, &loaded, &store);
    loaded.apply(resolved);

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "list_boards",
            ids: vec![],
        }],
    );
    assert_eq!(loaded.board_list_state().loaded().map(|b| b.len()), Some(1));
}

#[test]
fn test_a_missing_card_is_not_refetched_while_a_failed_one_is() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let failing = seed_card(&store, &board, &column, "a");
    let absent = uuid::Uuid::new_v4();
    store.fail_card(failing.id);
    let mut loaded = StubLoaded::default();
    let plan = CardsByIdPlan {
        ids: vec![absent, failing.id],
    };

    let resolved = resolve(&plan, &loaded, &store);
    loaded.apply(resolved);
    assert!(loaded.card_state(absent).is_missing());
    assert!(loaded.card_state(failing.id).is_failed());

    store.clear_log();
    let resolved = resolve(&plan, &loaded, &store);
    loaded.apply(resolved);

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "get_card",
            ids: vec![failing.id],
        }],
    );
    assert!(loaded.card_state(absent).is_missing());
}

#[test]
fn test_an_invalidated_entity_is_refetched_while_an_untouched_sibling_is_not() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let a = seed_card(&store, &board, &column, "a");
    let b = seed_card(&store, &board, &column, "b");
    let mut loaded = StubLoaded::default();
    let plan = CardsByIdPlan {
        ids: vec![a.id, b.id],
    };
    let resolved = resolve(&plan, &loaded, &store);
    loaded.apply(resolved);
    assert!(loaded.card_state(a.id).is_loaded());
    assert!(loaded.card_state(b.id).is_loaded());

    loaded.forget_card(a.id);
    assert!(loaded.card_state(a.id).is_not_loaded());
    assert!(loaded.card_state(b.id).is_loaded());

    store.clear_log();
    let resolved = resolve(&plan, &loaded, &store);

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "get_card",
            ids: vec![a.id],
        }],
    );
    assert_eq!(resolved.cards.by_id.len(), 1);
    assert!(resolved.cards.by_id[&a.id].is_loaded());
    loaded.apply(resolved);
    assert_eq!(loaded.card_state(a.id).loaded().map(|c| c.id), Some(a.id));
}

#[test]
fn test_a_refetched_collection_replaces_the_previously_cached_contents() {
    let store = store();
    seed_board(&store, "a");
    let mut loaded = StubLoaded::default();
    let plan = FixedPlan(FetchRound {
        board_list: true,
        ..Default::default()
    });
    let resolved = resolve(&plan, &loaded, &store);
    loaded.apply(resolved);
    assert_eq!(loaded.board_list_state().loaded().map(|b| b.len()), Some(1));

    seed_board(&store, "b");
    let resolved = resolve(&plan, &loaded, &store);
    assert_eq!(resolved.boards.all.loaded().map(|b| b.len()), Some(2));
    loaded.apply(resolved);

    assert_eq!(loaded.board_list_state().loaded().map(|b| b.len()), Some(2));
}
