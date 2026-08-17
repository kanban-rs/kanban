use super::*;

impl Model {
    /// The full unified live+archived collection. Only for callers that
    /// genuinely need id resolution regardless of archival status — see
    /// `live_cards`/`archived_cards` for the common display case.
    pub fn all_cards(&self) -> &[Card] {
        self.cards.as_deref().unwrap_or(&[])
    }

    /// Resolve a card by id from the single unified collection (live AND
    /// archived rows). One index lookup — no live/archived re-join. A card is
    /// a card regardless of whether its head is archived.
    pub fn card_by_id(&self, id: Uuid) -> Option<&Card> {
        let &idx = self.card_index.get(&id)?;
        self.cards.as_ref()?.get(idx)
    }

    /// The archived-card MARKER records (id + archived_at + restore context).
    /// For restore/permanent-delete logic that needs the marker itself, not the
    /// live entity. See `archived_cards()` for the full `Card` entities.
    pub fn archived_card_markers(&self) -> &[ArchivedCard] {
        self.archived_cards.as_deref().unwrap_or(&[])
    }

    /// Ids of the archived cards. Rows themselves live in the unified `all_cards()`
    /// collection; this set records which of them are archived (built from the
    /// markers). The live/archived partition is precomputed on load and served by
    /// [`displayed_cards`](Self::displayed_cards); this set backs that split.
    pub fn archived_card_ids(&self) -> &std::collections::HashSet<Uuid> {
        &self.archived_card_ids
    }

    /// The cards the tasks panel should display, selected by `want_archived`:
    /// the archived subset when a confirm dialog / the archived-cards view is
    /// active, the live subset otherwise. Returns a BORROW of the partition
    /// cached on the last `load_from_snapshot` — no per-frame filter or clone.
    pub fn displayed_cards(&self, want_archived: bool) -> &[Card] {
        if want_archived {
            &self.displayed_cards_archived
        } else {
            &self.displayed_cards_live
        }
    }

    /// The live cards — the common case for anything rendering to the user.
    /// Thin wrapper over the cached live/archived partition.
    pub fn live_cards(&self) -> &[Card] {
        self.displayed_cards(false)
    }

    /// The archived cards, as full `Card` entities (not the marker records —
    /// see `archived_card_markers` for those).
    pub fn archived_cards(&self) -> &[Card] {
        self.displayed_cards(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{ArchivedCard, Board, Card, Snapshot};

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
        let found = m.card_by_id(card_b_id).unwrap();
        assert_eq!(found.id, card_b_id);
    }

    #[test]
    fn test_card_by_id_resolves_live_and_archived_from_one_collection() {
        // After unification `all_cards()` holds live AND archived rows, and
        // `card_by_id` resolves either from the single collection — no
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
        assert_eq!(m.all_cards().len(), 2);

        // The single index resolves both.
        assert_eq!(m.card_by_id(live_id).map(|c| c.id), Some(live_id));
        assert_eq!(m.card_by_id(archived_id).map(|c| c.id), Some(archived_id));

        // The archived-id set records which rows are archived.
        assert!(!m.archived_card_ids().contains(&live_id));
        assert!(m.archived_card_ids().contains(&archived_id));
    }

    #[test]
    fn test_archived_view_filter_shows_archived_card_from_unified_collection() {
        // `archived_card_ids` records the archived subset of the unified `all_cards()`
        // collection (the same set that backs `displayed_cards`). Assert an
        // archived card is reachable by filtering `all_cards()` through that set.
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
            .all_cards()
            .iter()
            .filter(|c| m.archived_card_ids().contains(&c.id))
            .map(|c| c.id)
            .collect();
        assert_eq!(displayed, vec![archived_id]);
    }

    #[test]
    fn test_card_by_id_missing_id_returns_none() {
        let m = Model::default();
        assert!(m.card_by_id(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_displayed_cards_partition_cached_on_load() {
        // Cache-on-load guard: `load_from_snapshot` partitions the unified card
        // collection into live/archived subsets ONCE, and `displayed_cards`
        // returns the cached slice by `want_archived` — no per-frame filter.
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

        let live_ids: Vec<Uuid> = m.displayed_cards(false).iter().map(|c| c.id).collect();
        let archived_ids: Vec<Uuid> = m.displayed_cards(true).iter().map(|c| c.id).collect();
        assert_eq!(live_ids, vec![live_id]);
        assert_eq!(archived_ids, vec![archived_id]);
    }
}
