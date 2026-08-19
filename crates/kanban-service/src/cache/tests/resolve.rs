use kanban_domain::FetchRound;
use uuid::Uuid;

use super::{
    seed_board, seed_board_with_column, seed_card, store, BoardListPlan, FixedPlan,
    GraphThenCardPlan,
};
use crate::cache::EntityCache;
use crate::read_recorder::{assert_ops, ReadOp};

#[test]
fn test_resolve_for_a_board_list_need_issues_exactly_one_list_boards_read() {
    let store = store();
    seed_board(&store, "a");
    seed_board(&store, "b");
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        board_list: true,
        ..Default::default()
    });

    let resolved = cache.resolve(&plan, &store).unwrap();

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "list_boards",
            ids: vec![],
        }],
    );
    assert!(resolved.boards.all.is_loaded());
    assert_eq!(resolved.boards.all.loaded().unwrap().len(), 2);
}

#[test]
fn test_resolve_of_a_whole_collection_need_populates_all_not_just_by_id() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    seed_card(&store, &board, &column, "a");
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        card_list: true,
        ..Default::default()
    });

    let resolved = cache.resolve(&plan, &store).unwrap();

    assert!(resolved.cards.all.is_loaded());
    assert_eq!(resolved.cards.all.loaded().unwrap().len(), 1);
    assert!(resolved.cards.by_id.is_empty());
}

#[test]
fn test_resolve_of_a_genuinely_empty_card_list_is_loaded_not_not_loaded() {
    let store = store();
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        card_list: true,
        ..Default::default()
    });

    let resolved = cache.resolve(&plan, &store).unwrap();

    assert!(resolved.cards.all.is_loaded());
    assert!(resolved.cards.all.loaded().unwrap().is_empty());
    assert!(!resolved.cards.all.is_not_loaded());
    assert!(!resolved.cards.is_untouched());
}

#[test]
fn test_resolve_never_scans_a_collection_to_serve_a_single_id_need() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let a = seed_card(&store, &board, &column, "a");
    seed_card(&store, &board, &column, "b");
    seed_card(&store, &board, &column, "c");
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        cards: vec![a.id],
        ..Default::default()
    });

    let resolved = cache.resolve(&plan, &store).unwrap();

    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "get_card",
            ids: vec![a.id],
        }],
    );
    assert!(resolved.cards.all.is_not_loaded());
}

#[test]
fn test_resolve_maps_ok_none_to_missing() {
    let store = store();
    let id = Uuid::new_v4();
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        cards: vec![id],
        ..Default::default()
    });

    let resolved = cache.resolve(&plan, &store).unwrap();

    let state = &resolved.cards.by_id[&id];
    assert!(state.is_missing());
    assert!(!state.is_not_loaded());
    assert!(!state.is_failed());
    assert!(!state.is_loaded());
}

#[test]
fn test_resolve_maps_a_backend_error_to_failed_without_starving_the_round() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let bad = seed_card(&store, &board, &column, "bad");
    let good = seed_card(&store, &board, &column, "good");
    store.fail_card(bad.id);
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        cards: vec![bad.id, good.id],
        ..Default::default()
    });

    let resolved = cache.resolve(&plan, &store).unwrap();

    assert!(resolved.cards.by_id[&bad.id].is_failed());
    assert!(resolved.cards.by_id[&good.id].is_loaded());
    assert_ops(
        &store.ops(),
        &[
            ReadOp {
                method: "get_card",
                ids: vec![bad.id],
            },
            ReadOp {
                method: "get_card",
                ids: vec![good.id],
            },
        ],
    );
}

#[test]
fn test_resolve_does_not_refetch_an_already_loaded_entity() {
    let store = store();
    seed_board(&store, "a");
    let mut cache = EntityCache::new();
    let plan = BoardListPlan;

    cache.resolve(&plan, &store).unwrap();
    cache.resolve(&plan, &store).unwrap();

    assert_eq!(store.ops().lock().unwrap().len(), 1);
}

#[test]
fn test_a_card_never_fetched_reads_as_not_loaded_and_a_missing_one_reads_as_missing() {
    let store = store();
    let id = Uuid::new_v4();
    let mut cache = EntityCache::new();

    assert!(cache.card(id).is_not_loaded());

    let plan = FixedPlan(FetchRound {
        cards: vec![id],
        ..Default::default()
    });
    cache.resolve(&plan, &store).unwrap();

    assert!(cache.card(id).is_missing());
}

#[test]
fn test_a_loaded_card_collection_does_not_report_an_unfetched_card_id_as_loaded() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let a = seed_card(&store, &board, &column, "a");
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        card_list: true,
        ..Default::default()
    });

    cache.resolve(&plan, &store).unwrap();

    assert!(cache.card_list().is_loaded());
    assert!(cache.card(a.id).is_not_loaded());
}

#[test]
fn test_a_single_round_resolve_cannot_serve_a_dependent_need() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let card = seed_card(&store, &board, &column, "a");
    let mut cache = EntityCache::new();
    let plan = GraphThenCardPlan { card_id: card.id };

    let resolved = cache.resolve(&plan, &store).unwrap();

    assert!(cache.graph().is_loaded());
    assert!(cache.card(card.id).is_not_loaded());
    assert!(resolved.cards.by_id.is_empty());
    assert_ops(
        &store.ops(),
        &[ReadOp {
            method: "get_graph",
            ids: vec![],
        }],
    );
}
