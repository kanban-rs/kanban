use super::*;
use crate::resolved::Collection;
use crate::{EntityIds, KanbanError, Resolved};
use std::sync::Arc;

fn apply_collection<T>(
    target: &mut LoadState<Vec<T>>,
    incoming: Collection<T>,
    id_of: impl Fn(&T) -> Uuid,
) {
    if !incoming.all.is_not_loaded() {
        *target = incoming.all;
    }
    if incoming.by_id.is_empty() {
        return;
    }
    let LoadState::Loaded(items) = target else {
        return;
    };
    let mut entries: Vec<(Uuid, LoadState<T>)> = incoming.by_id.into_iter().collect();
    entries.sort_unstable_by_key(|(id, _)| *id);
    for (id, state) in entries {
        match state {
            LoadState::Loaded(entity) => match items.iter().position(|e| id_of(e) == id) {
                Some(pos) => items[pos] = entity,
                None => items.push(entity),
            },
            LoadState::Missing => items.retain(|e| id_of(e) != id),
            LoadState::NotLoaded | LoadState::Failed(_) => {}
        }
    }
}

impl Model {
    /// Applies one resolve pass. Each tier is left exactly as it was when its
    /// `Collection` is untouched (`all` is `NotLoaded` and `by_id` is empty).
    /// Otherwise `all` replaces the whole tier first (any of `Loaded`,
    /// `Missing`, `Failed`), then every `by_id` entry is applied on top; a
    /// `by_id` entry onto a tier that is not `Loaded` is dropped rather than
    /// promoted, since promoting it would report every other entity in that
    /// tier as `Missing`. `graph` follows the same not-`NotLoaded` rule.
    ///
    /// Maintains the id indexes only. A caller must follow with
    /// `Controller::sync` so the view layer's derived partitions do not lag.
    pub fn apply_resolved(&mut self, resolved: Resolved) {
        let boards_touched = !resolved.boards.is_untouched();
        let cards_touched = !resolved.cards.is_untouched();

        apply_collection(&mut self.boards, resolved.boards, |b| b.id);
        apply_collection(&mut self.columns, resolved.columns, |c| c.id);
        apply_collection(&mut self.cards, resolved.cards, |c| c.id);
        apply_collection(&mut self.sprints, resolved.sprints, |s| s.id);

        if !resolved.graph.is_not_loaded() {
            self.graph = resolved.graph;
        }

        if cards_touched {
            self.rebuild_card_index();
        }
        if boards_touched {
            self.rebuild_board_index();
        }
    }

    /// Marks every collection named in `ids` as `Failed(err)`, the finest
    /// granularity `Model` represents. An empty `EntityIds` changes nothing.
    /// `ids.prefixes` has no corresponding `Model` field.
    ///
    /// Maintains the id indexes only. A caller must follow with
    /// `Controller::sync` so the view layer's derived partitions do not lag.
    pub fn mark_failed(&mut self, ids: EntityIds, err: Arc<KanbanError>) {
        if !ids.boards.is_empty() {
            self.boards = LoadState::Failed(Arc::clone(&err));
            self.rebuild_board_index();
        }
        if !ids.columns.is_empty() {
            self.columns = LoadState::Failed(Arc::clone(&err));
        }
        if !ids.cards.is_empty() {
            self.cards = LoadState::Failed(Arc::clone(&err));
            self.rebuild_card_index();
        }
        if !ids.sprints.is_empty() {
            self.sprints = LoadState::Failed(Arc::clone(&err));
        }
        if ids.graph {
            self.graph = LoadState::Failed(err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::resolved::Collection;
    use crate::{ArchivedCard, EntityIds, KanbanError, Resolved};
    use std::sync::Arc;

    fn seed_card(board: &Board, column_id: Uuid) -> Card {
        Card::new(board.id, column_id, "task", 0)
    }

    fn seed_full_model() -> (Model, Board, Column, Sprint, Card, Card) {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let sprint = Sprint::new(board.id, 1, None, None::<String>);
        let card_a = seed_card(&board, column.id);
        let card_b = seed_card(&board, column.id);
        m.load_from_snapshot(Snapshot {
            boards: vec![board.clone()],
            columns: vec![column.clone()],
            sprints: vec![sprint.clone()],
            cards: vec![card_a.clone(), card_b.clone()],
            archived_boards: Vec::new(),
            ..Default::default()
        });
        (m, board, column, sprint, card_a, card_b)
    }

    #[test]
    fn test_applying_a_cards_only_result_leaves_other_entities_untouched() {
        let (mut m, board, column, sprint, _card_a, _card_b) = seed_full_model();
        let new_card = seed_card(&board, column.id);

        m.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::Loaded(vec![new_card.clone()]),
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(m.columns_state().is_loaded());
        assert_eq!(m.columns_state().loaded().unwrap(), &vec![column]);
        assert!(m.sprints_state().is_loaded());
        assert_eq!(m.sprints_state().loaded().unwrap(), &vec![sprint]);
        assert!(m.boards_state().is_loaded());
        assert_eq!(m.boards_state().loaded().unwrap(), &vec![board]);
        assert!(m.graph_state().is_loaded());
        assert!(m.cards_state().is_loaded());
        assert_eq!(m.cards_state().loaded().unwrap(), &vec![new_card]);
    }

    #[test]
    fn test_applying_all_then_by_id_applies_the_whole_collection_first() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let a = Column::new(board.id, "A", 0);
        let b = Column::new(board.id, "B", 1);
        m.load_from_snapshot(Snapshot {
            boards: vec![board],
            archived_boards: Vec::new(),
            ..Default::default()
        });

        let mut by_id = HashMap::new();
        by_id.insert(b.id, LoadState::Missing);
        m.apply_resolved(Resolved {
            columns: Collection {
                all: LoadState::Loaded(vec![a.clone(), b]),
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(m.columns_state().loaded().unwrap(), &vec![a]);
    }

    #[test]
    fn test_applying_a_by_id_only_result_updates_only_the_named_ids() {
        let (mut m, board, column, _sprint, card_a, card_b) = seed_full_model();
        let mut a_edited = card_a.clone();
        a_edited.title = "edited".to_string();

        let mut by_id = HashMap::new();
        by_id.insert(card_a.id, LoadState::Loaded(a_edited.clone()));
        m.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::NotLoaded,
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(m.cards_state().loaded().unwrap().len(), 2);
        assert_eq!(
            m.card_by_id_state(card_a.id)
                .loaded()
                .copied()
                .unwrap()
                .title,
            "edited"
        );
        assert_eq!(
            m.card_by_id_state(card_b.id).loaded().copied().unwrap(),
            &card_b
        );
        assert!(m.cards_state().is_loaded());
        let _ = (board, column);
    }

    #[test]
    fn test_mark_failed_preserves_the_error_and_scope() {
        let (mut m, board, column, sprint, card_a, card_b) = seed_full_model();
        let err = Arc::new(KanbanError::unsupported("boom"));

        m.mark_failed(EntityIds::cards([card_a.id]), Arc::clone(&err));

        match m.cards_state() {
            LoadState::Failed(e) => assert!(Arc::ptr_eq(e, &err)),
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(m.columns_state().is_loaded());
        assert_eq!(m.columns_state().loaded().unwrap(), &vec![column]);
        assert!(m.sprints_state().is_loaded());
        assert_eq!(m.sprints_state().loaded().unwrap(), &vec![sprint]);
        assert!(m.boards_state().is_loaded());
        assert_eq!(m.boards_state().loaded().unwrap(), &vec![board]);
        assert!(m.graph_state().is_loaded());
        let _ = card_b;
    }

    #[test]
    fn test_a_missing_entity_stays_missing_across_a_later_apply() {
        let (mut m, board, _column, sprint, card_a, card_b) = seed_full_model();
        let mut by_id = HashMap::new();
        by_id.insert(card_b.id, LoadState::Missing);
        m.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::NotLoaded,
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(m.card_by_id_state(card_b.id).is_missing());

        let new_sprint = Sprint::new(board.id, 2, None, None::<String>);
        m.apply_resolved(Resolved {
            sprints: Collection {
                all: LoadState::Loaded(vec![sprint, new_sprint]),
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(m.card_by_id_state(card_b.id).is_missing());
        assert!(!m.card_by_id_state(card_b.id).is_not_loaded());
        let _ = card_a;
    }

    #[test]
    fn test_apply_resolved_rebuilds_the_card_index_for_a_replaced_collection() {
        let (mut m, board, column, _sprint, a, _b) = seed_full_model();
        let c = seed_card(&board, column.id);
        let d = seed_card(&board, column.id);
        let e = seed_card(&board, column.id);

        m.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::Loaded(vec![c.clone(), d.clone(), e.clone()]),
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(m.card_index.len(), 3);
        for (i, card) in m.cards_state().loaded().unwrap().iter().enumerate() {
            assert_eq!(m.card_index[&card.id], i);
        }
        assert!(m.card_by_id_state(a.id).loaded().copied().is_none());
        assert!(m.card_by_id_state(a.id).is_missing());
        assert_eq!(m.card_by_id_state(c.id).loaded().copied().unwrap(), &c);
        assert_eq!(m.card_by_id_state(d.id).loaded().copied().unwrap(), &d);
        assert_eq!(m.card_by_id_state(e.id).loaded().copied().unwrap(), &e);
    }

    #[test]
    fn test_applying_a_missing_by_id_entry_reindexes_the_remaining_cards() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let a = seed_card(&board, column.id);
        let b = seed_card(&board, column.id);
        let c = seed_card(&board, column.id);
        m.load_from_snapshot(Snapshot {
            boards: vec![board],
            columns: vec![column],
            cards: vec![a.clone(), b.clone(), c.clone()],
            archived_boards: Vec::new(),
            ..Default::default()
        });

        let mut by_id = HashMap::new();
        by_id.insert(b.id, LoadState::Missing);
        m.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::NotLoaded,
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(m.card_by_id_state(b.id).is_missing());
        assert_eq!(m.card_by_id_state(c.id).loaded().copied().unwrap().id, c.id);
        assert_eq!(m.card_by_id_state(a.id).loaded().copied().unwrap().id, a.id);
    }

    #[test]
    fn test_apply_resolved_leaves_indexes_untouched_for_an_untouched_tier() {
        let mut m = Model::default();
        let board_live = Board::new("Live", None::<String>);
        let board_archived = Board::new("Archived", None::<String>);
        let board_archived_id = board_archived.id;
        let column = Column::new(board_live.id, "Col", 0);
        let live_card = seed_card(&board_live, column.id);
        let archived_card = seed_card(&board_live, column.id);
        let archived_card_id = archived_card.id;
        let sprint = Sprint::new(board_live.id, 1, None, None::<String>);
        m.load_from_snapshot(Snapshot {
            boards: vec![board_live.clone(), board_archived.clone()],
            columns: vec![column.clone()],
            cards: vec![live_card, archived_card],
            sprints: vec![sprint.clone()],
            archived_cards: vec![ArchivedCard::new(archived_card_id, Uuid::nil())],
            archived_boards: vec![crate::Archived::now(board_archived_id)],
            ..Default::default()
        });

        let card_index_before = m.card_index.clone();
        let board_index_before = m.board_index.clone();

        let new_sprint = Sprint::new(board_live.id, 2, None, None::<String>);
        m.apply_resolved(Resolved {
            sprints: Collection {
                all: LoadState::Loaded(vec![sprint, new_sprint]),
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(m.card_index, card_index_before);
        assert_eq!(m.board_index, board_index_before);
        assert!(m.sprints_state().is_loaded());
        assert_eq!(m.sprints_state().loaded().unwrap().len(), 2);
    }

    #[test]
    fn test_applying_a_failed_collection_read_marks_the_model_collection_failed() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        m.load_from_snapshot(Snapshot {
            boards: vec![board],
            columns: vec![column],
            archived_boards: Vec::new(),
            ..Default::default()
        });

        let err = Arc::new(KanbanError::unsupported("boom"));
        m.apply_resolved(Resolved {
            columns: Collection {
                all: LoadState::Failed(Arc::clone(&err)),
                ..Default::default()
            },
            ..Default::default()
        });

        match m.columns_state() {
            LoadState::Failed(e) => assert!(Arc::ptr_eq(e, &err)),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn test_applying_a_by_id_entry_onto_a_not_loaded_collection_leaves_it_not_loaded() {
        let mut m = Model::default();
        let x = Card::new(Uuid::new_v4(), Uuid::new_v4(), "x", 0);
        let x_id = x.id;
        let other_id = Uuid::new_v4();

        let mut by_id = HashMap::new();
        by_id.insert(x_id, LoadState::Loaded(x));
        m.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::NotLoaded,
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(m.cards_state().is_not_loaded());
        assert!(m.card_by_id_state(x_id).is_loaded());
        assert!(m.card_by_id_state(other_id).is_not_loaded());
        assert!(!m.card_by_id_state(other_id).is_missing());
    }

    #[test]
    fn test_mark_failed_with_an_empty_scope_changes_nothing() {
        let (mut m, board, column, sprint, card_a, card_b) = seed_full_model();
        let err = Arc::new(KanbanError::unsupported("boom"));

        m.mark_failed(EntityIds::default(), err);

        assert!(m.boards_state().is_loaded());
        assert_eq!(m.boards_state().loaded().unwrap(), &vec![board]);
        assert!(m.columns_state().is_loaded());
        assert_eq!(m.columns_state().loaded().unwrap(), &vec![column]);
        assert!(m.cards_state().is_loaded());
        assert_eq!(
            m.cards_state().loaded().unwrap(),
            &vec![card_a.clone(), card_b.clone()]
        );
        assert!(m.sprints_state().is_loaded());
        assert_eq!(m.sprints_state().loaded().unwrap(), &vec![sprint]);
        assert!(m.graph_state().is_loaded());
    }

    #[test]
    fn test_apply_resolved_applies_the_graph_only_when_the_pass_mentions_it() {
        let mut m = Model::default();
        assert!(m.graph_state().is_not_loaded());

        m.apply_resolved(Resolved {
            graph: LoadState::Loaded(DependencyGraph::default()),
            ..Default::default()
        });
        assert!(m.graph_state().is_loaded());

        m.apply_resolved(Resolved::default());
        assert!(m.graph_state().is_loaded());
    }

    #[test]
    fn test_applying_a_scoped_cards_result_is_readable_back() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let a = seed_card(&board, column.id);
        let b = seed_card(&board, column.id);

        let mut by_parent = HashMap::new();
        by_parent.insert(column.id, LoadState::Loaded(vec![a.clone(), b.clone()]));
        m.apply_resolved(Resolved {
            cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let scoped = m.column_cards_state(column.id).loaded().unwrap();
        assert_eq!(scoped.iter().map(|c| c.id).collect::<Vec<_>>(), vec![a.id, b.id]);
        assert!(m.cards_state().is_not_loaded());
    }

    #[test]
    fn test_a_missing_by_id_result_is_recorded_on_a_not_loaded_collection() {
        let mut m = Model::default();
        let ghost = Uuid::new_v4();
        let mut by_id = HashMap::new();
        by_id.insert(ghost, LoadState::Missing);
        m.apply_resolved(Resolved {
            cards: Collection {
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(m.card_by_id_state(ghost).is_missing());
        assert!(m.card_id_status(ghost).is_missing());
        assert!(m.cards_state().is_not_loaded());
    }

    #[test]
    fn test_a_loaded_by_id_result_lands_on_a_not_loaded_collection() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let x = seed_card(&board, column.id);
        let x_id = x.id;
        let mut by_id = HashMap::new();
        by_id.insert(x_id, LoadState::Loaded(x.clone()));
        m.apply_resolved(Resolved {
            cards: Collection {
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(m.card_by_id_state(x_id).loaded().copied().unwrap(), &x);
        assert!(m.cards_state().is_not_loaded());
    }

    #[test]
    fn test_a_not_loaded_by_id_entry_does_not_erase_a_recorded_missing() {
        let mut m = Model::default();
        let ghost = Uuid::new_v4();
        let mut by_id = HashMap::new();
        by_id.insert(ghost, LoadState::Missing);
        m.apply_resolved(Resolved {
            cards: Collection {
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        let mut by_id2 = HashMap::new();
        by_id2.insert(ghost, LoadState::NotLoaded);
        m.apply_resolved(Resolved {
            cards: Collection {
                by_id: by_id2,
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(m.card_id_status(ghost).is_missing());
    }

    #[test]
    fn test_a_scoped_result_never_touches_the_flat_collection() {
        let (mut m, board, column, _sprint, card_a, card_b) = seed_full_model();
        let third = seed_card(&board, column.id);

        let mut by_parent = HashMap::new();
        by_parent.insert(column.id, LoadState::Loaded(vec![third.clone()]));
        m.apply_resolved(Resolved {
            cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(
            m.cards_state().loaded().unwrap(),
            &vec![card_a.clone(), card_b.clone()]
        );
        let scoped = m.column_cards_state(column.id).loaded().unwrap();
        assert_eq!(scoped.iter().map(|c| c.id).collect::<Vec<_>>(), vec![third.id]);
    }

    #[test]
    fn test_a_loaded_empty_scope_is_not_a_not_loaded_scope() {
        let mut m = Model::default();
        let col = Uuid::new_v4();
        let other_col = Uuid::new_v4();
        let mut by_parent = HashMap::new();
        by_parent.insert(col, LoadState::Loaded(Vec::<Card>::new()));
        m.apply_resolved(Resolved {
            cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(m.column_cards_state(col).is_loaded());
        assert!(m.column_cards_state(col).loaded().unwrap().is_empty());
        assert!(m.column_cards_state(other_col).is_not_loaded());
    }

    #[test]
    fn test_a_failed_scope_is_applied_as_failed() {
        let mut m = Model::default();
        let col = Uuid::new_v4();
        let err = Arc::new(KanbanError::unsupported("boom"));
        let mut by_parent = HashMap::new();
        by_parent.insert(col, LoadState::Failed(Arc::clone(&err)));
        m.apply_resolved(Resolved {
            cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        match m.column_cards_state(col) {
            LoadState::Failed(e) => assert!(Arc::ptr_eq(&e, &err)),
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(!m.column_cards_state(col).is_missing());
    }

    #[test]
    fn test_a_not_loaded_scope_entry_leaves_an_existing_scope_alone() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let a = seed_card(&board, column.id);

        let mut by_parent = HashMap::new();
        by_parent.insert(column.id, LoadState::Loaded(vec![a.clone()]));
        m.apply_resolved(Resolved {
            cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let mut by_parent2 = HashMap::new();
        by_parent2.insert(column.id, LoadState::NotLoaded);
        m.apply_resolved(Resolved {
            cards: Collection {
                by_parent: by_parent2,
                ..Default::default()
            },
            ..Default::default()
        });

        let scoped = m.column_cards_state(column.id).loaded().unwrap();
        assert_eq!(scoped.iter().map(|c| c.id).collect::<Vec<_>>(), vec![a.id]);
    }

    #[test]
    fn test_every_scoped_kind_is_applied() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let sprint = Sprint::new(board.id, 1, None, None::<String>);
        let card = seed_card(&board, column.id);

        let mut columns_by_parent = HashMap::new();
        columns_by_parent.insert(board.id, LoadState::Loaded(vec![column.clone()]));
        let mut cards_by_parent = HashMap::new();
        cards_by_parent.insert(column.id, LoadState::Loaded(vec![card.clone()]));
        let mut sprints_by_parent = HashMap::new();
        sprints_by_parent.insert(board.id, LoadState::Loaded(vec![sprint.clone()]));

        m.apply_resolved(Resolved {
            columns: Collection {
                by_parent: columns_by_parent,
                ..Default::default()
            },
            cards: Collection {
                by_parent: cards_by_parent,
                ..Default::default()
            },
            sprints: Collection {
                by_parent: sprints_by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(
            m.board_columns_state(board.id)
                .loaded()
                .unwrap()
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>(),
            vec![column.id]
        );
        assert_eq!(
            m.column_cards_state(column.id)
                .loaded()
                .unwrap()
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>(),
            vec![card.id]
        );
        assert_eq!(
            m.board_sprints_state(board.id)
                .loaded()
                .unwrap()
                .iter()
                .map(|s| s.id)
                .collect::<Vec<_>>(),
            vec![sprint.id]
        );
    }

    #[test]
    fn test_applying_all_and_by_parent_in_one_pass_keeps_them_independent() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let x = seed_card(&board, column.id);
        let y = seed_card(&board, column.id);

        let mut by_parent = HashMap::new();
        by_parent.insert(column.id, LoadState::Loaded(vec![y.clone()]));
        m.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::Loaded(vec![x.clone()]),
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(m.cards_state().loaded().unwrap(), &vec![x]);
        let scoped = m.column_cards_state(column.id).loaded().unwrap();
        assert_eq!(scoped.iter().map(|c| c.id).collect::<Vec<_>>(), vec![y.id]);
    }

    #[test]
    fn test_a_by_id_result_wins_over_the_flat_collection() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let mut x_v1 = seed_card(&board, column.id);
        x_v1.title = "v1".to_string();
        let mut x_v2 = x_v1.clone();
        x_v2.title = "v2".to_string();

        let mut by_id = HashMap::new();
        by_id.insert(x_v1.id, LoadState::Loaded(x_v2));
        m.apply_resolved(Resolved {
            cards: Collection {
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        m.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::Loaded(vec![x_v1]),
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(
            m.card_by_id_state(m.cards_state().loaded().unwrap()[0].id)
                .loaded()
                .unwrap()
                .title,
            "v2"
        );
        assert_eq!(m.cards_state().loaded().unwrap()[0].title, "v1");
    }

    #[test]
    fn test_a_scope_loaded_card_is_readable_by_id() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column_x = Column::new(board.id, "X", 0);
        let card_a = seed_card(&board, column_x.id);
        let card_b = seed_card(&board, column_x.id);

        let mut by_parent = HashMap::new();
        by_parent.insert(
            column_x.id,
            LoadState::Loaded(vec![card_a.clone(), card_b.clone()]),
        );
        m.apply_resolved(Resolved {
            cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(
            m.card_by_id_state(card_a.id).loaded().copied(),
            Some(&card_a)
        );
        assert_eq!(
            m.card_by_id_state(card_b.id).loaded().copied(),
            Some(&card_b)
        );
    }

    #[test]
    fn test_a_scope_loaded_card_list_renders_every_row() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column_x = Column::new(board.id, "X", 0);
        let card_a = seed_card(&board, column_x.id);
        let card_b = seed_card(&board, column_x.id);

        let mut by_parent = HashMap::new();
        by_parent.insert(
            column_x.id,
            LoadState::Loaded(vec![card_a.clone(), card_b.clone()]),
        );
        m.apply_resolved(Resolved {
            cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let ids = vec![card_a.id, card_b.id];
        let rows: Vec<_> = ids
            .iter()
            .filter_map(|id| m.card_by_id_state(*id).loaded().copied())
            .collect();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn test_replacing_a_columns_cards_drops_the_old_ids_from_the_scoped_index() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let a = seed_card(&board, column.id);
        let b = seed_card(&board, column.id);

        m.set_cards_of_column(column.id, LoadState::Loaded(vec![a.clone()]));
        m.set_cards_of_column(column.id, LoadState::Loaded(vec![b.clone()]));

        assert!(m.card_by_id_state(b.id).is_loaded());
        assert!(m.card_by_id_state(a.id).is_not_loaded());
    }
}
