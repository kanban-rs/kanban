use super::*;
use kanban_domain::resolved::Collection;
use kanban_domain::{EntityIds, KanbanError, Resolved};
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
            self.rebuild_card_partitions();
        }
        if boards_touched {
            self.rebuild_board_index();
            self.rebuild_board_partitions();
        }
    }

    /// Marks every collection named in `ids` as `Failed(err)`, the finest
    /// granularity `Model` represents. An empty `EntityIds` changes nothing.
    /// `ids.prefixes` has no corresponding `Model` field.
    pub fn mark_failed(&mut self, ids: EntityIds, err: Arc<KanbanError>) {
        if !ids.boards.is_empty() {
            self.boards = LoadState::Failed(Arc::clone(&err));
            self.rebuild_board_index();
            self.rebuild_board_partitions();
        }
        if !ids.columns.is_empty() {
            self.columns = LoadState::Failed(Arc::clone(&err));
        }
        if !ids.cards.is_empty() {
            self.cards = LoadState::Failed(Arc::clone(&err));
            self.rebuild_card_index();
            self.rebuild_card_partitions();
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
    use kanban_domain::resolved::Collection;
    use kanban_domain::{ArchivedCard, EntityIds, KanbanError, Resolved};
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
            },
            ..Default::default()
        });

        assert!(m.card_by_id_state(b.id).is_missing());
        assert_eq!(m.card_by_id_state(c.id).loaded().copied().unwrap().id, c.id);
        assert_eq!(m.card_by_id_state(a.id).loaded().copied().unwrap().id, a.id);
    }

    #[test]
    fn test_apply_resolved_rebuilds_the_displayed_partitions() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let live = seed_card(&board, column.id);
        let archived = seed_card(&board, column.id);
        let live_id = live.id;
        let archived_id = archived.id;
        m.load_from_snapshot(Snapshot {
            boards: vec![board.clone()],
            columns: vec![column.clone()],
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, Uuid::nil())],
            archived_boards: Vec::new(),
            ..Default::default()
        });

        let mut live_edited = seed_card(&board, column.id);
        live_edited.id = live_id;
        live_edited.title = "live edited".to_string();
        let mut archived_edited = seed_card(&board, column.id);
        archived_edited.id = archived_id;
        archived_edited.title = "archived edited".to_string();
        let extra_live = seed_card(&board, column.id);
        let extra_live_id = extra_live.id;

        m.apply_resolved(Resolved {
            cards: Collection {
                all: LoadState::Loaded(vec![
                    live_edited.clone(),
                    archived_edited.clone(),
                    extra_live.clone(),
                ]),
                ..Default::default()
            },
            ..Default::default()
        });

        let live_ids: Vec<Uuid> = m.displayed_cards(false).iter().map(|c| c.id).collect();
        let archived_ids: Vec<Uuid> = m.displayed_cards(true).iter().map(|c| c.id).collect();
        assert_eq!(live_ids, vec![live_id, extra_live_id]);
        assert_eq!(archived_ids, vec![archived_id]);
        assert_eq!(
            m.displayed_cards(false)
                .iter()
                .find(|c| c.id == live_id)
                .unwrap()
                .title,
            "live edited"
        );
        assert_eq!(
            m.displayed_cards(true)
                .iter()
                .find(|c| c.id == archived_id)
                .unwrap()
                .title,
            "archived edited"
        );
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
            archived_boards: vec![kanban_domain::Archived::now(board_archived_id)],
            ..Default::default()
        });

        let card_index_before = m.card_index.clone();
        let board_index_before = m.board_index.clone();
        let dcl_before = m.displayed_cards_live.clone();
        let dca_before = m.displayed_cards_archived.clone();
        let dbl_before = m.displayed_boards_live.clone();
        let dba_before = m.displayed_boards_archived.clone();

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
        assert_eq!(m.displayed_cards_live, dcl_before);
        assert_eq!(m.displayed_cards_archived, dca_before);
        assert_eq!(m.displayed_boards_live, dbl_before);
        assert_eq!(m.displayed_boards_archived, dba_before);
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
            },
            ..Default::default()
        });

        assert!(m.cards_state().is_not_loaded());
        assert!(m.card_by_id_state(x_id).is_not_loaded());
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
}
