use uuid::Uuid;

use super::InMemoryStore;
use crate::{ArchivedCard, KanbanResult};

impl InMemoryStore {
    // F3b (KAN-884): archived cards are PURE MARKERS — `state.archived_cards`
    // stores `ArchivedCard { entity_id, metadata, context }` with no embedded
    // card. The card itself is the single source of truth, living in
    // `state.cards`; callers that need card fields fetch it by `entity_id`.
    pub(super) fn get_archived_card_impl(
        &self,
        card_id: Uuid,
    ) -> KanbanResult<Option<ArchivedCard>> {
        let state = self.read_state()?;
        Ok(state.archived_cards.get(&card_id).copied())
    }

    pub(super) fn list_archived_cards_impl(&self) -> KanbanResult<Vec<ArchivedCard>> {
        let state = self.read_state()?;
        let mut acs: Vec<ArchivedCard> = state.archived_cards.values().copied().collect();
        acs.sort_by(|a, b| a.metadata.archived_at.cmp(&b.metadata.archived_at));
        Ok(acs)
    }

    pub(super) fn insert_archived_card_impl(&self, ac: ArchivedCard) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        // Reference model: the card stays live in `cards`; this only records the
        // marker. Re-import paths (e.g. a DeleteCard inverse) upsert the live
        // card themselves before recording the marker.
        state.archived_cards.insert(ac.entity_id, ac);
        Ok(())
    }

    pub(super) fn list_archived_cards_by_board_impl(
        &self,
        board_id: Uuid,
    ) -> KanbanResult<Vec<ArchivedCard>> {
        let state = self.read_state()?;
        let mut acs: Vec<ArchivedCard> = state
            .archived_cards
            .values()
            .filter(|ac| ac.context.board_id == board_id)
            .copied()
            .collect();
        acs.sort_by(|a, b| a.metadata.archived_at.cmp(&b.metadata.archived_at));
        Ok(acs)
    }

    pub(super) fn clear_sprint_from_archived_cards_impl(
        &self,
        sprint_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        // Edit the LIVE card — an archived card is an ordinary editable card.
        let ids: Vec<Uuid> = state.archived_cards.keys().copied().collect();
        for id in ids {
            if let Some(card) = state.cards.get_mut(&id) {
                if card.sprint_id == Some(sprint_id) {
                    card.sprint_id = None;
                    card.updated_at = timestamp;
                }
            }
        }
        Ok(())
    }

    pub(super) fn delete_archived_card_impl(&self, card_id: Uuid) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        // Permanent delete = remove the marker AND the card row (matches SqliteStore).
        if let Some(card) = state.cards.remove(&card_id) {
            state.remove_card_from_column_index(card_id, card.column_id);
        }
        state.archived_cards.remove(&card_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_store::DataStore;
    use crate::in_memory_store::test_support::{make_board, make_card, make_column};

    #[test]
    fn test_insert_and_get_archived_card() {
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let col = make_column(board.id, "C", 0);
        let card = make_card(&mut board, col.id, "Card", 0);
        let card_id = card.id;
        let ac = ArchivedCard::new(card_id, uuid::Uuid::nil());
        store.insert_archived_card(ac).unwrap();

        let fetched = store.get_archived_card(card_id).unwrap().unwrap();
        assert_eq!(fetched.entity_id, card_id);
    }

    #[test]
    fn test_list_archived_cards() {
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let col = make_column(board.id, "C", 0);
        let card1 = make_card(&mut board, col.id, "C1", 0);
        let card2 = make_card(&mut board, col.id, "C2", 1);
        store
            .insert_archived_card(ArchivedCard::new(card1.id, uuid::Uuid::nil()))
            .unwrap();
        store
            .insert_archived_card(ArchivedCard::new(card2.id, uuid::Uuid::nil()))
            .unwrap();

        assert_eq!(store.list_archived_cards().unwrap().len(), 2);
    }

    #[test]
    fn test_clear_sprint_from_archived_cards() {
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let col = make_column(board.id, "C", 0);
        let sprint_id = Uuid::new_v4();
        let mut card = make_card(&mut board, col.id, "Card", 0);
        card.sprint_id = Some(sprint_id);
        let card_id = card.id;
        let before = card.updated_at;
        // Reference-marker model: the card stays LIVE; the marker only references it.
        store.upsert_card(card.clone()).unwrap();
        store
            .insert_archived_card(ArchivedCard::new(card_id, uuid::Uuid::nil()))
            .unwrap();

        let ts = chrono::Utc::now() + chrono::Duration::seconds(10);
        store
            .clear_sprint_from_archived_cards(sprint_id, ts)
            .unwrap();

        let live = store.get_card(card_id).unwrap().unwrap();
        assert!(live.sprint_id.is_none());
        assert!(live.updated_at > before);
        assert_eq!(live.updated_at, ts);
    }

    #[test]
    fn test_delete_archived_card() {
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let col = make_column(board.id, "C", 0);
        let card = make_card(&mut board, col.id, "Card", 0);
        let card_id = card.id;
        store
            .insert_archived_card(ArchivedCard::new(card_id, uuid::Uuid::nil()))
            .unwrap();
        store.delete_archived_card(card_id).unwrap();
        assert!(store.get_archived_card(card_id).unwrap().is_none());
    }

    #[test]
    fn test_list_archived_cards_by_board_returns_only_that_board() {
        let store = InMemoryStore::new();
        let mut board_a = make_board("A");
        let col_a = make_column(board_a.id, "CA", 0);
        let a1 = make_card(&mut board_a, col_a.id, "A1", 0);
        let a2 = make_card(&mut board_a, col_a.id, "A2", 1);
        let a1_id = a1.id;
        let a2_id = a2.id;

        let mut board_b = make_board("B");
        let col_b = make_column(board_b.id, "CB", 0);
        let b1 = make_card(&mut board_b, col_b.id, "B1", 0);

        store
            .insert_archived_card(ArchivedCard::new(a1.id, board_a.id))
            .unwrap();
        store
            .insert_archived_card(ArchivedCard::new(a2.id, board_a.id))
            .unwrap();
        store
            .insert_archived_card(ArchivedCard::new(b1.id, board_b.id))
            .unwrap();

        let only_a = store.list_archived_cards_by_board(board_a.id).unwrap();
        let ids: Vec<Uuid> = only_a.iter().map(|ac| ac.entity_id).collect();
        assert_eq!(only_a.len(), 2, "only board A's archived cards");
        assert!(ids.contains(&a1_id) && ids.contains(&a2_id));
        assert!(only_a.iter().all(|ac| ac.context.board_id == board_a.id));
    }

    #[test]
    fn test_list_archived_cards_by_board_override_matches_full_list_filter() {
        // The in-memory override must equal filtering the full list by the
        // board_id field. (This exercises the override via dispatch, not the
        // trait default; it pins that the override honours the same contract
        // the D6 functional default documents.)
        let store = InMemoryStore::new();
        let mut board_a = make_board("A");
        let col_a = make_column(board_a.id, "CA", 0);
        let card_a = make_card(&mut board_a, col_a.id, "A1", 0);
        let mut board_b = make_board("B");
        let col_b = make_column(board_b.id, "CB", 0);
        let card_b = make_card(&mut board_b, col_b.id, "B1", 0);
        store
            .insert_archived_card(ArchivedCard::new(card_a.id, board_a.id))
            .unwrap();
        store
            .insert_archived_card(ArchivedCard::new(card_b.id, board_b.id))
            .unwrap();

        let via_query = store.list_archived_cards_by_board(board_a.id).unwrap();
        let via_full_filter: Vec<ArchivedCard> = store
            .list_archived_cards()
            .unwrap()
            .into_iter()
            .filter(|ac| ac.context.board_id == board_a.id)
            .collect();
        assert_eq!(via_query, via_full_filter);
        assert!(!via_query.is_empty());
    }

    #[test]
    fn test_list_archived_cards_by_board_includes_card_with_deleted_column() {
        // Scope is by the board_id field, tolerating a dangling historical column
        // (the column was deleted after archival). The record must still be
        // returned — the load-bearing D2 behavior change.
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let dangling_col = Uuid::new_v4(); // never inserted as a column
        let card = make_card(&mut board, dangling_col, "Orphan", 0);
        let card_id = card.id;
        store
            .insert_archived_card(ArchivedCard::new(card_id, board.id))
            .unwrap();

        let found = store.list_archived_cards_by_board(board.id).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].entity_id, card_id);
    }
}

#[cfg(test)]
mod f1_reference_tests {
    //! F1 (KAN-870): the in-memory store archives cards BY REFERENCE — the card
    //! stays in the live `cards` collection, a marker records archival, `get_card`
    //! is unfiltered, live lists hide archived, and archived reads reconstruct
    //! from the live card (no embedded/stale copy). Matches SqliteStore.
    use super::*;
    use crate::data_store::DataStore;
    use crate::in_memory_store::test_support::{make_board, make_card, make_column};
    use crate::{Board, Card, Column};

    /// Archive a live card the way the ArchiveCards command does: insert the
    /// marker, then delete_card (which F1 makes a guarded no-op on archived ids).
    fn archive(store: &InMemoryStore, card: &Card, board_id: uuid::Uuid, _col_id: uuid::Uuid) {
        store
            .insert_archived_card(ArchivedCard::new(card.id, board_id))
            .unwrap();
        store.delete_card(card.id).unwrap();
    }

    fn seed() -> (InMemoryStore, Board, Column, Card) {
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let col = make_column(board.id, "Todo", 0);
        let card = make_card(&mut board, col.id, "Card", 0);
        store.upsert_board(board.clone()).unwrap();
        store.upsert_column(col.clone()).unwrap();
        store.upsert_card(card.clone()).unwrap();
        (store, board, col, card)
    }

    #[test]
    fn test_archived_card_stays_live_and_get_card_is_unfiltered() {
        let (store, board, col, card) = seed();
        archive(&store, &card, board.id, col.id);
        assert!(
            store.get_card(card.id).unwrap().is_some(),
            "reference model: get_card returns the archived card unfiltered"
        );
        assert!(
            store.list_all_cards().unwrap().is_empty(),
            "hidden from the live list"
        );
        assert!(store.list_cards_by_column(col.id).unwrap().is_empty());
        assert!(store.get_archived_card(card.id).unwrap().is_some());
    }

    #[test]
    fn test_archived_read_reconstructs_from_live_no_drift() {
        let (store, board, col, card) = seed();
        archive(&store, &card, board.id, col.id);
        let mut edited = store.get_card(card.id).unwrap().unwrap();
        edited.title = "edited".into();
        store.upsert_card(edited).unwrap();
        assert!(
            store.get_archived_card(card.id).unwrap().is_some(),
            "the marker still references the card"
        );
        assert_eq!(
            store.get_card(card.id).unwrap().unwrap().title,
            "edited",
            "archived read reconstructs from the LIVE card (no stale copy)"
        );
    }

    #[test]
    fn test_delete_card_is_guarded_noop_on_archived() {
        let (store, board, col, card) = seed();
        archive(&store, &card, board.id, col.id);
        store.delete_card(card.id).unwrap();
        assert!(
            store.get_card(card.id).unwrap().is_some(),
            "delete_card is a no-op on an archived card"
        );
        assert!(store.get_archived_card(card.id).unwrap().is_some());
    }

    #[test]
    fn test_delete_archived_card_removes_marker_and_card() {
        let (store, board, col, card) = seed();
        archive(&store, &card, board.id, col.id);
        store.delete_archived_card(card.id).unwrap();
        assert!(
            store.get_card(card.id).unwrap().is_none(),
            "permanent delete removes the card row too"
        );
        assert!(store.get_archived_card(card.id).unwrap().is_none());
    }

    #[test]
    fn test_clear_sprint_from_archived_edits_the_live_card() {
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let col = make_column(board.id, "Todo", 0);
        let mut card = make_card(&mut board, col.id, "Card", 0);
        let sprint_id = uuid::Uuid::new_v4();
        card.sprint_id = Some(sprint_id);
        store.upsert_board(board.clone()).unwrap();
        store.upsert_column(col.clone()).unwrap();
        store.upsert_card(card.clone()).unwrap();
        archive(&store, &card, board.id, col.id);
        store
            .clear_sprint_from_archived_cards(sprint_id, chrono::Utc::now())
            .unwrap();
        assert_eq!(
            store.get_card(card.id).unwrap().unwrap().sprint_id,
            None,
            "clearing the sprint edits the LIVE card"
        );
    }
}
