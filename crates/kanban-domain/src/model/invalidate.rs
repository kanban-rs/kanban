use super::*;
use crate::Invalidation;

impl Model {
    pub fn invalidate(&mut self, _invalidation: Invalidation) -> ModelChanged {
        ModelChanged::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolved::Collection;
    use crate::{
        ArchivedCard, Board, Card, Column, DependencyGraph, EntityIds, NoProjections, Resolved,
        Snapshot, Sprint,
    };

    fn seeded() -> (Model, Board, Column, Column, Card, Card, Sprint) {
        let board = Board::new("B", None::<String>);
        let col_a = Column::new(board.id, "A", 0);
        let col_b = Column::new(board.id, "B", 1);
        let c1 = Card::new(board.id, col_a.id, "one", 0);
        let c2 = Card::new(board.id, col_b.id, "two", 0);
        let sprint = Sprint::new(board.id, 1, None, None::<String>);

        let model = Model::with_load_states(ModelLoadStates {
            boards: LoadState::Loaded(vec![board.clone()]),
            columns: LoadState::Loaded(vec![col_a.clone(), col_b.clone()]),
            cards: LoadState::Loaded(vec![c1.clone(), c2.clone()]),
            sprints: LoadState::Loaded(vec![sprint.clone()]),
            graph: LoadState::Loaded(DependencyGraph::default()),
            ..Default::default()
        });

        (model, board, col_a, col_b, c1, c2, sprint)
    }

    fn load_every_tier(
        m: &mut Model,
        board: &Board,
        col_a: &Column,
        col_b: &Column,
        c1: &Card,
        c2: &Card,
        sprint: &Sprint,
    ) {
        let changed = m.apply_resolved(Resolved {
            columns: Collection {
                by_id: [(col_a.id, LoadState::Loaded(col_a.clone()))].into(),
                by_parent: [(
                    board.id,
                    LoadState::Loaded(vec![col_a.clone(), col_b.clone()]),
                )]
                .into(),
                ..Default::default()
            },
            cards: Collection {
                by_id: [(c1.id, LoadState::Loaded(c1.clone()))].into(),
                ..Default::default()
            },
            sprints: Collection {
                by_id: [(sprint.id, LoadState::Loaded(sprint.clone()))].into(),
                by_parent: [(board.id, LoadState::Loaded(vec![sprint.clone()]))].into(),
                ..Default::default()
            },
            ..Default::default()
        });
        NoProjections.resync(m, changed);
        m.set_cards_of_column(col_a.id, LoadState::Loaded(vec![c1.clone()]));
        m.set_cards_of_column(col_b.id, LoadState::Loaded(vec![c2.clone()]));
    }

    fn assert_every_tier_not_loaded(
        m: &Model,
        board: &Board,
        col_a: &Column,
        col_b: &Column,
        c1: &Card,
        c2: &Card,
        sprint: &Sprint,
    ) {
        assert!(m.boards_state().is_not_loaded());
        assert!(m.columns_state().is_not_loaded());
        assert!(m.cards_state().is_not_loaded());
        assert!(m.sprints_state().is_not_loaded());
        assert!(m.graph_state().is_not_loaded());
        assert!(m.board_by_id_state(board.id).is_not_loaded());
        assert!(m.column_id_status(col_a.id).is_not_loaded());
        assert!(m.column_id_status(col_b.id).is_not_loaded());
        assert!(m.card_id_status(c1.id).is_not_loaded());
        assert!(m.card_id_status(c2.id).is_not_loaded());
        assert!(m.sprint_id_status(sprint.id).is_not_loaded());
        assert!(m.column_cards_state(col_a.id).is_not_loaded());
        assert!(m.column_cards_state(col_b.id).is_not_loaded());
        assert!(m.board_columns_state(board.id).is_not_loaded());
        assert!(m.board_sprints_state(board.id).is_not_loaded());
        assert!(m.scoped_card_index.is_empty());
        assert!(m.card_index.is_empty());
        assert!(m.board_index.is_empty());
    }

    #[test]
    fn test_a_moved_card_is_not_served_from_a_stale_scope() {
        let board = Board::new("B", None::<String>);
        let col_a = Column::new(board.id, "A", 0);
        let col_b2 = Column::new(board.id, "B2", 1);
        let card = Card::new(board.id, col_a.id, "task", 0);
        let card_id = card.id;

        let mut m = Model::default();
        let changed = m.apply_resolved(Resolved {
            cards: Collection {
                by_parent: [
                    (col_a.id, LoadState::Loaded(vec![card.clone()])),
                    (col_b2.id, LoadState::Loaded(vec![])),
                ]
                .into(),
                ..Default::default()
            },
            ..Default::default()
        });
        NoProjections.resync(&m, changed);

        assert_eq!(
            m.column_cards_state(col_a.id)
                .loaded()
                .copied()
                .unwrap_or(&[])
                .len(),
            1
        );
        assert!(m
            .column_cards_state(col_b2.id)
            .loaded()
            .copied()
            .unwrap()
            .is_empty());
        assert!(m.card_by_id_state(card_id).is_loaded());

        let changed = m.invalidate(Invalidation::Entities(EntityIds::cards([card_id])));
        NoProjections.resync(&m, changed);

        assert!(m.column_cards_state(col_a.id).is_not_loaded());
        assert!(m.column_cards_state(col_b2.id).is_not_loaded());
        assert!(m.card_by_id_state(card_id).is_not_loaded());

        let mut moved = card.clone();
        moved.column_id = col_b2.id;
        let changed = m.apply_resolved(Resolved {
            cards: Collection {
                by_parent: [
                    (col_a.id, LoadState::Loaded(vec![])),
                    (col_b2.id, LoadState::Loaded(vec![moved.clone()])),
                ]
                .into(),
                ..Default::default()
            },
            ..Default::default()
        });
        NoProjections.resync(&m, changed);

        assert!(m
            .column_cards_state(col_a.id)
            .loaded()
            .copied()
            .unwrap()
            .is_empty());
        let in_b2 = m.column_cards_state(col_b2.id).loaded().copied().unwrap();
        assert_eq!(in_b2.len(), 1);
        assert_eq!(in_b2[0].id, card_id);
        assert_eq!(in_b2[0].column_id, col_b2.id);
    }

    #[test]
    fn test_invalidating_a_card_drops_every_cards_by_column_scope() {
        let board = Board::new("B", None::<String>);
        let col_a = Column::new(board.id, "A", 0);
        let col_b = Column::new(board.id, "B", 1);
        let c1 = Card::new(board.id, col_a.id, "one", 0);
        let c2 = Card::new(board.id, col_b.id, "two", 0);

        let mut m = Model::default();
        m.set_cards_of_column(col_a.id, LoadState::Loaded(vec![c1.clone()]));
        m.set_cards_of_column(col_b.id, LoadState::Loaded(vec![c2.clone()]));

        let _ = m.invalidate(Invalidation::Entities(EntityIds::cards([c1.id])));

        assert!(m.column_cards_state(col_a.id).is_not_loaded());
        assert!(m.column_cards_state(col_b.id).is_not_loaded());
    }

    #[test]
    fn test_invalidating_a_card_clears_the_scoped_card_index() {
        let board = Board::new("B", None::<String>);
        let col_a = Column::new(board.id, "A", 0);
        let card = Card::new(board.id, col_a.id, "task", 0);

        let mut m = Model::default();
        m.set_cards_of_column(col_a.id, LoadState::Loaded(vec![card.clone()]));

        let _ = m.invalidate(Invalidation::Entities(EntityIds::cards([card.id])));

        assert!(m.scoped_card_index.is_empty());
        assert!(m.card_by_id_state(card.id).is_not_loaded());
    }

    #[test]
    fn test_invalidating_a_column_clears_that_columns_scoped_card_index_entries() {
        let board = Board::new("B", None::<String>);
        let col_a = Column::new(board.id, "A", 0);
        let col_b = Column::new(board.id, "B", 1);
        let c1 = Card::new(board.id, col_a.id, "one", 0);
        let c2 = Card::new(board.id, col_b.id, "two", 0);

        let mut m = Model::default();
        m.set_cards_of_column(col_a.id, LoadState::Loaded(vec![c1.clone()]));
        m.set_cards_of_column(col_b.id, LoadState::Loaded(vec![c2.clone()]));

        let _ = m.invalidate(Invalidation::Entities(EntityIds::columns([col_a.id])));

        assert_eq!(m.scoped_card_index.get(&c1.id), None);
        assert_eq!(m.scoped_card_index.get(&c2.id), Some(&col_b.id));
        assert!(m.column_cards_state(col_b.id).is_loaded());
    }

    #[test]
    fn test_invalidate_leaves_set_cards_of_column_the_only_writer_of_the_pair() {
        let board = Board::new("B", None::<String>);
        let col_a = Column::new(board.id, "A", 0);
        let col_b = Column::new(board.id, "B", 1);
        let c1 = Card::new(board.id, col_a.id, "one", 0);
        let c2 = Card::new(board.id, col_b.id, "two", 0);
        let c3 = Card::new(board.id, col_a.id, "three", 0);

        let mut m = Model::default();
        m.set_cards_of_column(col_a.id, LoadState::Loaded(vec![c1.clone()]));
        m.set_cards_of_column(col_b.id, LoadState::Loaded(vec![c2.clone()]));

        let _ = m.invalidate(Invalidation::Entities(EntityIds::cards([c1.id])));
        m.set_cards_of_column(col_a.id, LoadState::Loaded(vec![c3.clone()]));

        assert_eq!(m.scoped_card_index.len(), 1);
        assert_eq!(m.scoped_card_index.get(&c3.id), Some(&col_a.id));
    }

    #[test]
    fn test_invalidating_a_card_clears_the_card_index() {
        let board = Board::new("B", None::<String>);
        let col = Column::new(board.id, "A", 0);
        let c1 = Card::new(board.id, col.id, "one", 0);
        let c2 = Card::new(board.id, col.id, "two", 0);

        let mut m = Model::with_load_states(ModelLoadStates {
            cards: LoadState::Loaded(vec![c1.clone(), c2.clone()]),
            ..Default::default()
        });

        let _ = m.invalidate(Invalidation::Entities(EntityIds::cards([c1.id])));

        assert!(m.cards_state().is_not_loaded());
        assert!(m.card_by_id_state(c2.id).is_not_loaded());
    }

    #[test]
    fn test_invalidating_a_board_clears_the_board_index() {
        let b1 = Board::new("B1", None::<String>);
        let b2 = Board::new("B2", None::<String>);

        let mut m = Model::with_load_states(ModelLoadStates {
            boards: LoadState::Loaded(vec![b1.clone(), b2.clone()]),
            ..Default::default()
        });

        let _ = m.invalidate(Invalidation::Entities(EntityIds::boards([b1.id])));

        assert!(m.boards_state().is_not_loaded());
        assert!(m.board_by_id_state(b2.id).is_not_loaded());
        assert!(m.board_index.is_empty());
    }

    #[test]
    fn test_archiving_a_card_drops_the_graph_tier_with_the_per_id_tier() {
        let board = Board::new("B", None::<String>);
        let col = Column::new(board.id, "A", 0);
        let card = Card::new(board.id, col.id, "task", 0);

        let mut m = Model::default();
        let changed = m.apply_resolved(Resolved {
            cards: Collection {
                by_id: [(card.id, LoadState::Loaded(card.clone()))].into(),
                ..Default::default()
            },
            graph: LoadState::Loaded(DependencyGraph::default()),
            ..Default::default()
        });
        NoProjections.resync(&m, changed);
        m.set_cards_of_column(col.id, LoadState::Loaded(vec![card.clone()]));

        let ids = EntityIds::cards([card.id]).with_graph();
        let _ = m.invalidate(Invalidation::Entities(ids));

        assert!(m.graph_state().is_not_loaded());
        assert!(m.card_id_status(card.id).is_not_loaded());
        assert!(m.column_cards_state(col.id).is_not_loaded());
    }

    #[test]
    fn test_invalidating_a_card_leaves_the_column_and_sprint_scopes_loaded() {
        let (mut m, board, col_a, col_b, c1, c2, sprint) = seeded();
        load_every_tier(&mut m, &board, &col_a, &col_b, &c1, &c2, &sprint);

        let _ = m.invalidate(Invalidation::Entities(EntityIds::cards([c1.id])));

        assert!(m.board_columns_state(board.id).is_loaded());
        assert!(m.board_sprints_state(board.id).is_loaded());
    }

    #[test]
    fn test_invalidating_a_column_drops_that_columns_card_scope() {
        let (mut m, board, col_a, col_b, c1, c2, sprint) = seeded();
        load_every_tier(&mut m, &board, &col_a, &col_b, &c1, &c2, &sprint);

        let _ = m.invalidate(Invalidation::Entities(EntityIds::columns([col_a.id])));

        assert!(m.column_cards_state(col_a.id).is_not_loaded());
        assert!(m.column_cards_state(col_b.id).is_loaded());
        assert!(m.board_columns_state(board.id).is_not_loaded());
    }

    #[test]
    fn test_invalidating_a_board_drops_only_that_boards_scopes() {
        let b1 = Board::new("B1", None::<String>);
        let b2 = Board::new("B2", None::<String>);
        let col1 = Column::new(b1.id, "A", 0);
        let col2 = Column::new(b2.id, "A", 0);
        let s1 = Sprint::new(b1.id, 1, None, None::<String>);
        let s2 = Sprint::new(b2.id, 1, None, None::<String>);

        let mut m = Model::default();
        let changed = m.apply_resolved(Resolved {
            columns: Collection {
                by_parent: [
                    (b1.id, LoadState::Loaded(vec![col1.clone()])),
                    (b2.id, LoadState::Loaded(vec![col2.clone()])),
                ]
                .into(),
                ..Default::default()
            },
            sprints: Collection {
                by_parent: [
                    (b1.id, LoadState::Loaded(vec![s1.clone()])),
                    (b2.id, LoadState::Loaded(vec![s2.clone()])),
                ]
                .into(),
                ..Default::default()
            },
            ..Default::default()
        });
        NoProjections.resync(&m, changed);

        let _ = m.invalidate(Invalidation::Entities(EntityIds::boards([b1.id])));

        assert!(m.board_columns_state(b1.id).is_not_loaded());
        assert!(m.board_sprints_state(b1.id).is_not_loaded());
        assert!(m.board_columns_state(b2.id).is_loaded());
        assert!(m.board_sprints_state(b2.id).is_loaded());
    }

    #[test]
    fn test_invalidating_a_sprint_drops_the_whole_sprints_by_board_tier() {
        let b1 = Board::new("B1", None::<String>);
        let b2 = Board::new("B2", None::<String>);
        let col1 = Column::new(b1.id, "A", 0);
        let s = Sprint::new(b1.id, 1, None, None::<String>);
        let s2 = Sprint::new(b2.id, 1, None, None::<String>);

        let mut m = Model::default();
        let changed = m.apply_resolved(Resolved {
            columns: Collection {
                by_parent: [(b1.id, LoadState::Loaded(vec![col1.clone()]))].into(),
                ..Default::default()
            },
            sprints: Collection {
                by_parent: [
                    (b1.id, LoadState::Loaded(vec![s.clone()])),
                    (b2.id, LoadState::Loaded(vec![s2.clone()])),
                ]
                .into(),
                ..Default::default()
            },
            ..Default::default()
        });
        NoProjections.resync(&m, changed);

        let _ = m.invalidate(Invalidation::Entities(EntityIds::sprints([s.id])));

        assert!(m.board_sprints_state(b1.id).is_not_loaded());
        assert!(m.board_sprints_state(b2.id).is_not_loaded());
        assert!(m.board_columns_state(b1.id).is_loaded());
    }

    #[test]
    fn test_invalidate_all_clears_every_tier() {
        let (mut m, board, col_a, col_b, c1, c2, sprint) = seeded();
        load_every_tier(&mut m, &board, &col_a, &col_b, &c1, &c2, &sprint);

        let _ = m.invalidate(Invalidation::All);

        assert_every_tier_not_loaded(&m, &board, &col_a, &col_b, &c1, &c2, &sprint);
    }

    #[test]
    fn test_an_empty_entity_ids_clears_every_tier() {
        let (mut m, board, col_a, col_b, c1, c2, sprint) = seeded();
        load_every_tier(&mut m, &board, &col_a, &col_b, &c1, &c2, &sprint);

        let _ = m.invalidate(Invalidation::Entities(EntityIds::default()));

        assert_every_tier_not_loaded(&m, &board, &col_a, &col_b, &c1, &c2, &sprint);
    }

    #[test]
    fn test_invalidate_all_resets_the_snapshot_derived_archived_fields_too() {
        let board = Board::new("B", None::<String>);
        let card = Card::new(board.id, Uuid::new_v4(), "task", 0);
        let marker = ArchivedCard::new(card.id, board.id);

        let mut m = Model::default();
        let changed = m.load_from_snapshot(Snapshot {
            boards: vec![board],
            cards: vec![card],
            archived_cards: vec![marker],
            ..Default::default()
        });
        NoProjections.resync(&m, changed);
        assert!(!m.archived_card_ids().is_empty());

        let _ = m.invalidate(Invalidation::All);

        assert!(m.archived_card_markers().is_empty());
        assert!(m.archived_card_ids().is_empty());
    }

    #[test]
    fn test_invalidate_does_not_touch_the_snapshot_derived_archived_ids() {
        let board = Board::new("B", None::<String>);
        let card = Card::new(board.id, Uuid::new_v4(), "task", 0);
        let card_id = card.id;
        let marker = ArchivedCard::new(card_id, board.id);

        let mut m = Model::default();
        let changed = m.load_from_snapshot(Snapshot {
            boards: vec![board],
            cards: vec![card],
            archived_cards: vec![marker],
            ..Default::default()
        });
        NoProjections.resync(&m, changed);
        let before: std::collections::HashSet<_> = m.archived_card_ids().clone();
        assert!(!before.is_empty());

        let _ = m.invalidate(Invalidation::Entities(EntityIds::cards([card_id])));

        assert_eq!(m.archived_card_ids(), &before);
        assert!(!m.archived_card_markers().is_empty());
    }

    #[test]
    fn test_invalidate_returns_a_model_changed_receipt() {
        let mut m = Model::default();
        let changed: ModelChanged = m.invalidate(Invalidation::All);
        NoProjections.resync(&m, changed);
        assert!(m.cards_state().is_not_loaded());
    }

    #[test]
    fn test_invalidate_a_column_id_drops_that_column_and_the_column_collection() {
        let (mut m, _board, col_a, col_b, _c1, _c2, _sprint) = seeded();

        let changed = m.apply_resolved(Resolved {
            columns: Collection {
                by_id: [
                    (col_a.id, LoadState::Loaded(col_a.clone())),
                    (col_b.id, LoadState::Loaded(col_b.clone())),
                ]
                .into(),
                ..Default::default()
            },
            ..Default::default()
        });
        NoProjections.resync(&m, changed);

        let _ = m.invalidate(Invalidation::Entities(EntityIds::columns([col_a.id])));

        assert!(m.column_id_status(col_a.id).is_not_loaded());
        assert!(m.column_id_status(col_b.id).is_loaded());
        assert!(m.columns_state().is_not_loaded());
        assert!(m.boards_state().is_loaded());
        assert!(m.cards_state().is_loaded());
        assert!(m.sprints_state().is_loaded());
        assert!(m.graph_state().is_loaded());
    }

    #[test]
    fn test_invalidate_a_sprint_id_drops_that_sprint_and_the_sprint_collection() {
        let board = Board::new("B", None::<String>);
        let sa = Sprint::new(board.id, 1, None, None::<String>);
        let sb = Sprint::new(board.id, 2, None, None::<String>);

        let mut m = Model::with_load_states(ModelLoadStates {
            boards: LoadState::Loaded(vec![board.clone()]),
            columns: LoadState::Loaded(vec![]),
            cards: LoadState::Loaded(vec![]),
            sprints: LoadState::Loaded(vec![sa.clone(), sb.clone()]),
            graph: LoadState::Loaded(DependencyGraph::default()),
            ..Default::default()
        });
        let changed = m.apply_resolved(Resolved {
            sprints: Collection {
                by_id: [
                    (sa.id, LoadState::Loaded(sa.clone())),
                    (sb.id, LoadState::Loaded(sb.clone())),
                ]
                .into(),
                ..Default::default()
            },
            ..Default::default()
        });
        NoProjections.resync(&m, changed);

        let _ = m.invalidate(Invalidation::Entities(EntityIds::sprints([sa.id])));

        assert!(m.sprint_id_status(sa.id).is_not_loaded());
        assert!(m.sprint_id_status(sb.id).is_loaded());
        assert!(m.sprints_state().is_not_loaded());
        assert!(m.boards_state().is_loaded());
        assert!(m.columns_state().is_loaded());
        assert!(m.cards_state().is_loaded());
        assert!(m.graph_state().is_loaded());
    }

    #[test]
    fn test_invalidate_a_board_id_drops_the_board_collection_and_that_boards_scopes() {
        let (mut m, board, col_a, col_b, c1, c2, sprint) = seeded();
        load_every_tier(&mut m, &board, &col_a, &col_b, &c1, &c2, &sprint);
        let changed = m.apply_resolved(Resolved {
            boards: Collection {
                by_id: [(board.id, LoadState::Loaded(board.clone()))].into(),
                ..Default::default()
            },
            ..Default::default()
        });
        NoProjections.resync(&m, changed);

        let _ = m.invalidate(Invalidation::Entities(EntityIds::boards([board.id])));

        assert!(m.boards_state().is_not_loaded());
        assert!(m.columns_state().is_loaded());
        assert!(m.cards_state().is_loaded());
        assert!(m.sprints_state().is_loaded());
        assert!(m.graph_state().is_loaded());
        assert!(m.column_id_status(col_a.id).is_loaded());
        assert!(m.card_id_status(c1.id).is_loaded());
        assert!(m.sprint_id_status(sprint.id).is_loaded());
    }

    #[test]
    fn test_invalidate_entities_clears_only_the_named_ids() {
        let board = Board::new("B", None::<String>);
        let col = Column::new(board.id, "A", 0);
        let a = Card::new(board.id, col.id, "a", 0);
        let b = Card::new(board.id, col.id, "b", 0);

        let mut m = Model::default();
        let changed = m.apply_resolved(Resolved {
            cards: Collection {
                by_id: [
                    (a.id, LoadState::Loaded(a.clone())),
                    (b.id, LoadState::Loaded(b.clone())),
                ]
                .into(),
                ..Default::default()
            },
            ..Default::default()
        });
        NoProjections.resync(&m, changed);

        let _ = m.invalidate(Invalidation::Entities(EntityIds::cards([a.id])));

        assert!(m.card_id_status(a.id).is_not_loaded());
        assert!(m.card_id_status(b.id).is_loaded());
    }

    #[test]
    fn test_invalidate_a_card_id_also_drops_the_whole_card_collection() {
        let (mut m, board, col_a, col_b, c1, c2, sprint) = seeded();
        load_every_tier(&mut m, &board, &col_a, &col_b, &c1, &c2, &sprint);

        let _ = m.invalidate(Invalidation::Entities(EntityIds::cards([c1.id])));

        assert!(m.cards_state().is_not_loaded());
        assert!(m.boards_state().is_loaded());
        assert!(m.columns_state().is_loaded());
        assert!(m.sprints_state().is_loaded());
        assert!(m.graph_state().is_loaded());
    }

    #[test]
    fn test_invalidate_all_clears_every_one_of_the_five_kinds() {
        let board = Board::new("B", None::<String>);
        let col = Column::new(board.id, "A", 0);
        let card = Card::new(board.id, col.id, "task", 0);
        let sprint = Sprint::new(board.id, 1, None, None::<String>);

        let mut m = Model::with_load_states(ModelLoadStates {
            boards: LoadState::Loaded(vec![board.clone()]),
            columns: LoadState::Loaded(vec![col.clone()]),
            cards: LoadState::Loaded(vec![card.clone()]),
            sprints: LoadState::Loaded(vec![sprint.clone()]),
            graph: LoadState::Loaded(DependencyGraph::default()),
            ..Default::default()
        });
        let changed = m.apply_resolved(Resolved {
            boards: Collection {
                by_id: [(board.id, LoadState::Loaded(board.clone()))].into(),
                ..Default::default()
            },
            columns: Collection {
                by_id: [(col.id, LoadState::Loaded(col.clone()))].into(),
                ..Default::default()
            },
            cards: Collection {
                by_id: [(card.id, LoadState::Loaded(card.clone()))].into(),
                ..Default::default()
            },
            sprints: Collection {
                by_id: [(sprint.id, LoadState::Loaded(sprint.clone()))].into(),
                ..Default::default()
            },
            ..Default::default()
        });
        NoProjections.resync(&m, changed);

        let _ = m.invalidate(Invalidation::All);

        assert!(m.boards_state().is_not_loaded());
        assert!(m.columns_state().is_not_loaded());
        assert!(m.cards_state().is_not_loaded());
        assert!(m.sprints_state().is_not_loaded());
        assert!(m.board_by_id_state(board.id).is_not_loaded());
        assert!(m.column_id_status(col.id).is_not_loaded());
        assert!(m.card_id_status(card.id).is_not_loaded());
        assert!(m.sprint_id_status(sprint.id).is_not_loaded());
    }

    #[test]
    fn test_invalidate_prefixes_flag_drops_the_board_collection() {
        let (mut m, board, col_a, col_b, c1, c2, sprint) = seeded();
        load_every_tier(&mut m, &board, &col_a, &col_b, &c1, &c2, &sprint);

        let _ = m.invalidate(Invalidation::Entities(EntityIds::default().with_prefixes()));

        assert!(m.boards_state().is_not_loaded());
        assert!(m.columns_state().is_loaded());
        assert!(m.cards_state().is_loaded());
        assert!(m.sprints_state().is_loaded());
        assert!(m.graph_state().is_loaded());
        assert!(m.column_id_status(col_a.id).is_loaded());
        assert!(m.card_id_status(c1.id).is_loaded());
        assert!(m.sprint_id_status(sprint.id).is_loaded());
        assert!(m.board_columns_state(board.id).is_loaded());
        assert!(m.board_sprints_state(board.id).is_loaded());
    }

    #[test]
    fn test_invalidate_graph_flag_drops_only_the_graph() {
        let (mut m, _board, col_a, col_b, c1, c2, sprint) = seeded();
        load_every_tier(&mut m, &_board, &col_a, &col_b, &c1, &c2, &sprint);

        let _ = m.invalidate(Invalidation::Entities(EntityIds::default().with_graph()));

        assert!(m.graph_state().is_not_loaded());
        assert!(m.boards_state().is_loaded());
        assert!(m.columns_state().is_loaded());
        assert!(m.cards_state().is_loaded());
        assert!(m.sprints_state().is_loaded());
    }

    #[test]
    fn test_invalidate_entities_with_no_ids_clears_everything() {
        let (mut m, board, col_a, col_b, c1, c2, sprint) = seeded();
        load_every_tier(&mut m, &board, &col_a, &col_b, &c1, &c2, &sprint);

        let _ = m.invalidate(Invalidation::Entities(EntityIds::default()));

        assert_every_tier_not_loaded(&m, &board, &col_a, &col_b, &c1, &c2, &sprint);
    }
}
