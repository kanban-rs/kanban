use super::*;

/// The settable half of a [`Model`]: the boards flat collection, the graph,
/// and the two archival marker vectors. The board id index and the
/// archived-id sets are derived and are rebuilt by
/// [`Model::with_load_states`]. Columns/cards/sprints have no flat tier to
/// seed here; use `apply_resolved` with `by_id`/`by_parent` for those.
#[derive(Default)]
pub struct ModelLoadStates {
    pub boards: LoadState<Vec<Board>>,
    pub graph: LoadState<DependencyGraph>,
    pub archived_cards: Option<Vec<ArchivedCard>>,
    pub archived_boards: Option<Vec<ArchivedBoard>>,
}

impl Model {
    /// A `Model` with per-collection load states chosen by the caller, in the
    /// same internally-consistent shape `load_from_snapshot` produces: the
    /// board id index and the archived-id sets are rebuilt from the values
    /// supplied. Test-only surface.
    pub fn with_load_states(states: ModelLoadStates) -> Self {
        let ModelLoadStates {
            boards,
            graph,
            archived_cards,
            archived_boards,
        } = states;

        let mut model = Self {
            boards,
            graph,
            ..Self::default()
        };

        model.absorb_archival_markers(archived_cards, archived_boards);
        model.rebuild_board_index();
        model
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
        let model = Model::with_load_states(ModelLoadStates::default());
        assert!(model.boards_state().is_not_loaded());
        assert!(model.graph_state().is_not_loaded());
    }

    #[test]
    fn test_with_load_states_supports_a_different_state_per_tier() {
        let model = Model::with_load_states(ModelLoadStates {
            boards: LoadState::Loaded(vec![seed_board()]),
            graph: LoadState::Loaded(DependencyGraph::default()),
            ..Default::default()
        });
        assert!(model.boards_state().is_loaded());
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
        let archived = seed_card(&board);
        let archived_id = archived.id;
        let marker = ArchivedCard::new(archived_id, board.id);
        let model = Model::with_load_states(ModelLoadStates {
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
        let random_id = Uuid::new_v4();
        assert_eq!(
            built.board_columns_state(random_id).is_not_loaded(),
            base.board_columns_state(random_id).is_not_loaded()
        );
        assert_eq!(
            built.column_cards_state(random_id).is_not_loaded(),
            base.column_cards_state(random_id).is_not_loaded()
        );
        assert_eq!(
            built.board_sprints_state(random_id).is_not_loaded(),
            base.board_sprints_state(random_id).is_not_loaded()
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
