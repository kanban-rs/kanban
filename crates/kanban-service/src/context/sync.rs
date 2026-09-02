use super::KanbanContext;
use crate::fetch_plan::FetchPlan;
use kanban_domain::{DerivedProjections, Invalidation, Model, ModelChanged};

impl KanbanContext {
    fn resolve_into(&self, plan: &dyn FetchPlan, model: &mut Model) -> ModelChanged {
        let resolved = self.resolve(plan, &*model);
        model.apply_resolved(resolved)
    }

    /// Runs `plan` against `model`, folds the result in, and resyncs `proj`.
    /// A failed read is recorded as `LoadState::Failed` on the affected tier
    /// rather than returned, so a partial failure is visible per tier
    /// instead of collapsing the sync.
    pub fn sync(
        &self,
        plan: &dyn FetchPlan,
        model: &mut Model,
        proj: &mut impl DerivedProjections,
    ) {
        let changed = self.resolve_into(plan, model);
        proj.resync(model, changed);
    }

    /// Applies `inv` to `model` before the plan is consulted, so the
    /// mutated entity is refetched instead of being left `Loaded` and
    /// skipped by the plan's `requestable` gate.
    pub fn sync_invalidated(
        &self,
        inv: Invalidation,
        plan: &dyn FetchPlan,
        model: &mut Model,
        proj: &mut impl DerivedProjections,
    ) {
        let invalidated = model.invalidate(inv);
        let changed = invalidated.merge(self.resolve_into(plan, model));
        proj.resync(model, changed);
    }
}

#[cfg(test)]
mod tests {
    use super::super::KanbanContext;
    use crate::fetch_plan::{requestable, FetchPlan, FetchRound, LoadedEntities};
    use kanban_backend_memory::InMemoryStore;
    use kanban_core::AppConfig;
    use kanban_domain::data_store::DataStore;
    use kanban_domain::{
        Board, CardUpdate, DerivedProjections, Invalidation, KanbanOperations, Model, ModelChanged,
        NoProjections,
    };
    use std::sync::Arc;

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

    struct CardListPlan;
    impl FetchPlan for CardListPlan {
        fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
            FetchRound {
                card_list: requestable(loaded.card_list()),
                ..Default::default()
            }
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
        ctx_a.sync(&CardListPlan, &mut model_a, &mut NoProjections);
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

        ctx_a.sync_invalidated(inv, &CardListPlan, &mut model_a, &mut NoProjections);
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
        ctx_b.sync(&CardListPlan, &mut model_b, &mut NoProjections);
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

        ctx_b.sync(&CardListPlan, &mut model_b, &mut NoProjections);
        assert_eq!(
            model_b.card_by_id_state(card_b.id).loaded().unwrap().title,
            "before"
        );
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
        assert!(model.cards_state().is_not_loaded());
        assert!(!model.cards_state().is_loaded());
        assert!(model.columns_state().is_not_loaded());
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
}
