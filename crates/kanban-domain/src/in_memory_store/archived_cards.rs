use uuid::Uuid;

use super::InMemoryStore;
use crate::{ArchivedCard, KanbanResult};

impl InMemoryStore {
    pub(super) fn get_archived_card_impl(
        &self,
        card_id: Uuid,
    ) -> KanbanResult<Option<ArchivedCard>> {
        let state = self.read_state()?;
        Ok(state.archived_cards.get(&card_id).cloned())
    }

    pub(super) fn list_archived_cards_impl(&self) -> KanbanResult<Vec<ArchivedCard>> {
        let state = self.read_state()?;
        let mut acs: Vec<ArchivedCard> = state.archived_cards.values().cloned().collect();
        acs.sort_by(|a, b| a.metadata.archived_at.cmp(&b.metadata.archived_at));
        Ok(acs)
    }

    pub(super) fn insert_archived_card_impl(&self, ac: ArchivedCard) -> KanbanResult<()> {
        use crate::archival::ArchivedEntity;
        let mut state = self.write_state()?;
        let key = ac.entity_id();
        state.archived_cards.insert(key, ac);
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
            .filter(|ac| ac.board_id == board_id)
            .cloned()
            .collect();
        acs.sort_by(|a, b| a.metadata.archived_at.cmp(&b.metadata.archived_at));
        Ok(acs)
    }

    pub(super) fn list_archived_cards_by_columns_impl(
        &self,
        column_ids: &[Uuid],
    ) -> KanbanResult<Vec<ArchivedCard>> {
        let state = self.read_state()?;
        let mut acs: Vec<ArchivedCard> = state
            .archived_cards
            .values()
            .filter(|ac| column_ids.contains(&ac.original_column_id))
            .cloned()
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
        for ac in state.archived_cards.values_mut() {
            if ac.card.sprint_id == Some(sprint_id) {
                ac.card.sprint_id = None;
                ac.card.updated_at = timestamp;
            }
        }
        Ok(())
    }

    pub(super) fn delete_archived_card_impl(&self, card_id: Uuid) -> KanbanResult<()> {
        let mut state = self.write_state()?;
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
        let ac = ArchivedCard::new(card, uuid::Uuid::nil(), col.id, 0);
        store.insert_archived_card(ac).unwrap();

        let fetched = store.get_archived_card(card_id).unwrap().unwrap();
        assert_eq!(fetched.card.id, card_id);
    }

    #[test]
    fn test_list_archived_cards() {
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let col = make_column(board.id, "C", 0);
        let card1 = make_card(&mut board, col.id, "C1", 0);
        let card2 = make_card(&mut board, col.id, "C2", 1);
        store
            .insert_archived_card(ArchivedCard::new(card1, uuid::Uuid::nil(), col.id, 0))
            .unwrap();
        store
            .insert_archived_card(ArchivedCard::new(card2, uuid::Uuid::nil(), col.id, 1))
            .unwrap();

        assert_eq!(store.list_archived_cards().unwrap().len(), 2);
    }

    #[test]
    fn test_list_archived_cards_by_columns_filters_correctly() {
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let col1 = make_column(board.id, "C1", 0);
        let col2 = make_column(board.id, "C2", 1);
        let card1 = make_card(&mut board, col1.id, "Card1", 0);
        let card2 = make_card(&mut board, col2.id, "Card2", 0);
        store
            .insert_archived_card(ArchivedCard::new(card1, uuid::Uuid::nil(), col1.id, 0))
            .unwrap();
        store
            .insert_archived_card(ArchivedCard::new(card2, uuid::Uuid::nil(), col2.id, 0))
            .unwrap();

        let result = store.list_archived_cards_by_columns(&[col1.id]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].original_column_id, col1.id);
    }

    #[test]
    fn test_list_archived_cards_by_columns_empty_ids_returns_empty() {
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let col = make_column(board.id, "C", 0);
        let card = make_card(&mut board, col.id, "Card", 0);
        store
            .insert_archived_card(ArchivedCard::new(card, uuid::Uuid::nil(), col.id, 0))
            .unwrap();

        let result = store.list_archived_cards_by_columns(&[]).unwrap();
        assert!(result.is_empty());
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
        let ac = ArchivedCard::new(card, uuid::Uuid::nil(), col.id, 0);
        store.insert_archived_card(ac).unwrap();

        let ts = chrono::Utc::now() + chrono::Duration::seconds(10);
        store
            .clear_sprint_from_archived_cards(sprint_id, ts)
            .unwrap();

        let ac = store.get_archived_card(card_id).unwrap().unwrap();
        assert!(ac.card.sprint_id.is_none());
        assert!(ac.card.updated_at > before);
        assert_eq!(ac.card.updated_at, ts);
    }

    #[test]
    fn test_delete_archived_card() {
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let col = make_column(board.id, "C", 0);
        let card = make_card(&mut board, col.id, "Card", 0);
        let card_id = card.id;
        store
            .insert_archived_card(ArchivedCard::new(card, uuid::Uuid::nil(), col.id, 0))
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
            .insert_archived_card(ArchivedCard::new(a1, board_a.id, col_a.id, 0))
            .unwrap();
        store
            .insert_archived_card(ArchivedCard::new(a2, board_a.id, col_a.id, 1))
            .unwrap();
        store
            .insert_archived_card(ArchivedCard::new(b1, board_b.id, col_b.id, 0))
            .unwrap();

        let only_a = store.list_archived_cards_by_board(board_a.id).unwrap();
        let ids: Vec<Uuid> = only_a.iter().map(|ac| ac.card.id).collect();
        assert_eq!(only_a.len(), 2, "only board A's archived cards");
        assert!(ids.contains(&a1_id) && ids.contains(&a2_id));
        assert!(only_a.iter().all(|ac| ac.board_id == board_a.id));
    }

    #[test]
    fn test_list_archived_cards_by_board_default_filters_by_board_id() {
        // The board query must equal filtering the full list by the board_id
        // field — the documented D6 functional default that non-overriding
        // backends inherit; the in-memory override must honour that contract.
        let store = InMemoryStore::new();
        let mut board_a = make_board("A");
        let col_a = make_column(board_a.id, "CA", 0);
        let card_a = make_card(&mut board_a, col_a.id, "A1", 0);
        let mut board_b = make_board("B");
        let col_b = make_column(board_b.id, "CB", 0);
        let card_b = make_card(&mut board_b, col_b.id, "B1", 0);
        store
            .insert_archived_card(ArchivedCard::new(card_a, board_a.id, col_a.id, 0))
            .unwrap();
        store
            .insert_archived_card(ArchivedCard::new(card_b, board_b.id, col_b.id, 0))
            .unwrap();

        let via_query = store.list_archived_cards_by_board(board_a.id).unwrap();
        let via_full_filter: Vec<ArchivedCard> = store
            .list_archived_cards()
            .unwrap()
            .into_iter()
            .filter(|ac| ac.board_id == board_a.id)
            .collect();
        assert_eq!(via_query, via_full_filter);
        assert!(!via_query.is_empty());
    }

    #[test]
    fn test_list_archived_cards_by_board_includes_card_with_deleted_column() {
        // Scope is by the board_id field, tolerating a dangling
        // original_column_id (the column was deleted after archival). The
        // record must still be returned — the load-bearing D2 behavior change.
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let dangling_col = Uuid::new_v4(); // never inserted as a column
        let card = make_card(&mut board, dangling_col, "Orphan", 0);
        let card_id = card.id;
        store
            .insert_archived_card(ArchivedCard::new(card, board.id, dangling_col, 0))
            .unwrap();

        let found = store.list_archived_cards_by_board(board.id).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].card.id, card_id);
    }
}
