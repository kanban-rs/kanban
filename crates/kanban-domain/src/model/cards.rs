use super::*;

impl Model {
    /// The full unified live+archived collection. Only for callers that
    /// genuinely need id resolution regardless of archival status — see the
    /// view layer's `Controller` for the common display case.
    pub fn cards_state(&self) -> &LoadState<Vec<Card>> {
        &self.cards
    }

    /// Resolve a card by id from the single unified collection (live AND
    /// archived rows). One index lookup — no live/archived re-join. A card is
    /// a card regardless of whether its head is archived.
    pub fn card_by_id_state(&self, id: Uuid) -> LoadState<&Card> {
        match self.cards.as_ref() {
            LoadState::Loaded(cards) => {
                match self.card_index.get(&id).and_then(|&idx| cards.get(idx)) {
                    Some(card) => LoadState::Loaded(card),
                    None => LoadState::Missing,
                }
            }
            LoadState::NotLoaded => LoadState::NotLoaded,
            LoadState::Missing => LoadState::Missing,
            LoadState::Failed(e) => LoadState::Failed(e),
        }
    }

    /// The archived-card MARKER records (id + archived_at + restore context).
    /// For restore/permanent-delete logic that needs the marker itself, not the
    /// live entity. See `Controller::archived_cards` for the full `Card`
    /// entities.
    pub fn archived_card_markers(&self) -> &[ArchivedCard] {
        self.archived_cards.as_deref().unwrap_or(&[])
    }

    /// Ids of the archived cards. Rows themselves live in the unified `cards_state()`
    /// collection; this set records which of them are archived (built from the
    /// markers). The live/archived partition is a presentation concern and lives
    /// on the view layer's `Controller`; this set is what backs that split.
    pub fn archived_card_ids(&self) -> &std::collections::HashSet<Uuid> {
        &self.archived_card_ids
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
    fn test_card_lookup_by_id_returns_correct_card() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let card_a = make_card(&board, col_id);
        let card_b = make_card(&board, col_id);
        let card_b_id = card_b.id;
        m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![card_a, card_b],
            ..Default::default()
        });
        let found = m.card_by_id_state(card_b_id).loaded().copied().unwrap();
        assert_eq!(found.id, card_b_id);
    }

    #[test]
    fn test_card_by_id_resolves_live_and_archived_from_one_collection() {
        // After unification `cards_state()` holds live AND archived rows, and
        // `card_by_id_state` resolves either from the single collection — no
        // `or_else(archived_card())` re-join.
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let live = make_card(&board, col_id);
        let archived = make_card(&board, col_id);
        let live_id = live.id;
        let archived_id = archived.id;
        m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, uuid::Uuid::nil())],
            ..Default::default()
        });

        // Both live and archived rows live in the single unified collection.
        assert_eq!(m.cards_state().loaded_or_empty().len(), 2);

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
    fn test_archived_view_filter_shows_archived_card_from_unified_collection() {
        // `archived_card_ids` records the archived subset of the unified `cards_state()`
        // collection. Assert an
        // archived card is reachable by filtering `cards_state()` through that set.
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let live = make_card(&board, col_id);
        let archived = make_card(&board, col_id);
        let archived_id = archived.id;
        m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![live, archived],
            archived_cards: vec![ArchivedCard::new(archived_id, uuid::Uuid::nil())],
            ..Default::default()
        });

        let displayed: Vec<Uuid> = m
            .cards_state()
            .loaded_or_empty()
            .iter()
            .filter(|c| m.archived_card_ids().contains(&c.id))
            .map(|c| c.id)
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
    fn test_cards_state_is_not_loaded_before_load_from_snapshot() {
        let m = Model::default();
        assert!(m.cards_state().is_not_loaded());
    }

    #[test]
    fn test_cards_state_is_loaded_and_empty_after_an_empty_snapshot() {
        let mut m = Model::default();
        m.load_from_snapshot(Snapshot::default());
        assert!(m.cards_state().is_loaded());
        assert!(m.cards_state().loaded().unwrap().is_empty());
        assert!(m.cards_state().loaded_or_empty().is_empty());
    }

    #[test]
    fn test_card_by_id_state_is_not_loaded_before_any_snapshot() {
        let m = Model::default();
        let state = m.card_by_id_state(Uuid::new_v4());
        assert!(state.is_not_loaded());
        assert!(!state.is_missing());
    }

    #[test]
    fn test_card_by_id_state_is_missing_for_an_absent_card_after_load() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let card = make_card(&board, col_id);
        m.load_from_snapshot(Snapshot {
            archived_boards: Vec::new(),
            cards: vec![card],
            ..Default::default()
        });
        let state = m.card_by_id_state(Uuid::new_v4());
        assert!(state.is_missing());
        assert!(state.is_terminal());
        assert!(!state.is_not_loaded());
    }

    #[test]
    fn test_card_by_id_state_is_loaded_for_a_present_card() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col_id = Uuid::new_v4();
        let first = make_card(&board, col_id);
        let second = make_card(&board, col_id);
        let second_id = second.id;
        m.load_from_snapshot(Snapshot {
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
        m.load_from_snapshot(Snapshot {
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
}
