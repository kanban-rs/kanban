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
        let mut state = self.write_state()?;
        state.archived_cards.insert(ac.card.id, ac);
        Ok(())
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
        let ac = ArchivedCard::new(card, col.id, 0);
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
            .insert_archived_card(ArchivedCard::new(card1, col.id, 0))
            .unwrap();
        store
            .insert_archived_card(ArchivedCard::new(card2, col.id, 1))
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
            .insert_archived_card(ArchivedCard::new(card1, col1.id, 0))
            .unwrap();
        store
            .insert_archived_card(ArchivedCard::new(card2, col2.id, 0))
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
            .insert_archived_card(ArchivedCard::new(card, col.id, 0))
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
        let ac = ArchivedCard::new(card, col.id, 0);
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
            .insert_archived_card(ArchivedCard::new(card, col.id, 0))
            .unwrap();
        store.delete_archived_card(card_id).unwrap();
        assert!(store.get_archived_card(card_id).unwrap().is_none());
    }
}
