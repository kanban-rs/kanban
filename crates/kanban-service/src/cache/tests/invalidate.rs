use kanban_domain::{EntityIds, FetchRound, Invalidation};

use super::{seed_board_with_column, seed_card, seed_sprint, store, FixedPlan};
use crate::cache::EntityCache;

#[test]
fn test_invalidate_entities_clears_only_the_named_ids() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let a = seed_card(&store, &board, &column, "a");
    let b = seed_card(&store, &board, &column, "b");
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        cards: vec![a.id, b.id],
        ..Default::default()
    });
    cache.resolve(&plan, &store).unwrap();

    cache.invalidate(Invalidation::Entities(EntityIds::cards([a.id])));

    assert!(cache.card(a.id).is_not_loaded());
    assert!(cache.card(b.id).is_loaded());
}

#[test]
fn test_invalidate_a_card_id_also_drops_the_whole_card_collection() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let a = seed_card(&store, &board, &column, "a");
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        board_list: true,
        column_list: true,
        card_list: true,
        sprint_list: true,
        graph: true,
        ..Default::default()
    });
    cache.resolve(&plan, &store).unwrap();
    assert!(cache.card_list().is_loaded());

    cache.invalidate(Invalidation::Entities(EntityIds::cards([a.id])));

    assert!(cache.card_list().is_not_loaded());
    assert!(cache.board_list().is_loaded());
    assert!(cache.column_list().is_loaded());
    assert!(cache.sprint_list().is_loaded());
    assert!(cache.graph().is_loaded());
}

#[test]
fn test_invalidate_all_clears_every_one_of_the_five_kinds() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let card = seed_card(&store, &board, &column, "a");
    let sprint = seed_sprint(&store, &board);
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        board_list: true,
        column_list: true,
        card_list: true,
        sprint_list: true,
        graph: true,
        columns: vec![column.id],
        cards: vec![card.id],
        sprints: vec![sprint.id],
    });
    cache.resolve(&plan, &store).unwrap();
    assert!(cache.column(column.id).is_loaded());
    assert!(cache.card(card.id).is_loaded());
    assert!(cache.sprint(sprint.id).is_loaded());

    cache.invalidate(Invalidation::All);

    assert!(cache.board_list().is_not_loaded());
    assert!(cache.column_list().is_not_loaded());
    assert!(cache.card_list().is_not_loaded());
    assert!(cache.sprint_list().is_not_loaded());
    assert!(cache.graph().is_not_loaded());
    assert!(cache.column(column.id).is_not_loaded());
    assert!(cache.card(card.id).is_not_loaded());
    assert!(cache.sprint(sprint.id).is_not_loaded());
}

#[test]
fn test_invalidate_prefixes_flag_drops_the_board_collection() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    seed_card(&store, &board, &column, "a");
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        board_list: true,
        card_list: true,
        graph: true,
        ..Default::default()
    });
    cache.resolve(&plan, &store).unwrap();

    cache.invalidate(Invalidation::Entities(EntityIds::default().with_prefixes()));

    assert!(cache.board_list().is_not_loaded());
    assert!(cache.card_list().is_loaded());
    assert!(cache.graph().is_loaded());
}

#[test]
fn test_invalidate_graph_flag_drops_only_the_graph() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    seed_card(&store, &board, &column, "a");
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        board_list: true,
        card_list: true,
        graph: true,
        ..Default::default()
    });
    cache.resolve(&plan, &store).unwrap();

    cache.invalidate(Invalidation::Entities(EntityIds::default().with_graph()));

    assert!(cache.graph().is_not_loaded());
    assert!(cache.board_list().is_loaded());
    assert!(cache.card_list().is_loaded());
}

#[test]
fn test_invalidate_entities_with_no_ids_clears_everything() {
    let store = store();
    let mut cache = EntityCache::new();
    let plan = FixedPlan(FetchRound {
        board_list: true,
        column_list: true,
        card_list: true,
        sprint_list: true,
        graph: true,
        ..Default::default()
    });
    cache.resolve(&plan, &store).unwrap();

    cache.invalidate(Invalidation::Entities(EntityIds::default()));

    assert!(cache.board_list().is_not_loaded());
    assert!(cache.column_list().is_not_loaded());
    assert!(cache.card_list().is_not_loaded());
    assert!(cache.sprint_list().is_not_loaded());
    assert!(cache.graph().is_not_loaded());
}
