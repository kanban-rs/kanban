use super::super::fault::faultable;
use super::super::BackendFactory;
use crate::fetch_plan::{FetchPlan, FetchRound, LoadedEntities};
use crate::KanbanContext;
use kanban_core::AppConfig;
use kanban_domain::{DerivedProjections, KanbanOperations, Model, NoProjections, Resolved};
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
