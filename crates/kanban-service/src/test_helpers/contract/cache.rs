use super::super::fault::faultable;
use super::super::BackendFactory;
use crate::fetch_plan::{requestable, FetchPlan, FetchRound, LoadedEntities};
use crate::KanbanContext;
use kanban_core::AppConfig;
use kanban_domain::{
    DerivedProjections, Invalidation, KanbanOperations, Model, NoProjections, Resolved,
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
    assert!(model
        .cards_state()
        .loaded_or_empty()
        .iter()
        .any(|c| c.id == card.id));

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
    assert!(!model
        .cards_state()
        .loaded_or_empty()
        .iter()
        .any(|c| c.id == card.id));
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
