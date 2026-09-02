use super::super::fault::faultable;
use super::super::BackendFactory;
use super::assert_card_eq;
use crate::fetch_plan::{requestable, FetchPlan, FetchRound, LoadedEntities};
use crate::KanbanContext;
use kanban_core::graph::Edge as _;
use kanban_core::AppConfig;
use kanban_domain::{
    CardUpdate, DependencyGraph, DerivedProjections, EntityIds, Invalidation, KanbanOperations,
    Model, NoProjections, Resolved, Severity,
};
use tempfile::TempDir;
use uuid::Uuid;

/// A plan that always requests the same fixed round, regardless of what has
/// already loaded. `resolve` still terminates in one pass, because a round
/// is narrowed against what this call has already fetched before it is
/// dispatched, and a constant round narrows to empty on the second pass.
pub struct StaticPlan(pub FetchRound);

impl FetchPlan for StaticPlan {
    fn next_round(&self, _loaded: &dyn LoadedEntities) -> FetchRound {
        self.0.clone()
    }
}

fn apply(model: &mut Model, resolved: Resolved) {
    let changed = model.apply_resolved(resolved);
    NoProjections.resync(model, changed);
}

pub async fn test_an_absent_card_resolves_missing_not_failed(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let model = Model::default();
    let ghost = Uuid::new_v4();

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            cards: vec![ghost],
            ..Default::default()
        }),
        &model,
    );

    let state = resolved
        .cards
        .by_id
        .get(&ghost)
        .expect("ghost card requested");
    assert!(state.is_missing());
    assert!(!state.is_failed());

    let mut model = model;
    apply(&mut model, resolved);
}

pub async fn test_an_absent_column_resolves_missing_not_failed(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let model = Model::default();
    let ghost = Uuid::new_v4();

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            columns: vec![ghost],
            ..Default::default()
        }),
        &model,
    );

    let state = resolved
        .columns
        .by_id
        .get(&ghost)
        .expect("ghost column requested");
    assert!(state.is_missing());
    assert!(!state.is_failed());

    let mut model = model;
    apply(&mut model, resolved);
}

pub async fn test_an_absent_sprint_resolves_missing_not_failed(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let model = Model::default();
    let ghost = Uuid::new_v4();

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            sprints: vec![ghost],
            ..Default::default()
        }),
        &model,
    );

    let state = resolved
        .sprints
        .by_id
        .get(&ghost)
        .expect("ghost sprint requested");
    assert!(state.is_missing());
    assert!(!state.is_failed());

    let mut model = model;
    apply(&mut model, resolved);
}

pub async fn test_a_deleted_card_resolves_missing_on_a_second_resolve(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let mut model = Model::default();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();

    let plan_with_list = StaticPlan(FetchRound {
        card_list: true,
        cards: vec![card.id],
        ..Default::default()
    });

    let resolved = ctx.resolve(&plan_with_list, &model);
    assert!(resolved
        .cards
        .by_id
        .get(&card.id)
        .expect("card requested")
        .is_loaded());
    apply(&mut model, resolved);
    assert!(model.card_by_id_state(card.id).is_loaded());
    let cards = model.cards_state().loaded().expect("card list loaded");
    assert!(cards.iter().any(|c| c.id == card.id));

    let _invalidation = ctx.delete_card_impl(card.id).unwrap();

    let plan_by_id_only = StaticPlan(FetchRound {
        cards: vec![card.id],
        ..Default::default()
    });

    let resolved2 = ctx.resolve(&plan_by_id_only, &model);
    assert!(resolved2
        .cards
        .by_id
        .get(&card.id)
        .expect("card requested again")
        .is_missing());
    apply(&mut model, resolved2);
    assert!(model.card_by_id_state(card.id).is_missing());
    let cards = model
        .cards_state()
        .loaded()
        .expect("card list still loaded after the delete");
    assert!(!cards.iter().any(|c| c.id == card.id));
}

pub async fn test_a_backend_read_error_resolves_failed_not_missing(factory: BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (factory, handles) = faultable(factory);
    let backend = factory(&path);
    let handle = handles.lock().unwrap()[&path].last().unwrap().clone();
    let mut ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let model = Model::default();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();

    handle.fail("get_card");

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            cards: vec![card.id],
            ..Default::default()
        }),
        &model,
    );

    let state = resolved.cards.by_id.get(&card.id).expect("card requested");
    assert!(state.is_failed());
    assert!(!state.is_missing());
}

pub async fn test_a_backend_list_error_resolves_the_list_failed_not_empty(factory: BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (factory, handles) = faultable(factory);
    let backend = factory(&path);
    let handle = handles.lock().unwrap()[&path].last().unwrap().clone();
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let model = Model::default();

    handle.fail("list_all_cards");

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            card_list: true,
            ..Default::default()
        }),
        &model,
    );

    assert!(resolved.cards.all.is_failed());
    assert!(!resolved.cards.all.is_loaded());
}

pub async fn test_a_backend_list_error_resolves_the_column_list_failed_not_empty(
    factory: BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (factory, handles) = faultable(factory);
    let backend = factory(&path);
    let handle = handles.lock().unwrap()[&path].last().unwrap().clone();
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let model = Model::default();

    handle.fail("list_all_columns");

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            column_list: true,
            ..Default::default()
        }),
        &model,
    );

    assert!(resolved.columns.all.is_failed());
    assert!(!resolved.columns.all.is_loaded());
}

pub async fn test_a_backend_list_error_resolves_the_sprint_list_failed_not_empty(
    factory: BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (factory, handles) = faultable(factory);
    let backend = factory(&path);
    let handle = handles.lock().unwrap()[&path].last().unwrap().clone();
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let model = Model::default();

    handle.fail("list_all_sprints");

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            sprint_list: true,
            ..Default::default()
        }),
        &model,
    );

    assert!(resolved.sprints.all.is_failed());
    assert!(!resolved.sprints.all.is_loaded());
}

pub async fn test_a_failed_read_is_retried_on_the_next_resolve(factory: BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (factory, handles) = faultable(factory);
    let backend = factory(&path);
    let handle = handles.lock().unwrap()[&path].last().unwrap().clone();
    let mut ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let mut model = Model::default();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();

    handle.fail("get_card");

    let plan = StaticPlan(FetchRound {
        cards: vec![card.id],
        ..Default::default()
    });

    let resolved = ctx.resolve(&plan, &model);
    assert!(resolved
        .cards
        .by_id
        .get(&card.id)
        .expect("card requested")
        .is_failed());
    apply(&mut model, resolved);

    handle.clear_faults();
    handle.clear_ops();

    let resolved2 = ctx.resolve(&plan, &model);
    assert!(resolved2
        .cards
        .by_id
        .get(&card.id)
        .expect("card requested again")
        .is_loaded());
    assert_eq!(handle.op_count("get_card"), 1);
}

pub async fn test_a_missing_read_is_not_retried_on_the_next_resolve(factory: BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (factory, handles) = faultable(factory);
    let backend = factory(&path);
    let handle = handles.lock().unwrap()[&path].last().unwrap().clone();
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let mut model = Model::default();
    let ghost = Uuid::new_v4();

    let plan = StaticPlan(FetchRound {
        cards: vec![ghost],
        ..Default::default()
    });

    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    assert!(model.card_by_id_state(ghost).is_missing());

    handle.clear_ops();
    handle.fail("get_card");

    let resolved2 = ctx.resolve(&plan, &model);
    assert!(!resolved2.cards.by_id.contains_key(&ghost));
    apply(&mut model, resolved2);

    assert!(model.card_by_id_state(ghost).is_missing());
    assert_eq!(handle.op_count("get_card"), 0);
}

/// Gated on the per-id loaded status, like `ScopedCardsPlan` but for the
/// by-id tier.
pub struct CardsByIdPlan(pub Vec<Uuid>);

impl FetchPlan for CardsByIdPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            cards: self
                .0
                .iter()
                .copied()
                .filter(|&id| requestable(loaded.card(id)))
                .collect(),
            ..Default::default()
        }
    }
}

pub struct ScopedCardsPlan(pub Vec<Uuid>);

impl FetchPlan for ScopedCardsPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            cards_by_column: self
                .0
                .iter()
                .copied()
                .filter(|&id| requestable(loaded.cards_of_column(id)))
                .collect(),
            ..Default::default()
        }
    }
}

pub async fn test_a_card_moved_between_columns_reads_correctly_after_invalidation_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let mut model = Model::default();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let source = ctx.create_column(board.id, "Source".into(), None).unwrap();
    let dest = ctx.create_column(board.id, "Dest".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            source.id,
            "Card".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();

    let plan = ScopedCardsPlan(vec![source.id, dest.id]);
    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);

    assert!(model
        .column_cards_state(source.id)
        .loaded()
        .copied()
        .unwrap_or(&[])
        .iter()
        .any(|c| c.id == card.id));
    assert!(model
        .column_cards_state(dest.id)
        .loaded()
        .copied()
        .unwrap_or(&[])
        .is_empty());

    let (_moved, invalidation): (_, Invalidation) =
        ctx.move_card_impl(card.id, dest.id, None).unwrap();
    let changed = model.invalidate(invalidation);
    NoProjections.resync(&model, changed);

    assert!(model.column_cards_state(source.id).is_not_loaded());
    assert!(model.column_cards_state(dest.id).is_not_loaded());

    let resolved2 = ctx.resolve(&plan, &model);
    apply(&mut model, resolved2);

    let in_dest = model.column_cards_state(dest.id).loaded().copied().unwrap();
    assert!(in_dest.iter().any(|c| c.id == card.id));
    let in_source = model
        .column_cards_state(source.id)
        .loaded()
        .copied()
        .unwrap();
    assert!(!in_source.iter().any(|c| c.id == card.id));
}

/// Gated on the loaded scope so a still-`Loaded` tier stops the refetch.
/// `resolve` narrows a scoped round only against what the current call
/// already fetched, never against `loaded`, so an ungated plan would
/// refetch unconditionally and hide a missed invalidation.
pub struct ArchivedByBoardPlan(pub Uuid);

impl FetchPlan for ArchivedByBoardPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        if requestable(loaded.archived_cards_of_board(self.0)) {
            FetchRound {
                archived_cards_by_board: vec![self.0],
                ..Default::default()
            }
        } else {
            FetchRound::default()
        }
    }
}

pub async fn test_a_boards_archived_cards_are_scoped_to_that_board_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let a = ctx.create_board("A".into(), Some("A".into())).unwrap();
    let b = ctx.create_board("B".into(), Some("B".into())).unwrap();
    let col_a = ctx.create_column(a.id, "Col".into(), None).unwrap();
    let col_b = ctx.create_column(b.id, "Col".into(), None).unwrap();
    let card_a = ctx
        .create_card(
            a.id,
            col_a.id,
            "Card A".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let card_b = ctx
        .create_card(
            b.id,
            col_b.id,
            "Card B".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let _ = ctx.archive_card_impl(card_a.id).unwrap();
    let _ = ctx.archive_card_impl(card_b.id).unwrap();

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            archived_cards_by_board: vec![a.id],
            ..Default::default()
        }),
        &Model::default(),
    );

    let scope = resolved
        .archived_cards
        .by_parent
        .get(&a.id)
        .expect("board a's scope requested");
    let markers = scope.loaded().expect("board a's scope loaded");
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].entity_id, card_a.id);
    assert!(!resolved.archived_cards.by_parent.contains_key(&b.id));

    let mut model = Model::default();
    apply(&mut model, resolved);

    let loaded_a = model
        .board_archived_cards_state(a.id)
        .loaded()
        .copied()
        .unwrap();
    assert_eq!(loaded_a.len(), 1);
    assert!(model.board_archived_cards_state(b.id).is_not_loaded());
}

pub async fn test_an_archived_card_restored_then_reread_is_absent_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let mut model = Model::default();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            col.id,
            "Card".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();

    let plan = ArchivedByBoardPlan(board.id);
    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    assert!(model
        .board_archived_cards_state(board.id)
        .loaded()
        .copied()
        .unwrap()
        .is_empty());

    let ((), inv) = ctx.archive_card_impl(card.id).unwrap();
    let changed = model.invalidate(inv);
    NoProjections.resync(&model, changed);

    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    let markers = model
        .board_archived_cards_state(board.id)
        .loaded()
        .copied()
        .unwrap();
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].entity_id, card.id);

    let (_card, inv) = ctx.restore_card_impl(card.id, None).unwrap();
    let changed = model.invalidate(inv);
    NoProjections.resync(&model, changed);

    assert!(model.board_archived_cards_state(board.id).is_not_loaded());

    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    let markers = model
        .board_archived_cards_state(board.id)
        .loaded()
        .copied()
        .unwrap();
    assert!(markers.is_empty());
}

pub async fn test_scoped_resolve_returns_the_same_graph_on_every_backend(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let col_a = ctx.create_column(board.id, "A".into(), None).unwrap();
    let col_b = ctx.create_column(board.id, "B".into(), None).unwrap();
    let card_a1 = ctx
        .create_card(
            board.id,
            col_a.id,
            "A1".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let card_a2 = ctx
        .create_card(
            board.id,
            col_a.id,
            "A2".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let card_b1 = ctx
        .create_card(
            board.id,
            col_b.id,
            "B1".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let sprint = ctx
        .create_sprint(board.id, Some("SPR".into()), Some("Sprint".into()))
        .unwrap();
    let archived = ctx
        .create_card(
            board.id,
            col_a.id,
            "A-archived".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let _ = ctx.archive_card_impl(archived.id).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            columns_by_board: vec![board.id],
            cards_by_column: vec![col_a.id, col_b.id],
            sprints_by_board: vec![board.id],
            ..Default::default()
        }),
        &Model::default(),
    );

    let columns_state = resolved
        .columns
        .by_parent
        .get(&board.id)
        .expect("board's columns requested");
    assert!(columns_state.is_loaded());
    let columns = columns_state.loaded().expect("columns loaded");
    assert_eq!(
        columns.iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![col_a.id, col_b.id]
    );

    let col_a_cards_state = resolved
        .cards
        .by_parent
        .get(&col_a.id)
        .expect("col_a's cards requested");
    assert!(col_a_cards_state.is_loaded());
    let col_a_cards = col_a_cards_state.loaded().expect("col_a cards loaded");
    let mut col_a_ids: Vec<Uuid> = col_a_cards.iter().map(|c| c.id).collect();
    col_a_ids.sort();
    let mut expected_a_ids = vec![card_a1.id, card_a2.id];
    expected_a_ids.sort();
    assert_eq!(col_a_ids, expected_a_ids);
    assert!(!col_a_cards.iter().any(|c| c.id == archived.id));

    let col_b_cards_state = resolved
        .cards
        .by_parent
        .get(&col_b.id)
        .expect("col_b's cards requested");
    assert!(col_b_cards_state.is_loaded());
    let col_b_cards = col_b_cards_state.loaded().expect("col_b cards loaded");
    assert_eq!(
        col_b_cards.iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![card_b1.id]
    );

    let sprints_state = resolved
        .sprints
        .by_parent
        .get(&board.id)
        .expect("board's sprints requested");
    assert!(sprints_state.is_loaded());
    let sprints = sprints_state.loaded().expect("sprints loaded");
    assert!(sprints.iter().any(|s| s.id == sprint.id));
}

pub async fn test_a_scope_on_an_unknown_parent_is_loaded_and_empty_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let unknown = Uuid::new_v4();

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            columns_by_board: vec![unknown],
            ..Default::default()
        }),
        &Model::default(),
    );

    let state = resolved
        .columns
        .by_parent
        .get(&unknown)
        .expect("unknown board's scope requested");
    assert!(state.is_loaded());
    assert!(state.loaded().expect("loaded").is_empty());
}

pub async fn test_a_failed_scoped_read_is_failed_not_empty_on_every_backend(
    factory: BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (factory, handles) = faultable(factory);
    let backend = factory(&path);
    let handle = handles.lock().unwrap()[&path].last().unwrap().clone();
    let mut ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let _column = ctx.create_column(board.id, "Col".into(), None).unwrap();

    handle.fail("list_columns_by_board");

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            columns_by_board: vec![board.id],
            ..Default::default()
        }),
        &Model::default(),
    );

    let state = resolved
        .columns
        .by_parent
        .get(&board.id)
        .expect("board's columns requested");
    assert!(state.is_failed());
    assert!(!state.is_loaded());
}

pub async fn test_a_card_resolved_by_id_matches_the_same_card_in_the_card_list(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let sprint = ctx
        .create_sprint(board.id, Some("SPR".into()), Some("Sprint".into()))
        .unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let (card, _inv) = ctx.assign_card_to_sprint_impl(card.id, sprint.id).unwrap();
    assert!(!card.sprint_logs.is_empty(), "assignment logs the sprint");

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            card_list: true,
            cards: vec![card.id],
            ..Default::default()
        }),
        &Model::default(),
    );

    let by_id = resolved
        .cards
        .by_id
        .get(&card.id)
        .expect("card requested")
        .loaded()
        .expect("card loaded");
    let in_list = resolved
        .cards
        .all
        .loaded()
        .expect("card list loaded")
        .iter()
        .find(|c| c.id == card.id)
        .expect("card present in the resolved list");
    assert_card_eq(by_id, in_list);
}

pub async fn test_a_column_resolved_by_id_matches_the_same_column_in_the_column_list(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), Some(3)).unwrap();

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            column_list: true,
            columns: vec![column.id],
            ..Default::default()
        }),
        &Model::default(),
    );

    let by_id = resolved
        .columns
        .by_id
        .get(&column.id)
        .expect("column requested")
        .loaded()
        .expect("column loaded");
    let in_list = resolved
        .columns
        .all
        .loaded()
        .expect("column list loaded")
        .iter()
        .find(|c| c.id == column.id)
        .expect("column present in the resolved list");
    assert_eq!(by_id.id, in_list.id, "column id");
    assert_eq!(by_id.board_id, in_list.board_id, "column board_id");
    assert_eq!(by_id.name, in_list.name, "column name");
    assert_eq!(by_id.position, in_list.position, "column position");
    assert_eq!(by_id.wip_limit, in_list.wip_limit, "column wip_limit");
    assert_eq!(
        by_id.default_status, in_list.default_status,
        "column default_status"
    );
}

pub async fn test_a_sprint_resolved_by_id_matches_the_same_sprint_in_the_sprint_list(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let sprint = ctx
        .create_sprint(board.id, Some("SPR".into()), Some("Sprint".into()))
        .unwrap();

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            sprint_list: true,
            sprints: vec![sprint.id],
            ..Default::default()
        }),
        &Model::default(),
    );

    let by_id = resolved
        .sprints
        .by_id
        .get(&sprint.id)
        .expect("sprint requested")
        .loaded()
        .expect("sprint loaded");
    let in_list = resolved
        .sprints
        .all
        .loaded()
        .expect("sprint list loaded")
        .iter()
        .find(|s| s.id == sprint.id)
        .expect("sprint present in the resolved list");
    assert_eq!(by_id.id, in_list.id, "sprint id");
    assert_eq!(by_id.board_id, in_list.board_id, "sprint board_id");
    assert_eq!(by_id.status, in_list.status, "sprint status");
    assert_eq!(by_id.start_date, in_list.start_date, "sprint start_date");
    assert_eq!(by_id.end_date, in_list.end_date, "sprint end_date");
}

pub async fn test_an_archived_card_resolves_loaded_by_id_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let _ = ctx.archive_card_impl(card.id).unwrap();

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            cards: vec![card.id],
            ..Default::default()
        }),
        &Model::default(),
    );

    let state = resolved.cards.by_id.get(&card.id).expect("card requested");
    assert!(state.is_loaded());
}

pub async fn test_an_archived_card_is_absent_from_the_resolved_card_list_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let _ = ctx.archive_card_impl(card.id).unwrap();

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            card_list: true,
            ..Default::default()
        }),
        &Model::default(),
    );

    let cards = resolved.cards.all.loaded().expect("card list loaded");
    assert!(!cards.iter().any(|c| c.id == card.id));
}

pub async fn test_a_restored_card_reappears_in_the_resolved_card_list(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let mut model = Model::default();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let sprint = ctx
        .create_sprint(board.id, Some("SPR".into()), Some("Sprint".into()))
        .unwrap();
    let other = ctx
        .create_card(
            board.id,
            column.id,
            "Other".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let (card, _inv) = ctx.assign_card_to_sprint_impl(card.id, sprint.id).unwrap();
    let inv = ctx.block_impl(other.id, card.id, Severity::High).unwrap();
    let changed = model.invalidate(inv);
    NoProjections.resync(&model, changed);

    let plan = StaticPlan(FetchRound {
        card_list: true,
        cards: vec![card.id],
        graph: true,
        ..Default::default()
    });
    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    let before_state = model.card_by_id_state(card.id);
    let before = before_state
        .loaded()
        .copied()
        .expect("card loaded before archive")
        .clone();

    let ((), inv) = ctx.archive_card_impl(card.id).unwrap();
    let changed = model.invalidate(inv);
    NoProjections.resync(&model, changed);

    let (_restored, inv) = ctx.restore_card_impl(card.id, None).unwrap();
    let changed = model.invalidate(inv);
    NoProjections.resync(&model, changed);

    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);

    let after_list = model.cards_state().loaded().expect("card list loaded");
    assert!(after_list.iter().any(|c| c.id == card.id));
    let after_state = model.card_by_id_state(card.id);
    let after = after_state.loaded().expect("card loaded after restore");
    let expected = kanban_domain::Card {
        updated_at: after.updated_at,
        ..before
    };
    assert_card_eq(&expected, after);

    let graph = model.graph_state().loaded().expect("graph loaded");
    assert!(graph.blocked(other.id).contains(&card.id));
}

pub async fn test_a_deleted_then_undone_card_is_resolvable_again_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let mut model = Model::default();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();

    let plan = StaticPlan(FetchRound {
        card_list: true,
        cards: vec![card.id],
        ..Default::default()
    });
    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    let before_state = model.card_by_id_state(card.id);
    let before = before_state.loaded().copied().expect("card loaded").clone();

    let inv = ctx.delete_card_impl(card.id).unwrap();
    let changed = model.invalidate(inv);
    NoProjections.resync(&model, changed);
    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    assert!(model.card_by_id_state(card.id).is_missing());

    let inv = ctx.undo().unwrap().expect("undo produced an invalidation");
    let changed = model.invalidate(inv);
    NoProjections.resync(&model, changed);

    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);

    let after_state = model.card_by_id_state(card.id);
    let after = after_state
        .loaded()
        .copied()
        .expect("card resolvable again after undo");
    assert_card_eq(&before, after);
    let list = model.cards_state().loaded().expect("card list loaded");
    assert!(list.iter().any(|c| c.id == card.id));
}

pub async fn test_a_backend_graph_error_resolves_failed_not_an_empty_graph(
    factory: BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (factory, handles) = faultable(factory);
    let backend = factory(&path);
    let handle = handles.lock().unwrap()[&path].last().unwrap().clone();
    let ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();

    handle.fail("get_graph");

    let resolved = ctx.resolve(
        &StaticPlan(FetchRound {
            graph: true,
            ..Default::default()
        }),
        &Model::default(),
    );

    assert!(resolved.graph.is_failed());
    assert!(!resolved.graph.is_loaded());
}

pub async fn test_the_resolved_graph_carries_edges_with_an_archived_endpoint_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let mut model = Model::default();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let a = ctx
        .create_card(
            board.id,
            column.id,
            "A".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let b = ctx
        .create_card(
            board.id,
            column.id,
            "B".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let inv = ctx.block_impl(a.id, b.id, Severity::Medium).unwrap();
    let changed = model.invalidate(inv);
    NoProjections.resync(&model, changed);

    let plan = StaticPlan(FetchRound {
        graph: true,
        ..Default::default()
    });
    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    let graph_state = model.graph_state();
    let graph = graph_state.loaded().expect("graph loaded");
    assert!(graph.blocked(a.id).contains(&b.id));
    assert!(graph
        .blocks_edges()
        .iter()
        .any(|e| e.source() == a.id && e.target() == b.id && e.is_active()));

    let ((), inv) = ctx.archive_card_impl(b.id).unwrap();
    let changed = model.invalidate(inv);
    NoProjections.resync(&model, changed);

    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    let graph_state = model.graph_state();
    let graph = graph_state.loaded().expect("graph loaded after archive");
    assert!(
        graph
            .blocks_edges()
            .iter()
            .any(|e| e.source() == a.id && e.target() == b.id),
        "an edge to an archived card must still be present in the graph, even though \
         `blocked`'s active-only query no longer surfaces it"
    );

    let (_restored, inv) = ctx.restore_card_impl(b.id, None).unwrap();
    let changed = model.invalidate(inv);
    NoProjections.resync(&model, changed);

    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    let graph_state = model.graph_state();
    let graph = graph_state.loaded().expect("graph loaded after restore");
    assert!(graph.blocked(a.id).contains(&b.id));
    assert!(graph
        .blocks_edges()
        .iter()
        .any(|e| e.source() == a.id && e.target() == b.id && e.is_active()));
}

pub async fn test_a_resolve_after_a_reopen_sees_the_committed_write_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "Original".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let _ = ctx
        .update_card_impl(
            card.id,
            CardUpdate {
                title: Some("Renamed".into()),
                ..Default::default()
            },
        )
        .unwrap();
    ctx.save().await.unwrap();
    drop(ctx);

    let reopened_ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let resolved = reopened_ctx.resolve(
        &StaticPlan(FetchRound {
            cards: vec![card.id],
            ..Default::default()
        }),
        &Model::default(),
    );

    let read = resolved
        .cards
        .by_id
        .get(&card.id)
        .expect("card requested")
        .loaded()
        .expect("card loaded on reopen");
    assert_eq!(read.title, "Renamed");
}

pub async fn test_a_committed_batch_makes_the_next_resolve_read_through(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let mut model = Model::default();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "Original".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();

    let plan = StaticPlan(FetchRound {
        cards: vec![card.id],
        ..Default::default()
    });
    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    assert_eq!(
        model
            .card_by_id_state(card.id)
            .loaded()
            .expect("card loaded")
            .title,
        "Original"
    );

    let (_card, inv) = ctx
        .update_card_impl(
            card.id,
            CardUpdate {
                title: Some("Updated".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let changed = model.invalidate(inv);
    NoProjections.resync(&model, changed);

    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    assert_eq!(
        model
            .card_by_id_state(card.id)
            .loaded()
            .expect("card loaded again")
            .title,
        "Updated"
    );
}

pub async fn test_invalidating_one_card_does_not_drop_another_cards_entry(factory: BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let (factory, handles) = faultable(factory);
    let backend = factory(&path);
    let handle = handles.lock().unwrap()[&path].last().unwrap().clone();
    let mut ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();
    let mut model = Model::default();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let a = ctx
        .create_card(
            board.id,
            column.id,
            "A".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let b = ctx
        .create_card(
            board.id,
            column.id,
            "B".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();

    let plan = CardsByIdPlan(vec![a.id, b.id]);
    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    let a_before_state = model.card_by_id_state(a.id);
    let a_before = a_before_state.loaded().copied().expect("a loaded").clone();

    let (_b, inv) = ctx
        .update_card_impl(
            b.id,
            CardUpdate {
                title: Some("B renamed".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let changed = model.invalidate(inv);
    NoProjections.resync(&model, changed);

    handle.clear_ops();
    handle.fail("get_card");

    let a_only_plan = CardsByIdPlan(vec![a.id]);
    let resolved = ctx.resolve(&a_only_plan, &model);
    apply(&mut model, resolved);

    let a_after_state = model.card_by_id_state(a.id);
    let a_after = a_after_state
        .loaded()
        .copied()
        .expect("a still loaded, untouched by b's invalidation");
    assert_eq!(a_before.title, a_after.title);
    assert_eq!(handle.op_count("get_card"), 0);
}

pub async fn test_invalidate_all_clears_every_collection_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let mut model = Model::default();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let _sprint = ctx
        .create_sprint(board.id, Some("SPR".into()), Some("Sprint".into()))
        .unwrap();
    let a = ctx
        .create_card(
            board.id,
            column.id,
            "A".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let b = ctx
        .create_card(
            board.id,
            column.id,
            "B".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let _ = ctx.block_impl(a.id, b.id, Severity::Low).unwrap();

    let plan = StaticPlan(FetchRound {
        card_list: true,
        column_list: true,
        sprint_list: true,
        cards: vec![a.id, b.id],
        graph: true,
        ..Default::default()
    });
    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    assert!(model.cards_state().is_loaded());
    assert!(model.columns_state().is_loaded());
    assert!(model.sprints_state().is_loaded());
    assert!(model.graph_state().is_loaded());

    let changed = model.invalidate(Invalidation::All);
    NoProjections.resync(&model, changed);

    assert!(model.cards_state().is_not_loaded());
    assert!(model.columns_state().is_not_loaded());
    assert!(model.sprints_state().is_not_loaded());
    assert!(model.graph_state().is_not_loaded());
}

pub async fn test_delete_archived_board_leaves_the_same_model_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let mut model = Model::default();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let _sprint = ctx
        .create_sprint(board.id, Some("SPR".into()), Some("Sprint".into()))
        .unwrap();
    let a = ctx
        .create_card(
            board.id,
            column.id,
            "A".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let b = ctx
        .create_card(
            board.id,
            column.id,
            "B".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();
    let _ = ctx.block_impl(a.id, b.id, Severity::Critical).unwrap();
    let _ = ctx.archive_board_impl(board.id).unwrap();

    let plan = StaticPlan(FetchRound {
        columns_by_board: vec![board.id],
        cards_by_column: vec![column.id],
        sprints_by_board: vec![board.id],
        graph: true,
        ..Default::default()
    });
    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);

    let columns_before = model
        .board_columns_state(board.id)
        .loaded()
        .map(|c| c.len())
        .unwrap_or_default();
    let sprints_before = model
        .board_sprints_state(board.id)
        .loaded()
        .map(|s| s.len())
        .unwrap_or_default();
    let cards_before = model
        .column_cards_state(column.id)
        .loaded()
        .map(|c| c.len())
        .unwrap_or_default();
    let edge_before = model
        .graph_state()
        .loaded()
        .expect("graph loaded")
        .blocked(a.id)
        .contains(&b.id);
    assert_eq!(columns_before, 1);
    assert_eq!(sprints_before, 1);
    assert_eq!(cards_before, 2);
    assert!(edge_before);

    ctx.data_store().delete_archived_board(board.id).unwrap();
    let changed = model.invalidate(Invalidation::Entities(EntityIds::boards([board.id])));
    NoProjections.resync(&model, changed);

    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);

    let columns_after = model
        .board_columns_state(board.id)
        .loaded()
        .map(|c| c.len())
        .unwrap_or_default();
    let sprints_after = model
        .board_sprints_state(board.id)
        .loaded()
        .map(|s| s.len())
        .unwrap_or_default();
    let cards_after = model
        .column_cards_state(column.id)
        .loaded()
        .map(|c| c.len())
        .unwrap_or_default();
    let edge_after = model
        .graph_state()
        .loaded()
        .map(|g: &DependencyGraph| g.blocked(a.id).contains(&b.id))
        .unwrap_or(false);

    assert_eq!(cards_after, cards_before, "cards carry no FK on column_id");
    assert_eq!(
        edge_after, edge_before,
        "the graph is untouched by delete_archived_board"
    );
    assert!(
        (columns_after == columns_before && sprints_after == sprints_before)
            || (columns_after == 0 && sprints_after == 0),
        "columns/sprints must either survive untouched (map-style backends) or be \
         fully cascaded away together (SQLite's ON DELETE CASCADE from boards); got \
         columns {columns_before}->{columns_after}, sprints {sprints_before}->{sprints_after}"
    );
}

pub async fn test_the_flat_archived_board_tier_round_trips_markers_on_every_backend(
    factory: &BackendFactory,
) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();
    let mut model = Model::default();

    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();

    let plan = StaticPlan(FetchRound {
        archived_board_list: true,
        ..Default::default()
    });
    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    let empty_state = model.archived_boards_state();
    assert!(empty_state
        .loaded()
        .expect("archived board list loaded")
        .is_empty());

    let inv = ctx.archive_board_impl(board.id).unwrap();
    let changed = model.invalidate(inv);
    NoProjections.resync(&model, changed);

    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    let archived_state = model.archived_boards_state();
    let markers = archived_state.loaded().expect("archived board list loaded");
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].entity_id, board.id);

    let inv = ctx.restore_board_impl(board.id).unwrap();
    let changed = model.invalidate(inv);
    NoProjections.resync(&model, changed);

    let resolved = ctx.resolve(&plan, &model);
    apply(&mut model, resolved);
    let restored_state = model.archived_boards_state();
    let markers = restored_state
        .loaded()
        .expect("archived board list loaded after restore");
    assert!(markers.is_empty());
}
