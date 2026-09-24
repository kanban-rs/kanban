use super::*;

impl Model {
    /// Resolves a card by id in per-id, then parent-scoped precedence order:
    /// a per-id result always wins, then a card found in a loaded column
    /// scope. `NotLoaded` when neither tier names the id.
    pub fn card_by_id_state(&self, id: Uuid) -> LoadState<&Card> {
        if let Some(state) = self.cards_by_id.get(&id) {
            return state.as_ref();
        }
        if let Some(column_id) = self.scoped_card_index.get(&id) {
            if let Some(LoadState::Loaded(cards)) = self.cards_by_column.get(column_id) {
                if let Some(card) = cards.iter().find(|c| c.id == id) {
                    return LoadState::Loaded(card);
                }
            }
        }
        LoadState::NotLoaded
    }

    /// The parent-scoped card tier for one column, independent of the per-id
    /// tier.
    pub fn column_cards_state(&self, column_id: Uuid) -> LoadState<&[Card]> {
        scoped_state(&self.cards_by_column, column_id)
    }

    /// The parent-scoped card tier for one board: the concatenation, in
    /// column order, of every column's `column_cards_state`. `Loaded` only
    /// when the board's column tier and every one of its columns' card
    /// tiers are `Loaded`; `Failed` takes precedence over `Missing` over
    /// `NotLoaded`.
    pub fn board_cards_state(&self, board_id: Uuid) -> LoadState<Vec<&Card>> {
        let columns = match self.board_columns_state(board_id) {
            LoadState::Loaded(columns) => columns,
            LoadState::Failed(e) => return LoadState::Failed(e),
            LoadState::Missing => return LoadState::Missing,
            LoadState::NotLoaded => return LoadState::NotLoaded,
        };

        let mut cards = Vec::new();
        let mut missing = false;
        let mut not_loaded = false;
        for column in columns {
            match self.column_cards_state(column.id) {
                LoadState::Loaded(column_cards) => cards.extend(column_cards.iter()),
                LoadState::Failed(e) => return LoadState::Failed(e),
                LoadState::Missing => missing = true,
                LoadState::NotLoaded => not_loaded = true,
            }
        }

        if missing {
            LoadState::Missing
        } else if not_loaded {
            LoadState::NotLoaded
        } else {
            LoadState::Loaded(cards)
        }
    }

    /// The per-id tier only, with no composition against the flat
    /// collection or the parent-scoped tier. Returns `NotLoaded` for an id
    /// that was never named by a resolve pass.
    pub fn card_id_status(&self, id: Uuid) -> LoadState<&Card> {
        self.cards_by_id
            .get(&id)
            .map(|s| s.as_ref())
            .unwrap_or(LoadState::NotLoaded)
    }

    /// Replaces the card set for one column's scoped tier. The only writer
    /// of `cards_by_column`'s membership: `load_from_snapshot` clears the
    /// map and index together, and `mark_failed` transitions state in place
    /// without touching membership.
    pub(crate) fn set_cards_of_column(&mut self, column_id: Uuid, state: LoadState<Vec<Card>>) {
        self.scoped_card_index.retain(|_, col| *col != column_id);
        if let LoadState::Loaded(cards) = &state {
            for c in cards {
                self.scoped_card_index.insert(c.id, column_id);
            }
        }
        self.cards_by_column.insert(column_id, state);
    }

    /// The archived-card MARKER records (id + archived_at + restore context).
    /// For restore/permanent-delete logic that needs the marker itself, not the
    /// live entity. See `Controller::archived_cards` for the full `Card`
    /// entities.
    pub fn archived_card_markers(&self) -> &[ArchivedCard] {
        self.archived_cards.as_deref().unwrap_or(&[])
    }

    /// Distinguishes a genuinely empty archived-cards tier from one that has
    /// never been absorbed (`archived_card_markers()` returns `&[]` for both).
    pub fn archived_card_markers_absorbed(&self) -> bool {
        self.archived_cards.is_some()
    }

    /// Ids of the archived cards. Rows themselves live in the scoped and
    /// per-id tiers; this set records which of them are archived (built from
    /// the markers). The live/archived partition is a presentation concern
    /// and lives on the view layer's `Controller`; this set is what backs
    /// that split.
    pub fn archived_card_ids(&self) -> &std::collections::HashSet<Uuid> {
        &self.archived_card_ids
    }

    /// The whole-store archived-marker tier, `Loaded` exactly when a snapshot
    /// has supplied it. Independent of `board_archived_cards_state`.
    pub fn archived_cards_state(&self) -> LoadState<&[ArchivedCard]> {
        if let Some(err) = &self.archived_cards_error {
            return LoadState::Failed(std::sync::Arc::clone(err));
        }
        match &self.archived_cards {
            Some(markers) => LoadState::Loaded(markers.as_slice()),
            None => LoadState::NotLoaded,
        }
    }

    /// The parent-scoped archived-marker tier for one board. Independent of
    /// `archived_card_ids()` and of the live/archived partition, which are
    /// derived from a whole-store snapshot: a board-scoped marker list cannot
    /// say which cards of other boards are archived, so it never feeds them.
    pub fn board_archived_cards_state(&self, board_id: Uuid) -> LoadState<&[ArchivedCard]> {
        scoped_state(&self.archived_cards_by_board, board_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArchivedCard, Board, Card, Snapshot};

    fn make_card(board: &Board, column_id: Uuid) -> Card {
        Card::new(board.id, column_id, "task", 0)
    }

    #[test]
    fn test_archived_card_markers_absorbed_distinguishes_never_loaded_from_genuinely_empty() {
        let mut m = Model::default();
        assert!(!m.archived_card_markers_absorbed());

        let _ = m.load_from_snapshot(Snapshot {
            archived_cards: Vec::new(),
            ..Default::default()
        });
        assert!(m.archived_card_markers().is_empty());
        assert!(m.archived_card_markers_absorbed());
    }

    #[test]
    fn test_card_lookup_by_id_returns_correct_card() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let card_a = make_card(&board, col_id);
        let card_b = make_card(&board, col_id);
        let card_b_id = card_b.id;
        let _ = m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![card_a, card_b],
            ..Default::default()
        });
        let found = m.card_by_id_state(card_b_id).loaded().copied().unwrap();
        assert_eq!(found.id, card_b_id);
    }

    #[test]
    fn test_card_by_id_resolves_live_and_archived_from_one_collection() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let live = make_card(&board, col_id);
        let archived = make_card(&board, col_id);
        let live_id = live.id;
        let archived_id = archived.id;
        let _ = m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, uuid::Uuid::nil())],
            ..Default::default()
        });

        // The live row lives in the scoped column tier, the archived row in
        // the per-id tier.
        assert_eq!(m.column_cards_state(col_id).loaded().unwrap().len(), 1);
        assert!(m.card_id_status(archived_id).is_loaded());

        // The single index resolves both.
        assert_eq!(
            m.card_by_id_state(live_id).loaded().copied().map(|c| c.id),
            Some(live_id)
        );
        assert_eq!(
            m.card_by_id_state(archived_id)
                .loaded()
                .copied()
                .map(|c| c.id),
            Some(archived_id)
        );

        // The archived-id set records which rows are archived.
        assert!(!m.archived_card_ids().contains(&live_id));
        assert!(m.archived_card_ids().contains(&archived_id));
    }

    #[test]
    fn test_archived_view_filter_finds_the_archived_card_through_the_per_id_tier() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let live = make_card(&board, col_id);
        let archived = make_card(&board, col_id);
        let archived_id = archived.id;
        let _ = m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, uuid::Uuid::nil())],
            ..Default::default()
        });

        let displayed: Vec<Uuid> = m
            .archived_card_ids()
            .iter()
            .filter(|id| m.card_id_status(**id).is_loaded())
            .copied()
            .collect();
        assert_eq!(displayed, vec![archived_id]);
    }

    #[test]
    fn test_card_by_id_missing_id_returns_none() {
        let m = Model::default();
        assert!(m
            .card_by_id_state(Uuid::new_v4())
            .loaded()
            .copied()
            .is_none());
    }

    #[test]
    fn test_card_by_id_state_is_not_loaded_before_any_snapshot() {
        let m = Model::default();
        let state = m.card_by_id_state(Uuid::new_v4());
        assert!(state.is_not_loaded());
        assert!(!state.is_missing());
    }

    #[test]
    fn test_card_by_id_state_is_loaded_for_a_present_card() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let first = make_card(&board, col_id);
        let second = make_card(&board, col_id);
        let second_id = second.id;
        let _ = m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![first, second],
            ..Default::default()
        });
        let state = m.card_by_id_state(second_id);
        assert!(state.is_loaded());
        assert_eq!(state.loaded().map(|c| c.id), Some(second_id));
    }

    #[test]
    fn test_card_by_id_state_is_loaded_for_an_archived_card() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let live = make_card(&board, col_id);
        let archived = make_card(&board, col_id);
        let archived_id = archived.id;
        let _ = m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, uuid::Uuid::nil())],
            ..Default::default()
        });
        let state = m.card_by_id_state(archived_id);
        assert!(state.is_loaded());
        assert!(!state.is_missing());
        assert!(m.archived_card_ids().contains(&archived_id));
    }

    #[test]
    fn test_load_from_snapshot_lands_archived_card_bodies_in_the_per_id_tier() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let live = make_card(&board, col_id);
        let archived = make_card(&board, col_id);
        let archived_id = archived.id;
        let _ = m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![live, archived.clone()],
            archived_cards: vec![ArchivedCard::new(archived_id, uuid::Uuid::nil())],
            ..Default::default()
        });

        let state = m.card_id_status(archived_id);
        assert!(state.is_loaded());
        assert_eq!(state.loaded().copied().unwrap().title, archived.title);
    }

    #[test]
    fn test_card_by_id_state_answers_not_loaded_for_an_id_absent_from_a_loaded_snapshot() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let card = make_card(&board, col_id);
        let _ = m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![card],
            ..Default::default()
        });

        let state = m.card_by_id_state(Uuid::new_v4());
        assert!(state.is_not_loaded());
        assert!(!state.is_missing());
    }

    #[test]
    fn test_an_unapplied_board_reads_not_loaded() {
        let mut m = Model::default();
        let board_a = Uuid::new_v4();
        let board_b = Uuid::new_v4();
        let card_id = Uuid::new_v4();

        let mut by_parent = std::collections::HashMap::new();
        by_parent.insert(
            board_a,
            crate::LoadState::Loaded(vec![ArchivedCard::new(card_id, board_a)]),
        );
        let _ = m.apply_resolved(crate::Resolved {
            archived_cards: crate::resolved::Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });

        let state = m.board_archived_cards_state(board_b);
        assert!(state.is_not_loaded());
        assert!(!state.is_loaded());
    }

    fn seed_columns_for_board(m: &mut Model, board_id: Uuid, columns: Vec<crate::Column>) {
        let mut by_parent = std::collections::HashMap::new();
        by_parent.insert(board_id, LoadState::Loaded(columns));
        let _ = m.apply_resolved(crate::Resolved {
            columns: crate::resolved::Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });
    }

    fn seed_cards_for_column(m: &mut Model, column_id: Uuid, state: LoadState<Vec<Card>>) {
        let mut by_parent = std::collections::HashMap::new();
        by_parent.insert(column_id, state);
        let _ = m.apply_resolved(crate::Resolved {
            cards: crate::resolved::Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });
    }

    #[test]
    fn test_board_cards_state_concats_loaded_column_tiers_in_column_order() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col_a = crate::Column::new(board.id, "A", 0);
        let col_b = crate::Column::new(board.id, "B", 1);
        let col_a_id = col_a.id;
        let col_b_id = col_b.id;
        seed_columns_for_board(&mut m, board.id, vec![col_a, col_b]);

        let a1 = make_card(&board, col_a_id);
        let b1 = make_card(&board, col_b_id);
        let a1_id = a1.id;
        let b1_id = b1.id;
        seed_cards_for_column(&mut m, col_a_id, LoadState::Loaded(vec![a1]));
        seed_cards_for_column(&mut m, col_b_id, LoadState::Loaded(vec![b1]));

        let state = m.board_cards_state(board.id);
        let ids: Vec<Uuid> = state.loaded().unwrap().iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![a1_id, b1_id]);
    }

    #[test]
    fn test_board_cards_state_not_loaded_when_any_column_tier_not_loaded() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col_a = crate::Column::new(board.id, "A", 0);
        let col_b = crate::Column::new(board.id, "B", 1);
        let col_a_id = col_a.id;
        seed_columns_for_board(&mut m, board.id, vec![col_a, col_b]);

        let a1 = make_card(&board, col_a_id);
        seed_cards_for_column(&mut m, col_a_id, LoadState::Loaded(vec![a1]));

        let state = m.board_cards_state(board.id);
        assert!(state.is_not_loaded());
    }

    #[test]
    fn test_board_cards_state_failed_wins_over_not_loaded() {
        let err = std::sync::Arc::new(KanbanError::unsupported("boom"));

        let mut m1 = Model::default();
        let board1 = Board::new("B", None::<String>);
        let col_a = crate::Column::new(board1.id, "A", 0);
        let col_b = crate::Column::new(board1.id, "B", 1);
        let col_a_id = col_a.id;
        seed_columns_for_board(&mut m1, board1.id, vec![col_a, col_b]);
        seed_cards_for_column(&mut m1, col_a_id, LoadState::Failed(err.clone()));
        let state1 = m1.board_cards_state(board1.id);
        assert!(matches!(state1, LoadState::Failed(e) if std::sync::Arc::ptr_eq(&e, &err)));

        let mut m2 = Model::default();
        let board2 = Board::new("B2", None::<String>);
        let col_a2 = crate::Column::new(board2.id, "A", 0);
        let col_b2 = crate::Column::new(board2.id, "B", 1);
        let col_b2_id = col_b2.id;
        seed_columns_for_board(&mut m2, board2.id, vec![col_a2, col_b2]);
        seed_cards_for_column(&mut m2, col_b2_id, LoadState::Failed(err.clone()));
        let state2 = m2.board_cards_state(board2.id);
        assert!(matches!(state2, LoadState::Failed(e) if std::sync::Arc::ptr_eq(&e, &err)));
    }

    #[test]
    fn test_board_cards_state_returns_not_loaded_for_a_board_with_no_scoped_column_entry() {
        let mut m = Model::default();
        let random_board = Uuid::new_v4();
        assert!(m.board_cards_state(random_board).is_not_loaded());

        let err = std::sync::Arc::new(KanbanError::unsupported("boom"));
        let mut by_parent = std::collections::HashMap::new();
        by_parent.insert(random_board, LoadState::Failed(err.clone()));
        let _ = m.apply_resolved(crate::Resolved {
            columns: crate::resolved::Collection {
                by_parent,
                ..Default::default()
            },
            ..Default::default()
        });
        let state = m.board_cards_state(random_board);
        assert!(matches!(state, LoadState::Failed(e) if std::sync::Arc::ptr_eq(&e, &err)));
    }
}
