use kanban_domain::{FetchRound, FetchStatus};
use uuid::Uuid;

use super::{seed_board_with_column, seed_card, seed_sprint, store, Observed, ProbePlan};
use crate::cache::EntityCache;

fn all(status: FetchStatus) -> Observed {
    Observed {
        board_list: status,
        column_list: status,
        card_list: status,
        sprint_list: status,
        graph: status,
        column: status,
        card: status,
        sprint: status,
    }
}

#[test]
fn test_an_empty_cache_projects_every_dimension_as_not_loaded() {
    let store = store();
    let plan = ProbePlan::new(
        FetchRound::default(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let mut cache = EntityCache::new();

    cache.resolve(&plan, &store).unwrap();

    assert_eq!(plan.last(), all(FetchStatus::NotLoaded));
}

#[test]
fn test_each_list_dimension_projects_its_own_collection_and_no_other() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let card = seed_card(&store, &board, &column, "a");
    let sprint = seed_sprint(&store, &board);

    for (round, expected) in [
        (
            FetchRound {
                board_list: true,
                ..Default::default()
            },
            Observed {
                board_list: FetchStatus::Loaded,
                ..all(FetchStatus::NotLoaded)
            },
        ),
        (
            FetchRound {
                column_list: true,
                ..Default::default()
            },
            Observed {
                column_list: FetchStatus::Loaded,
                ..all(FetchStatus::NotLoaded)
            },
        ),
        (
            FetchRound {
                card_list: true,
                ..Default::default()
            },
            Observed {
                card_list: FetchStatus::Loaded,
                ..all(FetchStatus::NotLoaded)
            },
        ),
        (
            FetchRound {
                sprint_list: true,
                ..Default::default()
            },
            Observed {
                sprint_list: FetchStatus::Loaded,
                ..all(FetchStatus::NotLoaded)
            },
        ),
        (
            FetchRound {
                graph: true,
                ..Default::default()
            },
            Observed {
                graph: FetchStatus::Loaded,
                ..all(FetchStatus::NotLoaded)
            },
        ),
    ] {
        let mut cache = EntityCache::new();
        cache
            .resolve(
                &ProbePlan::new(round, column.id, card.id, sprint.id),
                &store,
            )
            .unwrap();

        let probe = ProbePlan::new(FetchRound::default(), column.id, card.id, sprint.id);
        cache.resolve(&probe, &store).unwrap();

        assert_eq!(probe.last(), expected);
    }
}

#[test]
fn test_each_by_id_dimension_projects_its_own_map_and_no_other() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let card = seed_card(&store, &board, &column, "a");
    let sprint = seed_sprint(&store, &board);

    for (round, expected) in [
        (
            FetchRound {
                columns: vec![column.id],
                ..Default::default()
            },
            Observed {
                column: FetchStatus::Loaded,
                ..all(FetchStatus::NotLoaded)
            },
        ),
        (
            FetchRound {
                cards: vec![card.id],
                ..Default::default()
            },
            Observed {
                card: FetchStatus::Loaded,
                ..all(FetchStatus::NotLoaded)
            },
        ),
        (
            FetchRound {
                sprints: vec![sprint.id],
                ..Default::default()
            },
            Observed {
                sprint: FetchStatus::Loaded,
                ..all(FetchStatus::NotLoaded)
            },
        ),
    ] {
        let mut cache = EntityCache::new();
        cache
            .resolve(
                &ProbePlan::new(round, column.id, card.id, sprint.id),
                &store,
            )
            .unwrap();

        let probe = ProbePlan::new(FetchRound::default(), column.id, card.id, sprint.id);
        cache.resolve(&probe, &store).unwrap();

        assert_eq!(probe.last(), expected);
    }
}

#[test]
fn test_an_id_never_fetched_projects_as_not_loaded_even_when_a_sibling_is_loaded() {
    let store = store();
    let (board, column) = seed_board_with_column(&store);
    let fetched = seed_card(&store, &board, &column, "a");
    let never = seed_card(&store, &board, &column, "b");
    let sprint = seed_sprint(&store, &board);
    let mut cache = EntityCache::new();
    cache
        .resolve(
            &ProbePlan::new(
                FetchRound {
                    cards: vec![fetched.id],
                    ..Default::default()
                },
                column.id,
                fetched.id,
                sprint.id,
            ),
            &store,
        )
        .unwrap();

    let probe = ProbePlan::new(FetchRound::default(), column.id, never.id, sprint.id);
    cache.resolve(&probe, &store).unwrap();

    assert_eq!(probe.last().card, FetchStatus::NotLoaded);
}

#[test]
fn test_a_missing_entity_projects_as_missing_and_a_failed_one_as_failed() {
    let store = store();
    let absent = Uuid::new_v4();
    let mut cache = EntityCache::new();
    cache
        .resolve(
            &ProbePlan::new(
                FetchRound {
                    cards: vec![absent],
                    ..Default::default()
                },
                Uuid::new_v4(),
                absent,
                Uuid::new_v4(),
            ),
            &store,
        )
        .unwrap();

    let probe = ProbePlan::new(
        FetchRound::default(),
        Uuid::new_v4(),
        absent,
        Uuid::new_v4(),
    );
    cache.resolve(&probe, &store).unwrap();

    assert_eq!(probe.last().card, FetchStatus::Missing);

    let failing = super::store();
    let id = Uuid::new_v4();
    failing.fail_card(id);
    let mut cache = EntityCache::new();
    cache
        .resolve(
            &ProbePlan::new(
                FetchRound {
                    cards: vec![id],
                    ..Default::default()
                },
                Uuid::new_v4(),
                id,
                Uuid::new_v4(),
            ),
            &failing,
        )
        .unwrap();

    let probe = ProbePlan::new(FetchRound::default(), Uuid::new_v4(), id, Uuid::new_v4());
    cache.resolve(&probe, &failing).unwrap();

    assert_eq!(probe.last().card, FetchStatus::Failed);
}
