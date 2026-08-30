use kanban_domain::{ArchivedCard, LoadState, Resolved};

use crate::fetch_plan::FetchRound;
use uuid::Uuid;

use super::{
    seed_board, seed_board_with_column, seed_card, seed_column, store, CardsByIdPlan,
    HierarchyPlan, ProbePlan, StubLoaded,
};
use crate::fetch_plan::{FetchStatus, LoadedEntities, LoadedState as LoadedStateTrait};
use crate::read_recorder::assert_ops;
use crate::resolve::{resolve, Overlay};

#[test]
fn test_a_scope_discovered_from_an_earlier_round_resolves_in_the_same_call() {
    let store = store();
    let board = seed_board(&store, "b");
    let col_a = seed_column(&store, &board, "a");
    let col_b = seed_column(&store, &board, "b");
    seed_card(&store, &board, &col_a, "1");
    seed_card(&store, &board, &col_b, "2");
    let loaded = StubLoaded::default();
    let plan = HierarchyPlan { board_id: board.id };

    let resolved = resolve(&plan, &loaded, &store);

    assert!(resolved.boards.all.is_loaded());
    assert!(resolved.columns.by_parent[&board.id].is_loaded());
    assert_eq!(
        resolved.columns.by_parent[&board.id]
            .loaded()
            .unwrap()
            .len(),
        2
    );
    assert!(resolved.cards.by_parent[&col_a.id].is_loaded());
    assert!(resolved.cards.by_parent[&col_b.id].is_loaded());

    assert_ops(
        &store.ops(),
        &[
            crate::read_recorder::ReadOp {
                method: "list_boards",
                ids: vec![],
            },
            crate::read_recorder::ReadOp {
                method: "list_columns_by_board",
                ids: vec![board.id],
            },
            crate::read_recorder::ReadOp {
                method: "list_cards_by_column",
                ids: vec![col_a.id],
            },
            crate::read_recorder::ReadOp {
                method: "list_cards_by_column",
                ids: vec![col_b.id],
            },
        ],
    );
}

#[test]
fn test_the_overlay_reports_the_pass_over_the_base() {
    let base = StubLoaded::default();
    let mut resolved = Resolved::default();
    resolved.cards.all = LoadState::Loaded(vec![]);
    let overlay = Overlay {
        base: &base,
        pass: &resolved,
    };

    assert_eq!(overlay.card_list(), FetchStatus::Loaded);
}

#[test]
fn test_the_overlay_falls_through_to_the_base_for_an_untouched_tier() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let card = seed_card(&store, &board, &column, "a");
    let plan = CardsByIdPlan { ids: vec![card.id] };
    let mut base = StubLoaded::default();
    base.forget_card(card.id);
    base.cards.by_id.insert(card.id, LoadState::Missing);

    let resolved = Resolved::default();
    let overlay = Overlay {
        base: &base,
        pass: &resolved,
    };
    assert_eq!(overlay.card(card.id), FetchStatus::Missing);

    store.clear_log();
    resolve(&plan, &base, &store);
    assert!(store.ops().lock().unwrap().is_empty());
}

#[test]
fn test_a_scope_the_pass_failed_does_not_fall_through_to_a_loaded_base() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let mut base = StubLoaded::default();
    base.columns
        .by_parent
        .insert(board.id, LoadState::Loaded(vec![column.clone()]));

    let mut resolved = Resolved::default();
    resolved.columns.by_parent.insert(
        board.id,
        LoadState::Failed(std::sync::Arc::new(
            kanban_domain::KanbanError::unsupported("x"),
        )),
    );
    let overlay = Overlay {
        base: &base,
        pass: &resolved,
    };

    assert!(overlay.loaded_columns_of_board(board.id).is_none());
    assert_eq!(overlay.columns_of_board(board.id), FetchStatus::Failed);
}

#[test]
fn test_an_empty_base_and_empty_pass_project_every_dimension_as_not_loaded() {
    let store = store();
    let loaded = StubLoaded::default();
    let board_id = Uuid::new_v4();
    let column_id = Uuid::new_v4();
    let card_id = Uuid::new_v4();
    let sprint_id = Uuid::new_v4();
    let plan = ProbePlan::new(
        FetchRound::default(),
        board_id,
        column_id,
        card_id,
        sprint_id,
    );

    resolve(&plan, &loaded, &store);

    let observed = plan.last();
    assert_eq!(observed.board_list, FetchStatus::NotLoaded);
    assert_eq!(observed.column_list, FetchStatus::NotLoaded);
    assert_eq!(observed.card_list, FetchStatus::NotLoaded);
    assert_eq!(observed.sprint_list, FetchStatus::NotLoaded);
    assert_eq!(observed.graph, FetchStatus::NotLoaded);
    assert_eq!(observed.column, FetchStatus::NotLoaded);
    assert_eq!(observed.card, FetchStatus::NotLoaded);
    assert_eq!(observed.sprint, FetchStatus::NotLoaded);
    assert_eq!(observed.columns_of_board, FetchStatus::NotLoaded);
    assert_eq!(observed.cards_of_column, FetchStatus::NotLoaded);
    assert_eq!(observed.sprints_of_board, FetchStatus::NotLoaded);
}

#[test]
fn test_overlay_reports_a_pass_loaded_archived_board_as_satisfied() {
    let base = StubLoaded::default();
    let board_id = Uuid::new_v4();
    let mut resolved = Resolved::default();
    resolved
        .archived_cards
        .by_parent
        .insert(board_id, LoadState::Loaded(Vec::<ArchivedCard>::new()));
    let overlay = Overlay {
        base: &base,
        pass: &resolved,
    };

    assert_eq!(
        overlay.archived_cards_of_board(board_id),
        FetchStatus::Loaded
    );
    assert_eq!(
        overlay.archived_cards_of_board(Uuid::new_v4()),
        FetchStatus::NotLoaded
    );
    assert_eq!(overlay.archived_card_list(), FetchStatus::NotLoaded);
}
