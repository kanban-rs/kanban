use std::sync::Arc;

use kanban_backend_memory::InMemoryStore;
use kanban_core::AppConfig;
use kanban_domain::data_store::DataStore;
use kanban_domain::{Board, CardUpdate, KanbanOperations, NoProjections};

use super::*;
use crate::fetch_plan::{requestable, FetchRound, LoadedEntities};
use uuid::Uuid;

struct BoardListPlan;
impl FetchPlan for BoardListPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            board_list: requestable(loaded.board_list()),
            ..Default::default()
        }
    }
}

struct ArchivedBoardListPlan;
impl FetchPlan for ArchivedBoardListPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            archived_board_list: requestable(loaded.archived_board_list()),
            ..Default::default()
        }
    }
}

struct ForceArchivedBoardListPlan;
impl FetchPlan for ForceArchivedBoardListPlan {
    fn next_round(&self, _loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            archived_board_list: true,
            ..Default::default()
        }
    }
}

struct CardByIdPlan {
    id: Uuid,
}
impl FetchPlan for CardByIdPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            cards: if requestable(loaded.card(self.id)) {
                vec![self.id]
            } else {
                Vec::new()
            },
            ..Default::default()
        }
    }
}

struct NothingPlan;
impl FetchPlan for NothingPlan {
    fn next_round(&self, _loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound::default()
    }
}

#[derive(Default)]
struct CountingProjections {
    resyncs: usize,
}

impl DerivedProjections for CountingProjections {
    fn resync(&mut self, _model: &Model, _changed: ModelChanged) {
        self.resyncs += 1;
    }
}

#[derive(Default)]
struct RecordingProjections {
    any_flags: Vec<bool>,
}

impl DerivedProjections for RecordingProjections {
    fn resync(&mut self, _model: &Model, changed: ModelChanged) {
        self.any_flags.push(changed.any());
    }
}

fn ctx_with_seeded_board() -> (KanbanContext, Board) {
    let store = InMemoryStore::new();
    let board = Board::new("Seeded", None::<String>);
    store.upsert_board(board.clone()).unwrap();
    (
        KanbanContext::open_deferred(Arc::new(store), AppConfig::default()),
        board,
    )
}

#[test]
fn test_sync_applies_a_resolved_pass_into_the_model() {
    let (ctx, board) = ctx_with_seeded_board();
    let mut model = Model::default();
    assert!(model.boards_state().is_not_loaded());

    ctx.sync(&BoardListPlan, &mut model, &mut NoProjections);

    assert!(model.boards_state().is_loaded());
    assert!(model
        .boards_state()
        .loaded_or_empty()
        .iter()
        .any(|b| b.id == board.id));
}

#[test]
fn test_sync_invalidated_refetches_the_invalidated_card_before_planning() {
    let mut ctx_a =
        KanbanContext::open_deferred(Arc::new(InMemoryStore::new()), AppConfig::default());
    let board = ctx_a
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx_a.create_column(board.id, "Col".into(), None).unwrap();
    let card = ctx_a
        .create_card(
            board.id,
            column.id,
            "before".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();

    let mut model_a = Model::default();
    ctx_a.sync(
        &CardByIdPlan { id: card.id },
        &mut model_a,
        &mut NoProjections,
    );
    assert_eq!(
        model_a.card_by_id_state(card.id).loaded().unwrap().title,
        "before"
    );

    let (_card, inv) = ctx_a
        .update_card_impl(
            card.id,
            CardUpdate {
                title: Some("after".into()),
                ..Default::default()
            },
        )
        .unwrap();

    ctx_a.sync_invalidated(
        inv,
        &CardByIdPlan { id: card.id },
        &mut model_a,
        &mut NoProjections,
    );
    assert_eq!(
        model_a.card_by_id_state(card.id).loaded().unwrap().title,
        "after"
    );

    let mut ctx_b =
        KanbanContext::open_deferred(Arc::new(InMemoryStore::new()), AppConfig::default());
    let board_b = ctx_b
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column_b = ctx_b.create_column(board_b.id, "Col".into(), None).unwrap();
    let card_b = ctx_b
        .create_card(
            board_b.id,
            column_b.id,
            "before".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();

    let mut model_b = Model::default();
    ctx_b.sync(
        &CardByIdPlan { id: card_b.id },
        &mut model_b,
        &mut NoProjections,
    );
    assert_eq!(
        model_b.card_by_id_state(card_b.id).loaded().unwrap().title,
        "before"
    );

    let (_card_b, inv_b) = ctx_b
        .update_card_impl(
            card_b.id,
            CardUpdate {
                title: Some("after".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let _ = inv_b;

    ctx_b.sync(
        &CardByIdPlan { id: card_b.id },
        &mut model_b,
        &mut NoProjections,
    );
    assert_eq!(
        model_b.card_by_id_state(card_b.id).loaded().unwrap().title,
        "before"
    );
}

#[test]
fn test_sync_over_an_already_satisfied_plan_reports_an_unchanged_receipt() {
    let (ctx, _board) = ctx_with_seeded_board();
    let mut model = Model::default();
    let mut recording = RecordingProjections::default();

    ctx.sync(&BoardListPlan, &mut model, &mut recording);
    ctx.sync(&BoardListPlan, &mut model, &mut recording);

    assert_eq!(recording.any_flags, vec![true, false]);
}

#[test]
fn test_sync_invalidated_with_an_empty_entity_set_and_a_satisfied_plan_reports_unchanged() {
    let (ctx, board) = ctx_with_seeded_board();
    let mut model = Model::default();
    let mut recording = RecordingProjections::default();

    ctx.sync(&BoardListPlan, &mut model, &mut recording);

    ctx.sync_invalidated(
        Invalidation::Entities(kanban_domain::EntityIds::default()),
        &BoardListPlan,
        &mut model,
        &mut recording,
    );
    ctx.sync_invalidated(
        Invalidation::Entities(kanban_domain::EntityIds::boards([board.id])),
        &BoardListPlan,
        &mut model,
        &mut recording,
    );

    assert_eq!(recording.any_flags.last(), Some(&true));
    assert!(!recording.any_flags[recording.any_flags.len() - 2]);
}

#[test]
fn test_sync_leaves_untouched_tiers_alone() {
    let mut ctx =
        KanbanContext::open_deferred(Arc::new(InMemoryStore::new()), AppConfig::default());
    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let _card = ctx
        .create_card(
            board.id,
            column.id,
            "Card".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();

    let mut model = Model::default();
    ctx.sync(&BoardListPlan, &mut model, &mut NoProjections);

    assert!(model.boards_state().is_loaded());
    assert!(model.column_cards_state(column.id).is_not_loaded());
    assert!(!model.column_cards_state(column.id).is_loaded());
    assert!(model.board_columns_state(board.id).is_not_loaded());
}

#[test]
fn test_sync_hands_the_projections_a_resync() {
    let (ctx, _board) = ctx_with_seeded_board();
    let mut model = Model::default();
    let mut proj = CountingProjections::default();

    ctx.sync(&BoardListPlan, &mut model, &mut proj);
    assert_eq!(proj.resyncs, 1);

    ctx.sync_invalidated(Invalidation::All, &BoardListPlan, &mut model, &mut proj);
    assert_eq!(proj.resyncs, 2);
}

#[cfg(feature = "test-helpers")]
#[test]
fn test_sync_records_a_failed_read_as_failed_not_empty() {
    use crate::test_helpers::FaultInjectingBackend;
    use crate::KanbanBackend;

    let store = InMemoryStore::new();
    let board = Board::new("Seeded", None::<String>);
    store.upsert_board(board).unwrap();
    let backend = FaultInjectingBackend::new(Arc::new(store) as Arc<dyn KanbanBackend>);
    backend.fail("list_boards");

    let ctx = KanbanContext::open_deferred(Arc::new(backend), AppConfig::default());
    let mut model = Model::default();

    ctx.sync(&BoardListPlan, &mut model, &mut NoProjections);

    assert!(model.boards_state().is_failed());
    assert!(!model.boards_state().is_loaded());
}

#[cfg(feature = "test-helpers")]
#[test]
fn test_a_failed_archived_board_read_leaves_the_marker_sets_alone() {
    use crate::test_helpers::FaultInjectingBackend;
    use crate::KanbanBackend;
    use kanban_domain::Archived;

    let store = InMemoryStore::new();
    let board = Board::new("Archived", None::<String>);
    store.upsert_board(board.clone()).unwrap();
    store
        .insert_archived_board(Archived::now(board.id))
        .unwrap();
    let backend = Arc::new(FaultInjectingBackend::new(
        Arc::new(store) as Arc<dyn KanbanBackend>
    ));

    let ctx = KanbanContext::open_deferred(
        backend.clone() as Arc<dyn KanbanBackend>,
        AppConfig::default(),
    );
    let mut model = Model::default();

    ctx.sync(&ArchivedBoardListPlan, &mut model, &mut NoProjections);
    assert!(model.archived_boards_state().is_loaded());
    assert!(model.archived_board_ids().contains(&board.id));

    backend.fail("list_archived_boards");
    ctx.sync(&ForceArchivedBoardListPlan, &mut model, &mut NoProjections);

    assert!(model.archived_boards_state().is_failed());
    assert!(model.archived_board_ids().contains(&board.id));
    assert_eq!(model.archived_boards().len(), 1);
}

fn seed_card(ctx: &mut KanbanContext) -> kanban_domain::Card {
    let board = ctx
        .create_board("Board".into(), Some("BRD".into()))
        .unwrap();
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    ctx.create_card(
        board.id,
        column.id,
        "before".into(),
        kanban_domain::CreateCardOptions::default(),
    )
    .unwrap()
}

#[test]
fn test_resync_invalidated_refetches_a_mutated_card_the_caller_plan_never_names() {
    let mut ctx =
        KanbanContext::open_deferred(Arc::new(InMemoryStore::new()), AppConfig::default());
    let card = seed_card(&mut ctx);

    let mut model = Model::default();
    ctx.sync(
        &CardByIdPlan { id: card.id },
        &mut model,
        &mut NoProjections,
    );
    assert_eq!(
        model.card_by_id_state(card.id).loaded().unwrap().title,
        "before"
    );

    let (_card, inv) = ctx
        .update_card_impl(
            card.id,
            CardUpdate {
                title: Some("after".into()),
                ..Default::default()
            },
        )
        .unwrap();

    ctx.resync_invalidated(inv, &NothingPlan, &mut model, &mut NoProjections);
    assert_eq!(
        model.card_by_id_state(card.id).loaded().unwrap().title,
        "after"
    );
    let mut ctx_control =
        KanbanContext::open_deferred(Arc::new(InMemoryStore::new()), AppConfig::default());
    let card_control = seed_card(&mut ctx_control);

    let mut model_control = Model::default();
    ctx_control.sync(
        &CardByIdPlan {
            id: card_control.id,
        },
        &mut model_control,
        &mut NoProjections,
    );

    let (_card_control, inv_control) = ctx_control
        .update_card_impl(
            card_control.id,
            CardUpdate {
                title: Some("after".into()),
                ..Default::default()
            },
        )
        .unwrap();

    // The defect this card closes: sync_invalidated blanks the tier
    // per `inv_control` and NothingPlan asks for nothing back, so the
    // card is left unreadable rather than repaired.
    ctx_control.sync_invalidated(
        inv_control,
        &NothingPlan,
        &mut model_control,
        &mut NoProjections,
    );
    assert!(model_control
        .card_by_id_state(card_control.id)
        .loaded()
        .is_none());
}

#[test]
fn test_resync_invalidated_does_not_fetch_a_tier_the_model_never_read() {
    let mut ctx =
        KanbanContext::open_deferred(Arc::new(InMemoryStore::new()), AppConfig::default());
    let card = seed_card(&mut ctx);

    let mut model = Model::default();
    let (_card, inv) = ctx
        .update_card_impl(
            card.id,
            CardUpdate {
                title: Some("after".into()),
                ..Default::default()
            },
        )
        .unwrap();

    ctx.resync_invalidated(inv, &NothingPlan, &mut model, &mut NoProjections);

    assert!(model.boards_state().is_not_loaded());
    assert!(model
        .board_columns_state(uuid::Uuid::new_v4())
        .is_not_loaded());
    assert!(model
        .column_cards_state(uuid::Uuid::new_v4())
        .is_not_loaded());
    assert!(model
        .board_sprints_state(uuid::Uuid::new_v4())
        .is_not_loaded());
    assert!(model.graph_state().is_not_loaded());
    assert!(model.card_by_id_state(card.id).loaded().is_none());
}

#[test]
fn test_resync_invalidated_still_invalidates_when_there_is_no_bounded_repair() {
    let (ctx, _board) = ctx_with_seeded_board();
    let mut model = Model::default();
    assert!(model.boards_state().is_not_loaded());

    ctx.resync_invalidated(
        Invalidation::All,
        &NothingPlan,
        &mut model,
        &mut NoProjections,
    );

    assert!(model.boards_state().is_not_loaded());
    assert!(model
        .board_columns_state(uuid::Uuid::new_v4())
        .is_not_loaded());
    assert!(model
        .column_cards_state(uuid::Uuid::new_v4())
        .is_not_loaded());
    assert!(model
        .board_sprints_state(uuid::Uuid::new_v4())
        .is_not_loaded());
    assert!(model.graph_state().is_not_loaded());
}

#[test]
fn test_resync_invalidated_still_runs_the_callers_plan() {
    let (ctx, _board) = ctx_with_seeded_board();
    let mut model = Model::default();

    ctx.resync_invalidated(
        Invalidation::All,
        &BoardListPlan,
        &mut model,
        &mut NoProjections,
    );

    assert!(model.boards_state().is_loaded());
}

#[cfg(feature = "test-helpers")]
#[test]
fn test_resync_invalidated_does_not_refetch_what_the_repair_pass_already_read() {
    use crate::test_helpers::FaultInjectingBackend;
    use crate::KanbanBackend;

    let store = InMemoryStore::new();
    let board = Board::new("Board", None::<String>);
    store.upsert_board(board.clone()).unwrap();
    let backend = Arc::new(FaultInjectingBackend::new(
        Arc::new(store) as Arc<dyn KanbanBackend>
    ));

    let mut ctx = KanbanContext::open_deferred(
        backend.clone() as Arc<dyn KanbanBackend>,
        AppConfig::default(),
    );
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "before".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();

    let mut model = Model::default();
    ctx.sync(
        &CardByIdPlan { id: card.id },
        &mut model,
        &mut NoProjections,
    );

    let (_card, inv) = ctx
        .update_card_impl(
            card.id,
            CardUpdate {
                title: Some("after".into()),
                ..Default::default()
            },
        )
        .unwrap();

    backend.clear_ops();
    ctx.resync_invalidated(
        inv,
        &CardByIdPlan { id: card.id },
        &mut model,
        &mut NoProjections,
    );

    assert_eq!(backend.op_count("get_card"), 1);
}

#[cfg(feature = "test-helpers")]
#[test]
fn test_resync_invalidated_records_a_failed_repair_read_as_failed_not_empty() {
    use crate::test_helpers::FaultInjectingBackend;
    use crate::KanbanBackend;

    let store = InMemoryStore::new();
    let board = Board::new("Board", None::<String>);
    store.upsert_board(board.clone()).unwrap();
    let backend = Arc::new(FaultInjectingBackend::new(
        Arc::new(store) as Arc<dyn KanbanBackend>
    ));

    let mut ctx = KanbanContext::open_deferred(
        backend.clone() as Arc<dyn KanbanBackend>,
        AppConfig::default(),
    );
    let column = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let card = ctx
        .create_card(
            board.id,
            column.id,
            "before".into(),
            kanban_domain::CreateCardOptions::default(),
        )
        .unwrap();

    let mut model = Model::default();
    ctx.sync(
        &CardByIdPlan { id: card.id },
        &mut model,
        &mut NoProjections,
    );

    let (_card, inv) = ctx
        .update_card_impl(
            card.id,
            CardUpdate {
                title: Some("after".into()),
                ..Default::default()
            },
        )
        .unwrap();

    backend.fail("get_card");
    ctx.resync_invalidated(inv, &NothingPlan, &mut model, &mut NoProjections);

    assert!(model.card_by_id_state(card.id).is_failed());
    assert!(!model.card_by_id_state(card.id).is_loaded());
}
