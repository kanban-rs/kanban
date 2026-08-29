use super::*;

/// The settable half of a [`Model`]: the five fetchable collections plus the
/// two archival marker vectors. The id indexes and the archived-id sets are
/// derived and are rebuilt by [`Model::with_load_states`].
#[derive(Default)]
pub struct ModelLoadStates {
    pub boards: LoadState<Vec<Board>>,
    pub columns: LoadState<Vec<Column>>,
    pub cards: LoadState<Vec<Card>>,
    pub sprints: LoadState<Vec<Sprint>>,
    pub graph: LoadState<DependencyGraph>,
    pub archived_cards: Option<Vec<ArchivedCard>>,
    pub archived_boards: Option<Vec<ArchivedBoard>>,
}

impl Model {
    /// A `Model` with per-collection load states chosen by the caller, in the
    /// same internally-consistent shape `load_from_snapshot` produces: the id
    /// indexes and the archived-id sets are rebuilt from the values supplied.
    /// Test-only surface.
    pub fn with_load_states(_states: ModelLoadStates) -> Self {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArchiveMetadata, KanbanError, NoContext};
    use std::sync::Arc;

    fn seed_board() -> Board {
        Board::new("B", None::<String>)
    }

    fn seed_card(board: &Board) -> Card {
        Card::new(board.id, Uuid::new_v4(), "task", 0)
    }

    #[test]
    fn test_with_load_states_leaves_unnamed_collections_not_loaded() {
        let board = seed_board();
        let card = seed_card(&board);
        let model = Model::with_load_states(ModelLoadStates {
            cards: LoadState::Loaded(vec![card]),
            ..Default::default()
        });
        assert!(model.cards_state().is_loaded());
        assert!(model.boards_state().is_not_loaded());
        assert!(model.columns_state().is_not_loaded());
        assert!(model.sprints_state().is_not_loaded());
        assert!(model.graph_state().is_not_loaded());
    }

    #[test]
    fn test_with_load_states_supports_a_different_state_per_tier() {
        let err = Arc::new(KanbanError::unsupported("boom"));
        let model = Model::with_load_states(ModelLoadStates {
            boards: LoadState::Loaded(vec![seed_board()]),
            columns: LoadState::NotLoaded,
            cards: LoadState::Failed(err),
            sprints: LoadState::Missing,
            graph: LoadState::Loaded(DependencyGraph::default()),
            ..Default::default()
        });
        assert!(model.boards_state().is_loaded());
        assert!(model.columns_state().is_not_loaded());
        assert!(model.cards_state().is_failed());
        assert!(model.sprints_state().is_missing());
        assert!(model.graph_state().is_loaded());
    }

    #[test]
    fn test_with_load_states_preserves_a_failed_collection() {
        let err = Arc::new(KanbanError::unsupported("boom"));
        let model = Model::with_load_states(ModelLoadStates {
            boards: LoadState::Failed(err),
            ..Default::default()
        });
        assert!(model.boards_state().is_failed());
    }

    #[test]
    fn test_with_load_states_rebuilds_the_card_index() {
        let board = seed_board();
        let a = seed_card(&board);
        let b = seed_card(&board);
        let b_id = b.id;
        let model = Model::with_load_states(ModelLoadStates {
            cards: LoadState::Loaded(vec![a, b]),
            ..Default::default()
        });
        assert!(model.card_by_id_state(b_id).loaded().is_some());
    }

    #[test]
    fn test_with_load_states_rebuilds_the_board_index() {
        let a = seed_board();
        let b = Board::new("C", None::<String>);
        let b_id = b.id;
        let model = Model::with_load_states(ModelLoadStates {
            boards: LoadState::Loaded(vec![a, b]),
            ..Default::default()
        });
        assert!(model.board_by_id_state(b_id).loaded().is_some());
    }

    #[test]
    fn test_with_load_states_records_the_archived_card_ids_from_the_markers() {
        let board = seed_board();
        let live = seed_card(&board);
        let archived = seed_card(&board);
        let archived_id = archived.id;
        let marker = ArchivedCard::new(archived_id, board.id);
        let model = Model::with_load_states(ModelLoadStates {
            cards: LoadState::Loaded(vec![live, archived]),
            archived_cards: Some(vec![marker]),
            ..Default::default()
        });
        assert_eq!(model.archived_card_ids().len(), 1);
        assert!(model.archived_card_ids().contains(&archived_id));
        assert_eq!(model.archived_card_markers().len(), 1);
    }

    #[test]
    fn test_with_load_states_records_the_archived_board_ids_from_the_markers() {
        let live = seed_board();
        let archived = Board::new("Arch", None::<String>);
        let archived_id = archived.id;
        let marker = ArchivedBoard {
            entity_id: archived_id,
            metadata: ArchiveMetadata::now(),
            context: NoContext {},
        };
        let model = Model::with_load_states(ModelLoadStates {
            boards: LoadState::Loaded(vec![live, archived]),
            archived_boards: Some(vec![marker]),
            ..Default::default()
        });
        assert_eq!(model.archived_board_ids().len(), 1);
        assert!(model.archived_board_ids().contains(&archived_id));
        assert_eq!(model.archived_boards().len(), 1);
    }

    #[test]
    fn test_with_load_states_default_equals_a_default_model() {
        let built = Model::with_load_states(ModelLoadStates::default());
        let base = Model::default();
        assert_eq!(
            built.boards_state().is_not_loaded(),
            base.boards_state().is_not_loaded()
        );
        assert_eq!(
            built.columns_state().is_not_loaded(),
            base.columns_state().is_not_loaded()
        );
        assert_eq!(
            built.cards_state().is_not_loaded(),
            base.cards_state().is_not_loaded()
        );
        assert_eq!(
            built.sprints_state().is_not_loaded(),
            base.sprints_state().is_not_loaded()
        );
        assert_eq!(
            built.graph_state().is_not_loaded(),
            base.graph_state().is_not_loaded()
        );
        assert_eq!(
            built.archived_card_ids().len(),
            base.archived_card_ids().len()
        );
        assert_eq!(
            built.archived_board_ids().len(),
            base.archived_board_ids().len()
        );
    }
}
