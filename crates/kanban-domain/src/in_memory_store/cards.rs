use uuid::Uuid;

use super::ordering::sort_by_position;
use super::InMemoryStore;
use crate::{Card, KanbanResult};

impl InMemoryStore {
    pub(super) fn get_card_impl(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        let state = self.read_state()?;
        Ok(state.cards.get(&id).cloned())
    }

    pub(super) fn list_all_cards_impl(&self) -> KanbanResult<Vec<Card>> {
        let state = self.read_state()?;
        let mut cards: Vec<Card> = state.cards.values().cloned().collect();
        sort_by_position(&mut cards);
        Ok(cards)
    }

    pub(super) fn list_cards_by_column_impl(&self, column_id: Uuid) -> KanbanResult<Vec<Card>> {
        let state = self.read_state()?;
        let mut cards: Vec<Card> = state
            .cards_by_column
            .get(&column_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| state.cards.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default();
        sort_by_position(&mut cards);
        Ok(cards)
    }

    pub(super) fn list_cards_by_sprint_impl(&self, sprint_id: Uuid) -> KanbanResult<Vec<Card>> {
        let state = self.read_state()?;
        let mut cards: Vec<Card> = state
            .cards
            .values()
            .filter(|c| c.sprint_id == Some(sprint_id))
            .cloned()
            .collect();
        sort_by_position(&mut cards);
        Ok(cards)
    }

    pub(super) fn count_cards_in_column_impl(&self, column_id: Uuid) -> KanbanResult<usize> {
        let state = self.read_state()?;
        Ok(state
            .cards_by_column
            .get(&column_id)
            .map(|s| s.len())
            .unwrap_or(0))
    }

    pub(super) fn count_cards_in_column_excluding_impl(
        &self,
        column_id: Uuid,
        exclude: &[Uuid],
    ) -> KanbanResult<usize> {
        let state = self.read_state()?;
        let count = state
            .cards_by_column
            .get(&column_id)
            .map(|ids| ids.iter().filter(|id| !exclude.contains(id)).count())
            .unwrap_or(0);
        Ok(count)
    }

    pub(super) fn upsert_card_impl(&self, card: Card) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        let old_column_id = state.cards.get(&card.id).map(|c| c.column_id);
        if let Some(old) = old_column_id {
            if old != card.column_id {
                state.remove_card_from_column_index(card.id, old);
            }
        }
        state.add_card_to_column_index(card.id, card.column_id);
        state.cards.insert(card.id, card);
        Ok(())
    }

    pub(super) fn delete_card_impl(&self, id: Uuid) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        if let Some(card) = state.cards.remove(&id) {
            state.remove_card_from_column_index(id, card.column_id);
        }
        Ok(())
    }

    pub(super) fn delete_cards_by_columns_impl(&self, column_ids: &[Uuid]) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        state
            .cards
            .retain(|_, c| !column_ids.contains(&c.column_id));
        for col_id in column_ids {
            state.cards_by_column.remove(col_id);
        }
        Ok(())
    }

    pub(super) fn clear_sprint_from_cards_impl(
        &self,
        sprint_id: Uuid,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        for card in state.cards.values_mut() {
            if card.sprint_id == Some(sprint_id) {
                card.sprint_id = None;
                card.updated_at = timestamp;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_store::DataStore;
    use crate::in_memory_store::test_support::{make_board, make_card, make_column};

    #[test]
    fn test_list_cards_by_column_orders_equal_position_by_created_at() {
        use chrono::{TimeZone, Utc};
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let col = make_column(board.id, "C", 0);

        let mut first = make_card(&mut board, col.id, "First", 0);
        first.created_at = Utc.timestamp_opt(1_000, 0).unwrap();
        let mut second = make_card(&mut board, col.id, "Second", 0);
        second.created_at = Utc.timestamp_opt(2_000, 0).unwrap();

        store.upsert_card(second).unwrap();
        store.upsert_card(first).unwrap();

        let cards = store.list_cards_by_column(col.id).unwrap();

        assert_eq!(
            cards[0].title, "First",
            "cards with equal position must order deterministically by created_at"
        );
        assert_eq!(cards[1].title, "Second");
    }
}
