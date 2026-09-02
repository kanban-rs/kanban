use uuid::Uuid;

use kanban_domain::{Column, Model};

use crate::fetch_plan::{FetchStatus, LoadedEntities, LoadedState};

/// `column`/`card`/`sprint` read the per-id tier alone, never the composed
/// three-tier accessors: a composed read falls through to the flat
/// collection, whose `Loaded` arm answers `Missing` for any id it does not
/// name, which would make a live-only card list's absence terminal to a
/// fetch plan instead of leaving the id `NotLoaded`.
impl LoadedState for Model {
    fn board_list(&self) -> FetchStatus {
        self.boards_state().into()
    }

    fn column_list(&self) -> FetchStatus {
        self.columns_state().into()
    }

    fn card_list(&self) -> FetchStatus {
        self.cards_state().into()
    }

    fn sprint_list(&self) -> FetchStatus {
        self.sprints_state().into()
    }

    fn graph(&self) -> FetchStatus {
        self.graph_state().into()
    }

    fn column(&self, id: Uuid) -> FetchStatus {
        (&self.column_id_status(id)).into()
    }

    fn card(&self, id: Uuid) -> FetchStatus {
        (&self.card_id_status(id)).into()
    }

    fn sprint(&self, id: Uuid) -> FetchStatus {
        (&self.sprint_id_status(id)).into()
    }

    fn columns_of_board(&self, board_id: Uuid) -> FetchStatus {
        (&self.board_columns_state(board_id)).into()
    }

    fn cards_of_column(&self, column_id: Uuid) -> FetchStatus {
        (&self.column_cards_state(column_id)).into()
    }

    fn sprints_of_board(&self, board_id: Uuid) -> FetchStatus {
        (&self.board_sprints_state(board_id)).into()
    }

    fn archived_card_list(&self) -> FetchStatus {
        (&self.archived_cards_state()).into()
    }

    fn archived_cards_of_board(&self, board_id: Uuid) -> FetchStatus {
        (&self.board_archived_cards_state(board_id)).into()
    }

    fn archived_board_list(&self) -> FetchStatus {
        (&self.archived_boards_state()).into()
    }
}

impl LoadedEntities for Model {
    fn loaded_columns_of_board(&self, board_id: Uuid) -> Option<&[Column]> {
        self.board_columns_state(board_id).loaded().copied()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kanban_domain::resolved::Collection;
    use kanban_domain::{Card, Column, KanbanError, LoadState, Model, Resolved};
    use uuid::Uuid;

    use crate::fetch_plan::{requestable, FetchStatus, LoadedEntities, LoadedState};

    fn card_in(column_id: Uuid) -> Card {
        Card::new(Uuid::new_v4(), column_id, "task", 0)
    }

    #[test]
    fn test_the_fetch_status_of_a_card_absent_from_a_loaded_list_is_not_loaded() {
        let column_id = Uuid::new_v4();
        let card_a = card_in(column_id);
        let b_id = Uuid::new_v4();
        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::Loaded(vec![card_a]),
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(LoadedState::card(&model, b_id), FetchStatus::NotLoaded);
    }

    #[test]
    fn test_a_missing_per_id_entry_reports_missing_to_a_plan() {
        let b_id = Uuid::new_v4();
        let mut model = Model::default();
        let mut cards = Collection::<Card>::default();
        cards.by_id.insert(b_id, LoadState::Missing);
        let _ = model.apply_resolved(Resolved {
            cards,
            ..Default::default()
        });

        assert_eq!(LoadedState::card(&model, b_id), FetchStatus::Missing);
        assert!(!requestable(LoadedState::card(&model, b_id)));
    }

    #[test]
    fn test_the_list_status_and_the_scope_status_are_independent() {
        let column_id = Uuid::new_v4();
        let card = card_in(column_id);
        let mut model = Model::default();
        model.set_cards_of_column(column_id, LoadState::Loaded(vec![card]));

        assert_eq!(LoadedState::card_list(&model), FetchStatus::NotLoaded);
        assert_eq!(
            LoadedState::cards_of_column(&model, column_id),
            FetchStatus::Loaded
        );
    }

    #[test]
    fn test_loaded_columns_of_board_is_none_for_an_unfetched_scope_and_some_for_an_empty_one() {
        let empty_board = Uuid::new_v4();
        let unread_board = Uuid::new_v4();
        let mut model = Model::default();
        let mut columns = Collection::<Column>::default();
        columns
            .by_parent
            .insert(empty_board, LoadState::Loaded(Vec::new()));
        let _ = model.apply_resolved(Resolved {
            columns,
            ..Default::default()
        });

        assert!(matches!(
            model.loaded_columns_of_board(empty_board),
            Some(s) if s.is_empty()
        ));
        assert!(model.loaded_columns_of_board(unread_board).is_none());
    }

    #[test]
    fn test_loaded_columns_of_board_is_none_for_a_failed_scope() {
        let board_id = Uuid::new_v4();
        let mut model = Model::default();
        let mut columns = Collection::<Column>::default();
        columns.by_parent.insert(
            board_id,
            LoadState::Failed(Arc::new(KanbanError::unsupported("boom"))),
        );
        let _ = model.apply_resolved(Resolved {
            columns,
            ..Default::default()
        });

        assert_eq!(
            LoadedState::columns_of_board(&model, board_id),
            FetchStatus::Failed
        );
        assert!(model.loaded_columns_of_board(board_id).is_none());
    }

    #[test]
    fn test_a_model_is_usable_as_a_dyn_loaded_entities() {
        fn takes(_: &dyn LoadedEntities) {}
        takes(&Model::default());
    }
}
