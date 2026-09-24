use super::*;
use crate::resolved::Collection;
use crate::{EntityIds, KanbanError, Resolved};
use std::sync::Arc;

fn apply_collection<T: Clone>(
    target: &mut LoadState<Vec<T>>,
    by_id_target: &mut HashMap<Uuid, LoadState<T>>,
    all: LoadState<Vec<T>>,
    by_id: HashMap<Uuid, LoadState<T>>,
    id_of: impl Fn(&T) -> Uuid,
) {
    if !all.is_not_loaded() {
        *target = all;
    }
    if by_id.is_empty() {
        return;
    }
    let mut entries: Vec<(Uuid, LoadState<T>)> = by_id.into_iter().collect();
    entries.sort_unstable_by_key(|(id, _)| *id);
    for (id, state) in entries {
        if state.is_not_loaded() {
            continue;
        }
        by_id_target.insert(id, state.clone());
        let LoadState::Loaded(items) = &mut *target else {
            continue;
        };
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

fn apply_by_id<T>(target: &mut HashMap<Uuid, LoadState<T>>, by_id: HashMap<Uuid, LoadState<T>>) {
    for (id, state) in by_id {
        if state.is_not_loaded() {
            continue;
        }
        target.insert(id, state);
    }
}

fn apply_scopes<T>(
    target: &mut HashMap<Uuid, LoadState<Vec<T>>>,
    incoming: HashMap<Uuid, LoadState<Vec<T>>>,
) {
    for (parent, state) in incoming {
        if !state.is_not_loaded() {
            target.insert(parent, state);
        }
    }
}

fn apply_flat_archival<T>(
    target: &mut Option<Vec<T>>,
    error: &mut Option<Arc<KanbanError>>,
    ids: &mut HashSet<Uuid>,
    all: LoadState<Vec<T>>,
    id_of: impl Fn(&T) -> Uuid,
) {
    match all {
        LoadState::NotLoaded => {}
        LoadState::Loaded(v) => {
            *ids = v.iter().map(&id_of).collect();
            *target = Some(v);
            *error = None;
        }
        LoadState::Failed(e) => {
            *error = Some(e);
        }
        LoadState::Missing => {}
    }
}

impl Model {
    /// Applies one resolve pass across each entity kind's tiers. Boards keep
    /// a flat collection (`all`, then `by_id`); every other kind is per-id
    /// and parent-scoped only (`by_id`, then `by_parent`) — their `all` tier
    /// is never consulted, since nothing populates it anymore. A tier left
    /// `NotLoaded`/empty is untouched.
    ///
    /// Maintains the id indexes only. Returns a [`ModelChanged`] receipt:
    /// whatever derives from this `Model` is stale until a
    /// [`DerivedProjections`] implementor consumes it.
    pub fn apply_resolved(&mut self, resolved: Resolved) -> ModelChanged {
        let boards_touched = !resolved.boards.is_untouched();
        let touched = boards_touched
            || !resolved.cards.is_untouched()
            || !resolved.columns.is_untouched()
            || !resolved.sprints.is_untouched()
            || !resolved.archived_cards.is_untouched()
            || !resolved.archived_boards.is_untouched()
            || !resolved.graph.is_not_loaded();

        let Collection {
            all: boards_all,
            by_id: boards_by_id,
            by_parent: boards_by_parent,
        } = resolved.boards;
        debug_assert!(boards_by_parent.is_empty(), "boards have no parent scope");
        apply_collection(
            &mut self.boards,
            &mut self.boards_by_id,
            boards_all,
            boards_by_id,
            |b| b.id,
        );

        let Collection {
            all: columns_all,
            by_id: columns_by_id,
            by_parent: columns_by_parent,
        } = resolved.columns;
        debug_assert!(columns_all.is_not_loaded(), "columns have no flat tier");
        apply_by_id(&mut self.columns_by_id, columns_by_id);
        apply_scopes(&mut self.columns_by_board, columns_by_parent);

        let Collection {
            all: cards_all,
            by_id: cards_by_id,
            by_parent: cards_by_parent,
        } = resolved.cards;
        debug_assert!(cards_all.is_not_loaded(), "cards have no flat tier");
        apply_by_id(&mut self.cards_by_id, cards_by_id);
        for (column_id, state) in cards_by_parent {
            if state.is_not_loaded() {
                continue;
            }
            self.set_cards_of_column(column_id, state);
        }

        let Collection {
            all: sprints_all,
            by_id: sprints_by_id,
            by_parent: sprints_by_parent,
        } = resolved.sprints;
        debug_assert!(sprints_all.is_not_loaded(), "sprints have no flat tier");
        apply_by_id(&mut self.sprints_by_id, sprints_by_id);
        apply_scopes(&mut self.sprints_by_board, sprints_by_parent);

        apply_scopes(
            &mut self.archived_cards_by_board,
            resolved.archived_cards.by_parent,
        );
        apply_flat_archival(
            &mut self.archived_cards,
            &mut self.archived_cards_error,
            &mut self.archived_card_ids,
            resolved.archived_cards.all,
            |ac| ac.entity_id,
        );
        apply_flat_archival(
            &mut self.archived_boards,
            &mut self.archived_boards_error,
            &mut self.archived_board_ids,
            resolved.archived_boards.all,
            |ab| ab.entity_id,
        );

        if !resolved.graph.is_not_loaded() {
            self.graph = resolved.graph;
        }

        if boards_touched {
            self.rebuild_board_index();
        }

        if touched {
            ModelChanged::new()
        } else {
            ModelChanged::unchanged()
        }
    }

    /// Marks the boards flat collection, and the per-id entries and parent
    /// scopes of every kind named in `ids`, as `Failed(err)`, without
    /// removing any of them. An empty `EntityIds` changes nothing.
    /// `ids.prefixes` has no corresponding `Model` field.
    ///
    /// Maintains the id indexes only. Returns a [`ModelChanged`] receipt:
    /// whatever derives from this `Model` is stale until a
    /// [`DerivedProjections`] implementor consumes it.
    pub fn mark_failed(&mut self, ids: EntityIds, err: Arc<KanbanError>) -> ModelChanged {
        let touched = !ids.is_empty();
        if !ids.boards.is_empty() {
            self.boards = LoadState::Failed(Arc::clone(&err));
            self.rebuild_board_index();
            for state in self.boards_by_id.values_mut() {
                *state = LoadState::Failed(Arc::clone(&err));
            }
        }
        if !ids.columns.is_empty() {
            for state in self.columns_by_board.values_mut() {
                *state = LoadState::Failed(Arc::clone(&err));
            }
            for state in self.columns_by_id.values_mut() {
                *state = LoadState::Failed(Arc::clone(&err));
            }
        }
        if !ids.cards.is_empty() {
            for state in self.cards_by_column.values_mut() {
                *state = LoadState::Failed(Arc::clone(&err));
            }
            for state in self.cards_by_id.values_mut() {
                *state = LoadState::Failed(Arc::clone(&err));
            }
            for state in self.archived_cards_by_board.values_mut() {
                *state = LoadState::Failed(Arc::clone(&err));
            }
        }
        if !ids.sprints.is_empty() {
            for state in self.sprints_by_board.values_mut() {
                *state = LoadState::Failed(Arc::clone(&err));
            }
            for state in self.sprints_by_id.values_mut() {
                *state = LoadState::Failed(Arc::clone(&err));
            }
        }
        if ids.graph {
            self.graph = LoadState::Failed(err);
        }

        if touched {
            ModelChanged::new()
        } else {
            ModelChanged::unchanged()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::resolved::Collection;
    use crate::{ArchivedCard, EntityIds, KanbanError, NoProjections, Resolved};
    use std::sync::Arc;

    #[test]
    fn test_apply_resolved_returns_a_model_changed_receipt() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let card = Card::new(board.id, column.id, "task", 0);
        let mut cards_by_parent = HashMap::new();
        cards_by_parent.insert(column.id, LoadState::Loaded(vec![card]));
        let resolved = Resolved {
            cards: Collection {
                by_parent: cards_by_parent,
                ..Default::default()
            },
            ..Default::default()
        };
        let changed: ModelChanged = m.apply_resolved(resolved);
        assert_eq!(
            m.column_cards_state(column.id)
                .loaded()
                .map(|v| v.len())
                .unwrap_or(0),
            1
        );
        NoProjections.resync(&m, changed);
    }

    #[test]
    fn test_a_receipt_from_an_untouched_apply_reports_nothing_changed() {
        let mut m = Model::default();
        let changed = m.apply_resolved(Resolved::default());
        assert!(!changed.any());
        NoProjections.resync(&m, changed);
    }

    #[test]
    fn test_a_receipt_from_a_touched_apply_reports_changed() {
        let mut m = Model::default();
        assert!(m
            .apply_resolved(Resolved {
                boards: Collection {
                    all: LoadState::Loaded(Vec::new()),
                    ..Default::default()
                },
                ..Default::default()
            })
            .any());

        let card_id = Uuid::new_v4();
        let mut cards_by_id = HashMap::new();
        cards_by_id.insert(
            card_id,
            LoadState::Loaded(Card::new(Uuid::new_v4(), Uuid::new_v4(), "c", 0)),
        );
        assert!(m
            .apply_resolved(Resolved {
                cards: Collection {
                    by_id: cards_by_id,
                    ..Default::default()
                },
                ..Default::default()
            })
            .any());

        let board_id = Uuid::new_v4();
        let mut columns_by_parent = HashMap::new();
        columns_by_parent.insert(board_id, LoadState::Loaded(Vec::new()));
        assert!(m
            .apply_resolved(Resolved {
                columns: Collection {
                    by_parent: columns_by_parent,
                    ..Default::default()
                },
                ..Default::default()
            })
            .any());

        assert!(m
            .apply_resolved(Resolved {
                archived_cards: Collection {
                    all: LoadState::Loaded(Vec::new()),
                    ..Default::default()
                },
                ..Default::default()
            })
            .any());

        assert!(m
            .apply_resolved(Resolved {
                archived_boards: Collection {
                    all: LoadState::Loaded(Vec::new()),
                    ..Default::default()
                },
                ..Default::default()
            })
            .any());

        assert!(m
            .apply_resolved(Resolved {
                graph: LoadState::Loaded(DependencyGraph::default()),
                ..Default::default()
            })
            .any());
    }

    #[test]
    fn test_mark_failed_with_empty_ids_reports_unchanged() {
        let mut m = Model::default();
        let err = Arc::new(KanbanError::unsupported("x"));
        let changed = m.mark_failed(EntityIds::default(), err);
        assert!(!changed.any());
        assert!(m.boards_state().is_not_loaded());
        assert!(m.card_id_status(Uuid::new_v4()).is_not_loaded());
    }

    #[test]
    fn test_mark_failed_with_ids_reports_changed() {
        let mut m = Model::default();
        let card_id = Uuid::new_v4();
        let err = Arc::new(KanbanError::unsupported("x"));
        let changed = m.mark_failed(EntityIds::cards([card_id]), err);
        assert!(changed.any());
    }

    #[test]
    fn test_mark_failed_returns_a_model_changed_receipt() {
        let mut m = Model::default();
        let card_id = Uuid::new_v4();
        let mut by_id = HashMap::new();
        by_id.insert(
            card_id,
            LoadState::Loaded(Card::new(Uuid::new_v4(), Uuid::new_v4(), "c", 0)),
        );
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        let ids = EntityIds {
            cards: [card_id].into_iter().collect(),
            ..Default::default()
        };
        let err = Arc::new(KanbanError::unsupported("boom"));
        let changed: ModelChanged = m.mark_failed(ids, err);
        assert!(m.card_id_status(card_id).is_failed());
        NoProjections.resync(&m, changed);
    }

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
        let _ = m.load_from_snapshot(Snapshot {
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

        let mut cards_by_parent = HashMap::new();
        cards_by_parent.insert(column.id, LoadState::Loaded(vec![new_card.clone()]));
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                by_parent: cards_by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(m.board_columns_state(board.id).is_loaded());
        assert_eq!(
            m.board_columns_state(board.id).loaded().unwrap(),
            &vec![column.clone()]
        );
        assert!(m.board_sprints_state(board.id).is_loaded());
        assert_eq!(
            m.board_sprints_state(board.id).loaded().unwrap(),
            &vec![sprint.clone()]
        );
        assert!(m.boards_state().is_loaded());
        assert_eq!(m.boards_state().loaded().unwrap(), &vec![board]);
        assert!(m.graph_state().is_loaded());
        let state = m.column_cards_state(column.id);
        assert!(state.is_loaded());
        assert_eq!(state.loaded().unwrap(), &vec![new_card]);
    }

    #[test]
    fn test_applying_a_by_id_only_result_updates_only_the_named_ids() {
        let (mut m, board, column, _sprint, card_a, card_b) = seed_full_model();
        let mut a_edited = card_a.clone();
        a_edited.title = "edited".to_string();

        let mut by_id = HashMap::new();
        by_id.insert(card_a.id, LoadState::Loaded(a_edited.clone()));
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::NotLoaded,
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(m.column_cards_state(column.id).loaded().unwrap().len(), 2);
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
        assert!(m.column_cards_state(column.id).is_loaded());
        let _ = board;
    }

    #[test]
    fn test_mark_failed_preserves_the_error_and_scope() {
        let (mut m, board, column, sprint, card_a, card_b) = seed_full_model();
        let err = Arc::new(KanbanError::unsupported("boom"));

        let _ = m.mark_failed(EntityIds::cards([card_a.id]), Arc::clone(&err));

        match m.column_cards_state(column.id) {
            LoadState::Failed(e) => assert!(Arc::ptr_eq(&e, &err)),
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(m.board_columns_state(board.id).is_loaded());
        assert_eq!(
            m.board_columns_state(board.id).loaded().unwrap(),
            &vec![column.clone()]
        );
        assert!(m.board_sprints_state(board.id).is_loaded());
        assert_eq!(
            m.board_sprints_state(board.id).loaded().unwrap(),
            &vec![sprint.clone()]
        );
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
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::NotLoaded,
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(m.card_by_id_state(card_b.id).is_missing());

        let new_sprint = Sprint::new(board.id, 2, None, None::<String>);
        let _ = m.apply_resolved(Resolved {
            sprints: Collection {
                by_parent: [(board.id, LoadState::Loaded(vec![sprint, new_sprint]))].into(),
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(m.card_by_id_state(card_b.id).is_missing());
        assert!(!m.card_by_id_state(card_b.id).is_not_loaded());
        let _ = card_a;
    }

    #[test]
    fn test_applying_a_missing_by_id_entry_reindexes_the_remaining_cards() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let a = seed_card(&board, column.id);
        let b = seed_card(&board, column.id);
        let c = seed_card(&board, column.id);
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![board],
            columns: vec![column],
            cards: vec![a.clone(), b.clone(), c.clone()],
            archived_boards: Vec::new(),
            ..Default::default()
        });

        let mut by_id = HashMap::new();
        by_id.insert(b.id, LoadState::Missing);
        let _ = m.apply_resolved(Resolved {
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
    fn test_apply_resolved_leaves_other_tiers_untouched_for_an_untouched_tier() {
        let mut m = Model::default();
        let board_live = Board::new("Live", None::<String>);
        let board_archived = Board::new("Archived", None::<String>);
        let board_archived_id = board_archived.id;
        let board_live_id = board_live.id;
        let column = Column::new(board_live.id, "Col", 0);
        let column_id = column.id;
        let live_card = seed_card(&board_live, column.id);
        let archived_card = seed_card(&board_live, column.id);
        let archived_card_id = archived_card.id;
        let sprint = Sprint::new(board_live.id, 1, None, None::<String>);
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![board_live.clone(), board_archived.clone()],
            columns: vec![column.clone()],
            cards: vec![live_card, archived_card],
            sprints: vec![sprint.clone()],
            archived_cards: vec![ArchivedCard::new(archived_card_id, Uuid::nil())],
            archived_boards: vec![crate::Archived::now(board_archived_id)],
            ..Default::default()
        });

        let mut sprints_by_parent = HashMap::new();
        sprints_by_parent.insert(
            board_live_id,
            LoadState::Loaded(vec![
                sprint.clone(),
                Sprint::new(board_live_id, 2, None, None::<String>),
            ]),
        );
        let _ = m.apply_resolved(Resolved {
            sprints: Collection {
                by_parent: sprints_by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(m.card_id_status(archived_card_id).is_loaded());
        assert_eq!(m.column_cards_state(column_id).loaded().unwrap().len(), 1);
        assert!(m.board_sprints_state(board_live_id).is_loaded());
        assert_eq!(
            m.board_sprints_state(board_live_id).loaded().unwrap().len(),
            2
        );
    }

    #[test]
    fn test_applying_a_failed_collection_read_marks_the_scoped_tier_failed() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let board_id = board.id;
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![board],
            columns: vec![column],
            archived_boards: Vec::new(),
            ..Default::default()
        });

        let err = Arc::new(KanbanError::unsupported("boom"));
        let mut columns_by_parent = HashMap::new();
        columns_by_parent.insert(board_id, LoadState::Failed(Arc::clone(&err)));
        let _ = m.apply_resolved(Resolved {
            columns: Collection {
                by_parent: columns_by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        match m.board_columns_state(board_id) {
            LoadState::Failed(e) => assert!(Arc::ptr_eq(&e, &err)),
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
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::NotLoaded,
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(m.column_cards_state(Uuid::new_v4()).is_not_loaded());
        assert!(m.card_by_id_state(x_id).is_loaded());
        assert!(m.card_by_id_state(other_id).is_not_loaded());
        assert!(!m.card_by_id_state(other_id).is_missing());
    }

    #[test]
    fn test_mark_failed_with_an_empty_scope_changes_nothing() {
        let (mut m, board, column, sprint, card_a, card_b) = seed_full_model();
        let err = Arc::new(KanbanError::unsupported("boom"));

        let _ = m.mark_failed(EntityIds::default(), err);

        assert!(m.boards_state().is_loaded());
        assert_eq!(m.boards_state().loaded().unwrap(), &vec![board.clone()]);
        assert!(m.board_columns_state(board.id).is_loaded());
        assert_eq!(
            m.board_columns_state(board.id).loaded().unwrap(),
            &vec![column.clone()]
        );
        let cards_state = m.column_cards_state(column.id);
        assert!(cards_state.is_loaded());
        assert_eq!(
            cards_state
                .loaded()
                .unwrap()
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>(),
            vec![card_a.id, card_b.id]
        );
        assert!(m.board_sprints_state(board.id).is_loaded());
        assert_eq!(
            m.board_sprints_state(board.id).loaded().unwrap(),
            &vec![sprint.clone()]
        );
        assert!(m.graph_state().is_loaded());
    }

    #[test]
    fn test_apply_resolved_applies_the_graph_only_when_the_pass_mentions_it() {
        let mut m = Model::default();
        assert!(m.graph_state().is_not_loaded());

        let _ = m.apply_resolved(Resolved {
            graph: LoadState::Loaded(DependencyGraph::default()),
            ..Default::default()
        });
        assert!(m.graph_state().is_loaded());

        let _ = m.apply_resolved(Resolved::default());
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
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let state = m.column_cards_state(column.id);
        let scoped = state.loaded().unwrap();
        assert_eq!(
            scoped.iter().map(|c| c.id).collect::<Vec<_>>(),
            vec![a.id, b.id]
        );
    }

    #[test]
    fn test_a_missing_by_id_result_is_recorded_on_a_not_loaded_collection() {
        let mut m = Model::default();
        let ghost = Uuid::new_v4();
        let mut by_id = HashMap::new();
        by_id.insert(ghost, LoadState::Missing);
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(m.card_by_id_state(ghost).is_missing());
        assert!(m.card_id_status(ghost).is_missing());
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
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(m.card_by_id_state(x_id).loaded().copied().unwrap(), &x);
    }

    #[test]
    fn test_a_not_loaded_by_id_entry_does_not_erase_a_recorded_missing() {
        let mut m = Model::default();
        let ghost = Uuid::new_v4();
        let mut by_id = HashMap::new();
        by_id.insert(ghost, LoadState::Missing);
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        let mut by_id2 = HashMap::new();
        by_id2.insert(ghost, LoadState::NotLoaded);
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                by_id: by_id2,
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(m.card_id_status(ghost).is_missing());
    }

    #[test]
    fn test_a_scoped_write_replaces_the_columns_whole_bucket() {
        let (mut m, board, column, _sprint, card_a, card_b) = seed_full_model();
        let third = seed_card(&board, column.id);

        let mut by_parent = HashMap::new();
        by_parent.insert(column.id, LoadState::Loaded(vec![third.clone()]));
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let state = m.column_cards_state(column.id);
        let scoped = state.loaded().unwrap();
        assert_eq!(
            scoped.iter().map(|c| c.id).collect::<Vec<_>>(),
            vec![third.id]
        );
        let _ = (card_a, card_b);
    }

    #[test]
    fn test_a_loaded_empty_scope_is_not_a_not_loaded_scope() {
        let mut m = Model::default();
        let col = Uuid::new_v4();
        let other_col = Uuid::new_v4();
        let mut by_parent = HashMap::new();
        by_parent.insert(col, LoadState::Loaded(Vec::<Card>::new()));
        let _ = m.apply_resolved(Resolved {
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
        let _ = m.apply_resolved(Resolved {
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
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let mut by_parent2 = HashMap::new();
        by_parent2.insert(column.id, LoadState::NotLoaded);
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                by_parent: by_parent2,
                ..Default::default()
            },
            ..Default::default()
        });

        let state = m.column_cards_state(column.id);
        let scoped = state.loaded().unwrap();
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

        let _ = m.apply_resolved(Resolved {
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
    fn test_a_by_id_result_wins_over_the_scoped_tier() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let mut x_v1 = seed_card(&board, column.id);
        x_v1.title = "v1".to_string();
        let mut x_v2 = x_v1.clone();
        x_v2.title = "v2".to_string();
        let x_id = x_v1.id;

        let mut by_id = HashMap::new();
        by_id.insert(x_id, LoadState::Loaded(x_v2));
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                by_id,
                ..Default::default()
            },
            ..Default::default()
        });

        let mut by_parent = HashMap::new();
        by_parent.insert(column.id, LoadState::Loaded(vec![x_v1]));
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        assert_eq!(m.card_by_id_state(x_id).loaded().unwrap().title, "v2");
        assert_eq!(
            m.column_cards_state(column.id).loaded().unwrap()[0].title,
            "v1"
        );
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
        let _ = m.apply_resolved(Resolved {
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
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let ids = [card_a.id, card_b.id];
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

    #[test]
    fn test_applying_a_board_scoped_archived_result_is_readable_by_board() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let card = Card::new(board.id, column.id, "task", 0);

        let mut by_parent = HashMap::new();
        by_parent.insert(
            board.id,
            LoadState::Loaded(vec![ArchivedCard::new(card.id, board.id)]),
        );
        let _ = m.apply_resolved(Resolved {
            archived_cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let scoped = m
            .board_archived_cards_state(board.id)
            .loaded()
            .copied()
            .unwrap();
        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].entity_id, card.id);

        assert!(m.archived_card_ids().is_empty());
        assert!(m.archived_card_markers().is_empty());
    }

    #[test]
    fn test_applying_a_flat_archived_card_list_feeds_the_marker_set() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let card = Card::new(board.id, column.id, "task", 0);
        let marker = ArchivedCard::new(card.id, board.id);

        let _ = m.apply_resolved(Resolved {
            archived_cards: Collection {
                all: LoadState::Loaded(vec![marker]),
                ..Default::default()
            },
            ..Default::default()
        });

        assert!(m.archived_card_ids().contains(&card.id));

        let mut m2 = Model::default();
        let mut by_parent = HashMap::new();
        by_parent.insert(
            board.id,
            LoadState::Loaded(vec![ArchivedCard::new(card.id, board.id)]),
        );
        let _ = m2.apply_resolved(Resolved {
            archived_cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });
        assert!(m2.archived_card_ids().is_empty());
    }

    #[test]
    fn test_a_failed_archived_read_is_readable_as_failed() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let err = Arc::new(KanbanError::unsupported("boom"));

        let mut by_parent = HashMap::new();
        by_parent.insert(board.id, LoadState::Failed(Arc::clone(&err)));
        let _ = m.apply_resolved(Resolved {
            archived_cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let state = m.board_archived_cards_state(board.id);
        assert!(state.is_failed());
        assert!(!state.is_loaded());
        assert!(!state.is_missing());
    }

    #[test]
    fn test_mark_failed_marks_the_archived_tier_when_cards_are_named() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let card = Card::new(board.id, column.id, "task", 0);

        let mut by_parent = HashMap::new();
        by_parent.insert(
            board.id,
            LoadState::Loaded(vec![ArchivedCard::new(card.id, board.id)]),
        );
        let _ = m.apply_resolved(Resolved {
            archived_cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let err = Arc::new(KanbanError::unsupported("boom"));
        let _ = m.mark_failed(EntityIds::cards([card.id]), Arc::clone(&err));

        let state = m.board_archived_cards_state(board.id);
        assert!(state.is_failed());
        assert!(!state.is_not_loaded());
    }

    #[test]
    fn test_mark_failed_leaves_the_archived_tier_loaded_when_cards_are_not_named() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let card = Card::new(board.id, column.id, "task", 0);

        let mut by_parent = HashMap::new();
        by_parent.insert(
            board.id,
            LoadState::Loaded(vec![ArchivedCard::new(card.id, board.id)]),
        );
        let _ = m.apply_resolved(Resolved {
            archived_cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let err = Arc::new(KanbanError::unsupported("boom"));
        let _ = m.mark_failed(EntityIds::boards([board.id]), err);

        let state = m.board_archived_cards_state(board.id);
        assert!(state.is_loaded());
        assert!(!state.is_failed());
    }

    #[test]
    fn test_load_from_snapshot_clears_the_scoped_archived_tier() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let card = Card::new(board.id, column.id, "task", 0);

        let mut by_parent = HashMap::new();
        by_parent.insert(
            board.id,
            LoadState::Loaded(vec![ArchivedCard::new(card.id, board.id)]),
        );
        let _ = m.apply_resolved(Resolved {
            archived_cards: Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let _ = m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            ..Default::default()
        });

        assert!(m.board_archived_cards_state(board.id).is_not_loaded());
    }

    #[test]
    fn test_load_from_snapshot_clears_every_new_tier() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let sprint = Sprint::new(board.id, 1, None, None::<String>);
        let card = seed_card(&board, column.id);

        let mut cards_by_id = HashMap::new();
        cards_by_id.insert(card.id, LoadState::Loaded(card.clone()));
        let mut columns_by_id = HashMap::new();
        columns_by_id.insert(column.id, LoadState::Loaded(column.clone()));
        let mut sprints_by_id = HashMap::new();
        sprints_by_id.insert(sprint.id, LoadState::Loaded(sprint.clone()));
        let mut columns_by_parent = HashMap::new();
        columns_by_parent.insert(board.id, LoadState::Loaded(vec![column.clone()]));
        let mut cards_by_parent = HashMap::new();
        cards_by_parent.insert(column.id, LoadState::Loaded(vec![card.clone()]));
        let mut sprints_by_parent = HashMap::new();
        sprints_by_parent.insert(board.id, LoadState::Loaded(vec![sprint.clone()]));

        let _ = m.apply_resolved(Resolved {
            columns: Collection {
                by_id: columns_by_id,
                by_parent: columns_by_parent,
                ..Default::default()
            },
            cards: Collection {
                by_id: cards_by_id,
                by_parent: cards_by_parent,
                ..Default::default()
            },
            sprints: Collection {
                by_id: sprints_by_id,
                by_parent: sprints_by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let _ = m.load_from_snapshot(Snapshot::default());

        assert!(m.board_columns_state(board.id).is_not_loaded());
        assert!(m.column_cards_state(column.id).is_not_loaded());
        assert!(m.board_sprints_state(board.id).is_not_loaded());
        assert!(m.card_id_status(card.id).is_not_loaded());
        assert!(m.column_id_status(column.id).is_not_loaded());
        assert!(m.sprint_id_status(sprint.id).is_not_loaded());
    }

    #[test]
    fn test_mark_failed_marks_the_matching_scopes_and_ids_failed() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let sprint = Sprint::new(board.id, 1, None, None::<String>);
        let card = seed_card(&board, column.id);

        m.set_cards_of_column(column.id, LoadState::Loaded(vec![card.clone()]));
        let mut cards_by_id = HashMap::new();
        cards_by_id.insert(card.id, LoadState::Loaded(card.clone()));
        let mut sprints_by_parent = HashMap::new();
        sprints_by_parent.insert(board.id, LoadState::Loaded(vec![sprint.clone()]));
        let _ = m.apply_resolved(Resolved {
            cards: Collection {
                by_id: cards_by_id,
                ..Default::default()
            },
            sprints: Collection {
                by_parent: sprints_by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let err = Arc::new(KanbanError::unsupported("boom"));
        let _ = m.mark_failed(EntityIds::cards([card.id]), Arc::clone(&err));

        match m.column_cards_state(column.id) {
            LoadState::Failed(e) => assert!(Arc::ptr_eq(&e, &err)),
            other => panic!("expected Failed, got {other:?}"),
        }
        match m.card_id_status(card.id) {
            LoadState::Failed(e) => assert!(Arc::ptr_eq(&e, &err)),
            other => panic!("expected Failed, got {other:?}"),
        }
        assert!(m.board_sprints_state(board.id).is_loaded());
    }

    #[test]
    fn test_a_failed_card_scope_does_not_serve_a_card_through_the_scoped_index() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let a = seed_card(&board, column.id);

        m.set_cards_of_column(column.id, LoadState::Loaded(vec![a.clone()]));
        let err = Arc::new(KanbanError::unsupported("boom"));
        let _ = m.mark_failed(EntityIds::cards([a.id]), err);

        assert!(!m.card_by_id_state(a.id).is_loaded());
    }
}
