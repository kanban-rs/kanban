use kanban_domain::{EntityIds, FetchRound, Invalidation};

use super::{
    seed_board, seed_board_with_column, seed_card, store, BoardListPlan, CardByIdPlan, FixedPlan,
};
use crate::cache::EntityCache;
use crate::read_recorder::{assert_ops, ReadOp};

#[test]
fn test_a_failed_card_is_refetched_and_recovers_once_the_backend_does() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let card = seed_card(&store, &board, &column, "a");
    store.fail_card(card.id);
    let mut cache = EntityCache::new();
    let plan = CardByIdPlan { card_id: card.id };

    cache.resolve(&plan, &store).unwrap();
    assert!(cache.card(card.id).is_failed());

    store.clear_failures();
    store.clear_log();

    let resolved = cache.resolve(&plan, &store).unwrap();

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "get_card",
            ids: vec![card.id],
        }],
    );
    assert_eq!(cache.card(card.id).loaded().map(|c| c.id), Some(card.id));
    assert!(resolved.cards.by_id[&card.id].is_loaded());
}

#[test]
fn test_a_failed_collection_is_refetched_and_recovers_once_the_backend_does() {
    let store = store();
    seed_board(&store, "a");
    store.fail_method("list_boards");
    let mut cache = EntityCache::new();

    cache.resolve(&BoardListPlan, &store).unwrap();
    assert!(cache.board_list().is_failed());

    store.clear_failures();
    store.clear_log();

    let resolved = cache.resolve(&BoardListPlan, &store).unwrap();

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "list_boards",
            ids: vec![],
        }],
    );
    assert_eq!(cache.board_list().loaded().map(|b| b.len()), Some(1));
    assert!(resolved.boards.all.is_loaded());
}

#[test]
fn test_a_missing_card_is_not_refetched_while_a_failed_one_is() {
    let store = store();
    seed_board_with_column(&store);
    let absent = uuid::Uuid::new_v4();
    let mut cache = EntityCache::new();
    let plan = CardByIdPlan { card_id: absent };

    cache.resolve(&plan, &store).unwrap();
    assert!(cache.card(absent).is_missing());

    store.clear_log();
    cache.resolve(&plan, &store).unwrap();

    assert_ops(&store.ops(), &[]);
    assert!(cache.card(absent).is_missing());
}

#[test]
fn test_an_invalidated_entity_is_refetched_while_an_untouched_sibling_is_not() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let a = seed_card(&store, &board, &column, "a");
    let b = seed_card(&store, &board, &column, "b");
    let mut cache = EntityCache::new();
    let round = FetchRound {
        cards: vec![a.id, b.id],
        ..Default::default()
    };
    cache.resolve(&FixedPlan(round.clone()), &store).unwrap();
    assert!(cache.card(a.id).is_loaded());

    cache.invalidate(Invalidation::Entities(EntityIds::cards([a.id])));
    assert!(cache.card(a.id).is_not_loaded());
    assert!(cache.card(b.id).is_loaded());

    store.clear_log();
    let resolved = cache.resolve(&FixedPlan(round), &store).unwrap();

    assert_eq!(cache.card(a.id).loaded().map(|c| c.id), Some(a.id));
    assert!(resolved.cards.by_id[&a.id].is_loaded());
}

#[test]
fn test_a_refetched_collection_replaces_the_previously_cached_contents() {
    let store = store();
    seed_board(&store, "a");
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        board_list: true,
        ..Default::default()
    });
    cache.resolve(&plan, &store).unwrap();
    assert_eq!(cache.board_list().loaded().map(|b| b.len()), Some(1));

    seed_board(&store, "b");
    let resolved = cache.resolve(&plan, &store).unwrap();

    assert_eq!(cache.board_list().loaded().map(|b| b.len()), Some(2));
    assert_eq!(resolved.boards.all.loaded().map(|b| b.len()), Some(2));
}
