use std::collections::HashMap;

use kanban_domain::resolved::Collection;
use kanban_domain::{
    Board, Card, Column, DependencyGraph, DerivedProjections, EntityIds, LoadState, Model,
    NoProjections, Resolved, Sprint,
};
use uuid::Uuid;

use super::*;

struct StubWorld {
    board_list: FetchStatus,
    graph: FetchStatus,
    boards: HashMap<Uuid, FetchStatus>,
    columns: HashMap<Uuid, FetchStatus>,
    cards: HashMap<Uuid, FetchStatus>,
    sprints: HashMap<Uuid, FetchStatus>,
    columns_of_board: HashMap<Uuid, FetchStatus>,
    cards_of_column: HashMap<Uuid, FetchStatus>,
    sprints_of_board: HashMap<Uuid, FetchStatus>,
    archived_card_list: FetchStatus,
    archived_cards_of_board: HashMap<Uuid, FetchStatus>,
    archived_board_list: FetchStatus,
}

impl Default for StubWorld {
    fn default() -> Self {
        StubWorld {
            board_list: FetchStatus::NotLoaded,
            graph: FetchStatus::NotLoaded,
            boards: HashMap::new(),
            columns: HashMap::new(),
            cards: HashMap::new(),
            sprints: HashMap::new(),
            columns_of_board: HashMap::new(),
            cards_of_column: HashMap::new(),
            sprints_of_board: HashMap::new(),
            archived_card_list: FetchStatus::NotLoaded,
            archived_cards_of_board: HashMap::new(),
            archived_board_list: FetchStatus::NotLoaded,
        }
    }
}

impl LoadedState for StubWorld {
    fn board_list(&self) -> FetchStatus {
        self.board_list
    }
    fn graph(&self) -> FetchStatus {
        self.graph
    }
    fn board(&self, id: Uuid) -> FetchStatus {
        self.boards
            .get(&id)
            .copied()
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn column(&self, id: Uuid) -> FetchStatus {
        self.columns
            .get(&id)
            .copied()
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn card(&self, id: Uuid) -> FetchStatus {
        self.cards
            .get(&id)
            .copied()
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn sprint(&self, id: Uuid) -> FetchStatus {
        self.sprints
            .get(&id)
            .copied()
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn columns_of_board(&self, board_id: Uuid) -> FetchStatus {
        self.columns_of_board
            .get(&board_id)
            .copied()
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn cards_of_column(&self, column_id: Uuid) -> FetchStatus {
        self.cards_of_column
            .get(&column_id)
            .copied()
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn sprints_of_board(&self, board_id: Uuid) -> FetchStatus {
        self.sprints_of_board
            .get(&board_id)
            .copied()
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn archived_card_list(&self) -> FetchStatus {
        self.archived_card_list
    }
    fn archived_cards_of_board(&self, board_id: Uuid) -> FetchStatus {
        self.archived_cards_of_board
            .get(&board_id)
            .copied()
            .unwrap_or(FetchStatus::NotLoaded)
    }
    fn archived_board_list(&self) -> FetchStatus {
        self.archived_board_list
    }
    fn board_in_collection(&self, id: Uuid) -> FetchStatus {
        self.board(id)
    }
}

impl LoadedEntities for StubWorld {
    fn loaded_columns_of_board(&self, _board_id: Uuid) -> Option<&[Column]> {
        None
    }
    fn loaded_archived_card_markers(&self) -> Option<&[kanban_domain::ArchivedCard]> {
        None
    }
    fn loaded_archived_board_markers(&self) -> Option<&[kanban_domain::ArchivedBoard]> {
        None
    }
    fn loaded_archived_cards_of_board(
        &self,
        _board_id: Uuid,
    ) -> Option<&[kanban_domain::ArchivedCard]> {
        None
    }
    fn loaded_graph_neighbours(&self, _card_id: Uuid) -> Option<Vec<Uuid>> {
        None
    }
}

fn all_loaded() -> StubWorld {
    StubWorld {
        board_list: FetchStatus::Loaded,
        graph: FetchStatus::Loaded,
        boards: HashMap::new(),
        columns: HashMap::new(),
        cards: HashMap::new(),
        sprints: HashMap::new(),
        columns_of_board: HashMap::new(),
        cards_of_column: HashMap::new(),
        sprints_of_board: HashMap::new(),
        archived_card_list: FetchStatus::Loaded,
        archived_cards_of_board: HashMap::new(),
        archived_board_list: FetchStatus::Loaded,
    }
}

#[test]
fn test_an_invalidated_card_that_was_never_loaded_is_not_requested() {
    let world = StubWorld::default();
    let a = Uuid::new_v4();

    let plan =
        InvalidationPlan::for_invalidation(&Invalidation::Entities(EntityIds::cards([a])), &world);

    assert!(plan.is_none());
}

#[test]
fn test_an_invalidated_card_that_was_loaded_is_requested_by_id() {
    let a = Uuid::new_v4();
    let world = StubWorld {
        cards: HashMap::from([(a, FetchStatus::Loaded)]),
        ..Default::default()
    };

    let plan =
        InvalidationPlan::for_invalidation(&Invalidation::Entities(EntityIds::cards([a])), &world)
            .expect("card was loaded");

    assert_eq!(plan.round().cards, vec![a]);
    assert!(plan.round().columns.is_empty());
    assert!(plan.round().sprints.is_empty());
    assert!(!plan.round().board_list);
    assert!(!plan.round().graph);
}

#[test]
fn test_a_named_board_is_re_requested_as_the_board_list() {
    let b = Uuid::new_v4();
    let world = StubWorld {
        board_list: FetchStatus::Loaded,
        ..Default::default()
    };

    let plan =
        InvalidationPlan::for_invalidation(&Invalidation::Entities(EntityIds::boards([b])), &world)
            .expect("board_list was loaded");

    assert!(plan.round().board_list);
    assert!(plan.round().columns.is_empty());
    assert!(plan.round().cards.is_empty());
    assert!(plan.round().sprints.is_empty());
    assert!(!plan.round().graph);
}

#[test]
fn test_a_prefix_only_invalidation_re_requests_a_loaded_board_list() {
    let world = StubWorld {
        board_list: FetchStatus::Loaded,
        ..Default::default()
    };

    let plan = InvalidationPlan::for_invalidation(
        &Invalidation::Entities(EntityIds::default().with_prefixes()),
        &world,
    )
    .expect("board_list was loaded");

    assert!(plan.round().board_list);
    assert!(plan.round().columns.is_empty());
    assert!(plan.round().cards.is_empty());
    assert!(plan.round().sprints.is_empty());
    assert!(!plan.round().graph);
}

#[test]
fn test_a_prefix_bump_over_an_unread_board_list_yields_no_plan() {
    let world = StubWorld::default();

    let plan = InvalidationPlan::for_invalidation(
        &Invalidation::Entities(EntityIds::default().with_prefixes()),
        &world,
    );

    assert!(plan.is_none());
}

#[test]
fn test_a_graph_invalidation_requests_the_graph_alongside_its_loaded_cards() {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let world = StubWorld {
        graph: FetchStatus::Loaded,
        cards: HashMap::from([(a, FetchStatus::Loaded), (b, FetchStatus::Loaded)]),
        ..Default::default()
    };

    let plan = InvalidationPlan::for_invalidation(
        &Invalidation::Entities(EntityIds::cards([a, b]).with_graph()),
        &world,
    )
    .expect("graph and cards were loaded");

    assert!(plan.round().graph);
    let mut expected = vec![a, b];
    expected.sort_unstable();
    assert_eq!(plan.round().cards, expected);
}

#[test]
fn test_only_the_loaded_ids_of_a_mixed_invalidation_are_requested() {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let world = StubWorld {
        cards: HashMap::from([(a, FetchStatus::Loaded), (b, FetchStatus::NotLoaded)]),
        ..Default::default()
    };

    let plan = InvalidationPlan::for_invalidation(
        &Invalidation::Entities(EntityIds::cards([a, b])),
        &world,
    )
    .expect("card a was loaded");

    assert_eq!(plan.round().cards, vec![a]);
}

#[test]
fn test_a_failed_board_list_is_still_re_requested_because_it_was_read() {
    let b = Uuid::new_v4();
    let world = StubWorld {
        board_list: FetchStatus::Failed,
        ..Default::default()
    };

    let plan =
        InvalidationPlan::for_invalidation(&Invalidation::Entities(EntityIds::boards([b])), &world)
            .expect("board_list was read, even though it failed");

    assert!(plan.round().board_list);
}

#[test]
fn test_a_missing_card_id_is_still_re_requested_because_it_was_read() {
    let a = Uuid::new_v4();
    let world = StubWorld {
        cards: HashMap::from([(a, FetchStatus::Missing)]),
        ..Default::default()
    };

    let plan =
        InvalidationPlan::for_invalidation(&Invalidation::Entities(EntityIds::cards([a])), &world)
            .expect("card a was read, even though it came back missing");

    assert_eq!(plan.round().cards, vec![a]);
}

#[test]
fn test_the_plan_never_emits_a_scoped_tier() {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let board = Uuid::new_v4();
    let column = Uuid::new_v4();
    let sprint = Uuid::new_v4();

    let shapes = vec![
        EntityIds::boards([board]),
        EntityIds::columns([column]),
        EntityIds::cards([a, b]),
        EntityIds::sprints([sprint]),
        EntityIds::default().with_graph(),
        EntityIds::default().with_prefixes(),
        EntityIds {
            boards: [board].into(),
            columns: [column].into(),
            cards: [a, b].into(),
            sprints: [sprint].into(),
            graph: true,
            prefixes: true,
        },
    ];

    for ids in shapes {
        let world = StubWorld {
            columns_of_board: HashMap::from([(board, FetchStatus::Loaded)]),
            cards_of_column: HashMap::from([(column, FetchStatus::Loaded)]),
            sprints_of_board: HashMap::from([(board, FetchStatus::Loaded)]),
            archived_cards_of_board: HashMap::from([(board, FetchStatus::Loaded)]),
            ..all_loaded()
        };
        let Some(plan) = InvalidationPlan::for_invalidation(&Invalidation::Entities(ids), &world)
        else {
            continue;
        };

        assert!(plan.round().columns_by_board.is_empty());
        assert!(plan.round().cards_by_column.is_empty());
        assert!(plan.round().sprints_by_board.is_empty());
        assert!(plan.round().archived_cards_by_board.is_empty());

        let next = plan.next_round(&world);
        assert!(next.columns_by_board.is_empty());
        assert!(next.cards_by_column.is_empty());
        assert!(next.sprints_by_board.is_empty());
        assert!(next.archived_cards_by_board.is_empty());
    }
}

#[test]
fn test_an_empty_entities_invalidation_yields_no_plan() {
    let world = all_loaded();

    let plan =
        InvalidationPlan::for_invalidation(&Invalidation::Entities(EntityIds::default()), &world);

    assert!(plan.is_none());
}

#[test]
fn test_empty_entities_invalidation_plans_nothing_and_wipes_nothing() {
    let board = Board::new("B", None::<String>);
    let column = Column::new(board.id, "A", 0);
    let card = Card::new(board.id, column.id, "task", 0);
    let sprint = Sprint::new(board.id, 1, None, None::<String>);

    let mut model = Model::default();
    let changed = model.apply_resolved(Resolved {
        boards: Collection {
            all: LoadState::Loaded(vec![board.clone()]),
            ..Default::default()
        },
        columns: Collection {
            by_id: [(column.id, LoadState::Loaded(column.clone()))].into(),
            by_parent: [(board.id, LoadState::Loaded(vec![column.clone()]))].into(),
            ..Default::default()
        },
        cards: Collection {
            by_parent: [(column.id, LoadState::Loaded(vec![card.clone()]))].into(),
            ..Default::default()
        },
        sprints: Collection {
            by_parent: [(board.id, LoadState::Loaded(vec![sprint.clone()]))].into(),
            ..Default::default()
        },
        graph: LoadState::Loaded(DependencyGraph::default()),
        ..Default::default()
    });
    NoProjections.resync(&model, changed);

    let inv = Invalidation::Entities(EntityIds::default());
    let plan_is_none = InvalidationPlan::for_invalidation(&inv, &model).is_none();
    assert!(plan_is_none);

    let _ = model.invalidate(inv);

    assert_eq!(LoadedState::board_list(&model), FetchStatus::Loaded);
    assert!(model.board_columns_state(board.id).is_loaded());
    assert!(model.column_cards_state(column.id).is_loaded());
    assert!(model.board_sprints_state(board.id).is_loaded());
    assert_eq!(LoadedState::graph(&model), FetchStatus::Loaded);
    assert_eq!(LoadedState::column(&model, column.id), FetchStatus::Loaded);
}

#[test]
fn test_invalidation_plan_for_all_re_requests_every_surviving_tier_that_was_read() {
    let world = StubWorld {
        board_list: FetchStatus::Loaded,
        graph: FetchStatus::Loaded,
        archived_card_list: FetchStatus::Loaded,
        archived_board_list: FetchStatus::Loaded,
        ..Default::default()
    };

    let plan = InvalidationPlan::for_invalidation(&Invalidation::All, &world)
        .expect("everything was loaded");

    let round = plan.round();
    assert!(round.board_list);
    assert!(round.graph);
    assert!(round.archived_card_list);
    assert!(round.archived_board_list);
    assert!(round.columns.is_empty());
    assert!(round.cards.is_empty());
    assert!(round.sprints.is_empty());
    assert!(round.columns_by_board.is_empty());
    assert!(round.cards_by_column.is_empty());
    assert!(round.sprints_by_board.is_empty());
    assert!(round.archived_cards_by_board.is_empty());
}

#[test]
fn test_entities_plan_rerequests_archived_card_list_when_model_had_read_it() {
    let a = Uuid::new_v4();
    let world = StubWorld {
        cards: HashMap::from([(a, FetchStatus::Loaded)]),
        archived_card_list: FetchStatus::Loaded,
        ..Default::default()
    };

    let plan =
        InvalidationPlan::for_invalidation(&Invalidation::Entities(EntityIds::cards([a])), &world)
            .expect("card a was loaded");

    assert_eq!(plan.round().cards, vec![a]);
    assert!(plan.round().archived_card_list);
    assert!(!plan.round().archived_board_list);
}

#[test]
fn test_entities_plan_rerequests_archived_board_list_when_model_had_read_it() {
    let b = Uuid::new_v4();
    let world = StubWorld {
        board_list: FetchStatus::Loaded,
        archived_board_list: FetchStatus::Loaded,
        ..Default::default()
    };

    let plan =
        InvalidationPlan::for_invalidation(&Invalidation::Entities(EntityIds::boards([b])), &world)
            .expect("board_list was loaded");

    assert!(plan.round().board_list);
    assert!(plan.round().archived_board_list);
    assert!(!plan.round().archived_card_list);
}

#[test]
fn test_entities_plan_rerequests_archived_board_list_on_a_prefix_only_invalidation() {
    let world = StubWorld {
        board_list: FetchStatus::Loaded,
        archived_board_list: FetchStatus::Loaded,
        ..Default::default()
    };

    let plan = InvalidationPlan::for_invalidation(
        &Invalidation::Entities(EntityIds::default().with_prefixes()),
        &world,
    )
    .expect("board_list was loaded");

    assert!(plan.round().archived_board_list);
}

#[test]
fn test_entities_plan_skips_archived_card_list_when_never_read() {
    let a = Uuid::new_v4();
    let world = StubWorld {
        cards: HashMap::from([(a, FetchStatus::Loaded)]),
        ..Default::default()
    };

    let plan =
        InvalidationPlan::for_invalidation(&Invalidation::Entities(EntityIds::cards([a])), &world)
            .expect("card a was loaded");

    assert!(!plan.round().archived_card_list);
}

#[test]
fn test_entities_plan_skips_archived_board_list_when_never_read() {
    let b = Uuid::new_v4();
    let world = StubWorld {
        board_list: FetchStatus::Loaded,
        ..Default::default()
    };

    let plan =
        InvalidationPlan::for_invalidation(&Invalidation::Entities(EntityIds::boards([b])), &world)
            .expect("board_list was loaded");

    assert!(!plan.round().archived_board_list);
}

#[test]
fn test_next_round_is_empty_once_everything_it_asked_for_is_loaded() {
    let a = Uuid::new_v4();
    let world = StubWorld {
        cards: HashMap::from([(a, FetchStatus::Loaded)]),
        ..Default::default()
    };

    let plan =
        InvalidationPlan::for_invalidation(&Invalidation::Entities(EntityIds::cards([a])), &world)
            .expect("card a was loaded");

    let still_missing = StubWorld {
        cards: HashMap::from([(a, FetchStatus::NotLoaded)]),
        ..Default::default()
    };
    assert_eq!(plan.next_round(&still_missing).cards, vec![a]);

    let now_loaded = StubWorld {
        cards: HashMap::from([(a, FetchStatus::Loaded)]),
        ..Default::default()
    };
    assert!(plan.next_round(&now_loaded).is_empty());
}

#[test]
fn test_a_multi_id_round_is_deterministic() {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    let world = all_loaded_with_cards(&[a, b, c]);

    let plan_one = InvalidationPlan::for_invalidation(
        &Invalidation::Entities(EntityIds::cards([a, b, c])),
        &world,
    )
    .expect("cards were loaded");
    let plan_two = InvalidationPlan::for_invalidation(
        &Invalidation::Entities(EntityIds::cards([c, b, a])),
        &world,
    )
    .expect("cards were loaded");

    let mut expected = vec![a, b, c];
    expected.sort_unstable();
    assert_eq!(plan_one.round().cards, expected);
    assert_eq!(plan_one.round().cards, plan_two.round().cards);
}

fn all_loaded_with_cards(ids: &[Uuid]) -> StubWorld {
    StubWorld {
        cards: ids.iter().map(|id| (*id, FetchStatus::Loaded)).collect(),
        ..all_loaded()
    }
}

#[test]
fn test_invalidate_all_over_seeded_flat_collections_issues_no_flat_list_read() {
    let board = Board::new("B", None::<String>);
    let column = Column::new(board.id, "A", 0);
    let card = Card::new(board.id, column.id, "task", 0);
    let sprint = Sprint::new(board.id, 1, None, None::<String>);

    let mut model = Model::default();
    let changed = model.apply_resolved(Resolved {
        boards: Collection {
            all: LoadState::Loaded(vec![board.clone()]),
            ..Default::default()
        },
        columns: Collection {
            by_parent: [(board.id, LoadState::Loaded(Vec::new()))].into(),
            ..Default::default()
        },
        cards: Collection {
            by_parent: [(column.id, LoadState::Loaded(vec![card.clone()]))].into(),
            ..Default::default()
        },
        sprints: Collection {
            by_parent: [(board.id, LoadState::Loaded(Vec::new()))].into(),
            ..Default::default()
        },
        ..Default::default()
    });
    NoProjections.resync(&model, changed);

    let plan = InvalidationPlan::for_invalidation(&Invalidation::All, &model)
        .expect("board_list was loaded");
    let _ = model.invalidate(Invalidation::All);

    let store = crate::read_recorder::RecordingStore::new();
    kanban_domain::DataStore::upsert_board(&store, board.clone()).unwrap();
    kanban_domain::DataStore::upsert_column(&store, column.clone()).unwrap();
    kanban_domain::DataStore::upsert_card(&store, card.clone()).unwrap();
    kanban_domain::DataStore::upsert_sprint(&store, sprint.clone()).unwrap();

    crate::resolve::resolve(&plan, &model, &store);

    let ops = store.ops();
    let ops = ops.lock().unwrap();
    assert!(
        !ops.iter().any(|op| matches!(
            op.method,
            "list_all_columns" | "list_all_cards" | "list_all_sprints"
        )),
        "no plan may drive a whole-workspace list read; ops were: {ops:?}"
    );
}
