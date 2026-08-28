use kanban_domain::FetchRound;
use uuid::Uuid;

use super::{
    seed_board_with_column, seed_card, store, CardListThenCardPlan, CardsByIdPlan, ChainPlan,
    FixedPlan, GraphThenCardPlan, StickyThenNextPlan,
};
use crate::cache::EntityCache;
use crate::read_recorder::{assert_ops, ReadOp};

#[test]
fn test_resolve_serves_a_need_whose_ids_depend_on_an_earlier_round() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let card = seed_card(&store, &board, &column, "a");
    let mut cache = EntityCache::new();
    let plan = GraphThenCardPlan { card_id: card.id };

    let resolved = cache.resolve(&plan, &store).unwrap();

    assert_ops(
        &store.ops(),
        &[
            ReadOp {
                method: "get_graph",
                ids: vec![],
            },
            ReadOp {
                method: "get_card",
                ids: vec![card.id],
            },
        ],
    );
    assert!(cache.graph().is_loaded());
    assert_eq!(cache.card(card.id).loaded().map(|c| c.id), Some(card.id));
    assert!(resolved.cards.by_id[&card.id].is_loaded());
}

#[test]
fn test_resolve_walks_a_chain_of_twelve_dependent_links() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let ids: Vec<Uuid> = (0..12)
        .map(|i| seed_card(&store, &board, &column, &format!("c{i}")).id)
        .collect();
    let mut cache = EntityCache::new();
    let plan = ChainPlan { ids: ids.clone() };

    let resolved = cache.resolve(&plan, &store).unwrap();

    for id in &ids {
        assert!(cache.card(*id).is_loaded());
    }
    assert_eq!(store.ops().lock().unwrap().len(), 12);
    assert_eq!(resolved.cards.by_id.len(), 12);
}

#[test]
fn test_a_permanently_missing_id_costs_exactly_one_read_across_repeated_resolves() {
    let store = store();
    let absent = Uuid::new_v4();
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        cards: vec![absent],
        ..Default::default()
    });

    cache.resolve(&plan, &store).unwrap();
    cache.resolve(&plan, &store).unwrap();

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "get_card",
            ids: vec![absent],
        }],
    );
    assert!(cache.card(absent).is_missing());
}

#[test]
fn test_a_missing_id_is_read_once_across_calls_while_a_loaded_sibling_is_refetched() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let present = seed_card(&store, &board, &column, "present");
    let absent = Uuid::new_v4();
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        cards: vec![absent, present.id],
        ..Default::default()
    });

    cache.resolve(&plan, &store).unwrap();
    cache.resolve(&plan, &store).unwrap();

    assert_ops(
        &store.ops(),
        &[
            ReadOp {
                method: "get_card",
                ids: vec![absent],
            },
            ReadOp {
                method: "get_card",
                ids: vec![present.id],
            },
            ReadOp {
                method: "get_card",
                ids: vec![present.id],
            },
        ],
    );
    assert!(cache.card(absent).is_missing());
    assert!(cache.card(present.id).is_loaded());
}

#[test]
fn test_a_plan_that_keeps_naming_a_loaded_card_fetches_it_once_and_still_serves_the_dependent_need()
{
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let first = seed_card(&store, &board, &column, "first");
    let second = seed_card(&store, &board, &column, "second");
    let mut cache = EntityCache::new();
    let plan = StickyThenNextPlan {
        first: first.id,
        second: second.id,
    };

    cache.resolve(&plan, &store).unwrap();

    assert_ops(
        &store.ops(),
        &[
            ReadOp {
                method: "get_card",
                ids: vec![first.id],
            },
            ReadOp {
                method: "get_card",
                ids: vec![second.id],
            },
        ],
    );
    assert!(cache.card(first.id).is_loaded());
    assert!(cache.card(second.id).is_loaded());
}

#[test]
fn test_resolve_returns_entities_from_every_round_not_only_the_last() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let card = seed_card(&store, &board, &column, "a");
    let mut cache = EntityCache::new();
    let plan = GraphThenCardPlan { card_id: card.id };

    let resolved = cache.resolve(&plan, &store).unwrap();

    assert!(resolved.graph.is_loaded());
    assert!(resolved
        .cards
        .by_id
        .get(&card.id)
        .is_some_and(|s| s.is_loaded()));
}

#[test]
fn test_a_later_round_does_not_clobber_a_collection_loaded_by_an_earlier_round() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let card = seed_card(&store, &board, &column, "a");
    let mut cache = EntityCache::new();
    let plan = CardListThenCardPlan { card_id: card.id };

    let resolved = cache.resolve(&plan, &store).unwrap();

    assert_ops(
        &store.ops(),
        &[
            ReadOp {
                method: "list_all_cards",
                ids: vec![],
            },
            ReadOp {
                method: "get_card",
                ids: vec![card.id],
            },
        ],
    );
    assert!(resolved.cards.all.is_loaded());
    assert_eq!(resolved.cards.all.loaded().unwrap().len(), 1);
    assert!(resolved.cards.by_id[&card.id].is_loaded());
}

#[test]
fn test_narrowing_does_not_drop_a_not_loaded_id() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let card = seed_card(&store, &board, &column, "a");
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        cards: vec![card.id],
        ..Default::default()
    });

    cache.resolve(&plan, &store).unwrap();

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "get_card",
            ids: vec![card.id],
        }],
    );
    assert!(cache.card(card.id).is_loaded());
}

#[test]
fn test_an_empty_first_round_performs_zero_reads() {
    let store = store();
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound::default());

    let resolved = cache.resolve(&plan, &store).unwrap();

    assert!(store.ops().lock().unwrap().is_empty());
    assert!(resolved.boards.is_untouched());
    assert!(resolved.columns.is_untouched());
    assert!(resolved.cards.is_untouched());
    assert!(resolved.sprints.is_untouched());
    assert!(resolved.graph.is_not_loaded());
}

#[test]
fn test_a_failed_entity_is_not_refetched_within_a_single_resolve() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let card = seed_card(&store, &board, &column, "a");
    store.fail_card(card.id);
    let mut cache = EntityCache::new();
    let plan = CardsByIdPlan { ids: vec![card.id] };

    cache.resolve(&plan, &store).unwrap();

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "get_card",
            ids: vec![card.id],
        }],
    );
    assert!(cache.card(card.id).is_failed());
}
