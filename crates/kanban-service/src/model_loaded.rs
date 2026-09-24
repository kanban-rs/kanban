use uuid::Uuid;

use kanban_domain::{ArchivedBoard, ArchivedCard, Column, Model};

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

    fn graph(&self) -> FetchStatus {
        self.graph_state().into()
    }

    fn board(&self, id: Uuid) -> FetchStatus {
        (&self.board_id_status(id)).into()
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

    fn board_in_collection(&self, id: Uuid) -> FetchStatus {
        (&self.board_in_collection_status(id)).into()
    }
}

impl LoadedEntities for Model {
    fn loaded_columns_of_board(&self, board_id: Uuid) -> Option<&[Column]> {
        self.board_columns_state(board_id).loaded().copied()
    }

    fn loaded_archived_card_markers(&self) -> Option<&[ArchivedCard]> {
        self.archived_cards_state().loaded().copied()
    }

    fn loaded_archived_board_markers(&self) -> Option<&[ArchivedBoard]> {
        self.archived_boards_state().loaded().copied()
    }

    fn loaded_archived_cards_of_board(&self, board_id: Uuid) -> Option<&[ArchivedCard]> {
        self.board_archived_cards_state(board_id).loaded().copied()
    }

    fn loaded_graph_neighbours(&self, card_id: Uuid) -> Option<Vec<Uuid>> {
        self.graph_state().loaded().map(|g| g.neighbours(card_id))
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
        let card_a_id = card_a.id;
        let b_id = Uuid::new_v4();
        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            cards: Collection {
                by_id: [(card_a_id, LoadState::Loaded(card_a))].into(),
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
        let mut cards = Collection::<Card>::default();
        cards
            .by_parent
            .insert(column_id, LoadState::Loaded(vec![card]));
        let _ = model.apply_resolved(Resolved {
            cards,
            ..Default::default()
        });

        assert_eq!(LoadedState::board_list(&model), FetchStatus::NotLoaded);
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

    #[test]
    fn test_board_in_collection_reports_not_loaded_for_an_id_absent_from_a_loaded_flat_list() {
        use kanban_domain::Board;

        let live = Board::new("Live", None::<String>);
        let archived_id = Uuid::new_v4();
        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            boards: Collection {
                all: LoadState::Loaded(vec![live]),
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(
            LoadedState::board_in_collection(&model, archived_id),
            FetchStatus::NotLoaded
        );

        let err = Arc::new(KanbanError::unsupported("boom"));
        let _ = model.apply_resolved(Resolved {
            boards: Collection {
                all: LoadState::Failed(err),
                ..Default::default()
            },
            ..Default::default()
        });
        assert_eq!(
            LoadedState::board_in_collection(&model, archived_id),
            FetchStatus::Failed
        );
    }

    #[test]
    fn test_loaded_archived_card_markers_is_none_until_the_marker_tier_loads_and_some_when_empty() {
        use kanban_domain::ArchivedCard;

        let model = Model::default();
        assert!(model.loaded_archived_card_markers().is_none());

        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            archived_cards: Collection {
                all: LoadState::Loaded(Vec::new()),
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(matches!(
            model.loaded_archived_card_markers(),
            Some(s) if s.is_empty()
        ));

        let marker_id = Uuid::new_v4();
        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            archived_cards: Collection {
                all: LoadState::Loaded(vec![ArchivedCard::new(marker_id, Uuid::new_v4())]),
                ..Default::default()
            },
            ..Default::default()
        });
        let markers = model.loaded_archived_card_markers().unwrap();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].entity_id, marker_id);
    }

    #[test]
    fn test_loaded_archived_board_markers_is_none_until_the_marker_tier_loads_and_some_when_empty()
    {
        use kanban_domain::Archived;

        let model = Model::default();
        assert!(model.loaded_archived_board_markers().is_none());

        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            archived_boards: Collection {
                all: LoadState::Loaded(Vec::new()),
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(matches!(
            model.loaded_archived_board_markers(),
            Some(s) if s.is_empty()
        ));

        let marker_id = Uuid::new_v4();
        let mut model = Model::default();
        let _ = model.apply_resolved(Resolved {
            archived_boards: Collection {
                all: LoadState::Loaded(vec![Archived::now(marker_id)]),
                ..Default::default()
            },
            ..Default::default()
        });
        let markers = model.loaded_archived_board_markers().unwrap();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].entity_id, marker_id);
    }
}
