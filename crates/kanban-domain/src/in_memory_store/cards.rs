use uuid::Uuid;

use super::ordering::sort_by_position;
use super::InMemoryStore;
use crate::{Card, KanbanResult};

impl InMemoryStore {
    pub(super) fn get_card_impl(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        let state = self.read_state()?;
        Ok(state.cards.get(&id).cloned())
    }

    // F1 (KAN-870): live list/count reads exclude archived cards (which now stay
    // in `cards` behind a marker). `get_card` stays unfiltered.
    pub(super) fn list_all_cards_impl(&self) -> KanbanResult<Vec<Card>> {
        let state = self.read_state()?;
        let mut cards: Vec<Card> = state
            .cards
            .values()
            .filter(|c| !state.is_card_archived(&c.id))
            .cloned()
            .collect();
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
                    .filter(|id| !state.is_card_archived(id))
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
            .filter(|c| c.sprint_id == Some(sprint_id) && !state.is_card_archived(&c.id))
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
            .map(|ids| ids.iter().filter(|id| !state.is_card_archived(id)).count())
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
            .map(|ids| {
                ids.iter()
                    .filter(|id| !exclude.contains(id) && !state.is_card_archived(id))
                    .count()
            })
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
        // F1 (KAN-870): guarded — an archived card is removed only via
        // `delete_archived_card` (marker + row). This lets the ArchiveCards
        // command's insert-marker-then-delete-card leave the card live (matching
        // SqliteStore's `NOT EXISTS(archived_cards)` delete guard).
        if state.is_card_archived(&id) {
            return Ok(());
        }
        if let Some(card) = state.cards.remove(&id) {
            state.remove_card_from_column_index(id, card.column_id);
        }
        Ok(())
    }

    pub(super) fn delete_cards_by_columns_impl(&self, column_ids: &[Uuid]) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        // Reference-marker model: this deletes only LIVE cards in the columns.
        // Archived-but-live cards are owned by `delete_archived_card` (marker +
        // row); deleting their row here would strip them from the cascade's
        // archived-card inverse (which captures the still-live card just before it
        // runs), breaking delete↔undo reversibility for archived cards.
        let archived: std::collections::HashSet<Uuid> =
            state.archived_cards.keys().copied().collect();
        state
            .cards
            .retain(|id, c| !column_ids.contains(&c.column_id) || archived.contains(id));
        for col_id in column_ids {
            if let Some(ids) = state.cards_by_column.get_mut(col_id) {
                ids.retain(|id| archived.contains(id));
                if ids.is_empty() {
                    state.cards_by_column.remove(col_id);
                }
            }
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
        // See boards.rs: many equal-position cards inserted in reverse so the
        // old position-only sort cannot pass by luck (1/16!).
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let col = make_column(board.id, "C", 0);
        let n = 16;
        for k in (0..n).rev() {
            let mut c = make_card(&mut board, col.id, &format!("c{k:02}"), 0);
            c.created_at = Utc.timestamp_opt(1_000 + k as i64, 0).unwrap();
            store.upsert_card(c).unwrap();
        }

        let titles: Vec<String> = store
            .list_cards_by_column(col.id)
            .unwrap()
            .iter()
            .map(|c| c.title.clone())
            .collect();
        let expected: Vec<String> = (0..n).map(|k| format!("c{k:02}")).collect();

        assert_eq!(
            titles, expected,
            "equal-position cards must come back fully ordered by created_at"
        );
    }
}
