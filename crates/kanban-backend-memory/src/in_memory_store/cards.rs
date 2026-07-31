use uuid::Uuid;

use super::ordering::sort_by_position;
use super::state::StoreState;
use super::InMemoryStore;
use kanban_domain::{ArchivedFilter, Card, KanbanResult};

/// 3-state archived predicate shared by the in-memory `*_filtered` reads. Hoisted
/// to a free function so both `list`/`count` overrides use one spec and neither
/// captures `state` in a closure across the borrow.
fn keep_by_filter(state: &StoreState, id: &Uuid, archived: ArchivedFilter) -> bool {
    match archived {
        ArchivedFilter::LiveOnly => !state.is_card_archived(id),
        ArchivedFilter::ArchivedOnly => state.is_card_archived(id),
        ArchivedFilter::Include => true,
    }
}

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

    // F1c (KAN-926): 3-state archived-aware column reads. Overrides the loud-floor
    // trait default so in-memory (and thus JSON) honour ArchivedOnly/Include;
    // LiveOnly stays identical to `list_cards_by_column`/`count_cards_in_column`.
    pub(super) fn list_cards_by_column_filtered_impl(
        &self,
        column_id: Uuid,
        archived: ArchivedFilter,
    ) -> KanbanResult<Vec<Card>> {
        let state = self.read_state()?;
        let mut cards: Vec<Card> = state
            .cards_by_column
            .get(&column_id)
            .map(|ids| {
                ids.iter()
                    .filter(|id| keep_by_filter(&state, id, archived))
                    .filter_map(|id| state.cards.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default();
        sort_by_position(&mut cards);
        Ok(cards)
    }

    pub(super) fn count_cards_in_column_filtered_impl(
        &self,
        column_id: Uuid,
        archived: ArchivedFilter,
    ) -> KanbanResult<usize> {
        let state = self.read_state()?;
        Ok(state
            .cards_by_column
            .get(&column_id)
            .map(|ids| {
                ids.iter()
                    .filter(|id| keep_by_filter(&state, id, archived))
                    .count()
            })
            .unwrap_or(0))
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
        // Live-only, matching SQLite (`AND NOT EXISTS archived_cards`): the archived
        // subset is owned by `clear_sprint_from_archived_cards`. Mirrors the same
        // HashSet snapshot pattern used by `delete_cards_by_columns_impl` (line 117)
        // to avoid a borrow conflict between `values_mut()` and `archived_cards`.
        let archived: std::collections::HashSet<Uuid> =
            state.archived_cards.keys().copied().collect();
        for card in state.cards.values_mut() {
            if card.sprint_id == Some(sprint_id) && !archived.contains(&card.id) {
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
    use crate::in_memory_store::test_support::{make_board, make_card, make_column};
    use kanban_domain::data_store::DataStore;
    use kanban_domain::{ArchivedCard, ArchivedFilter};

    /// Seed one column with 2 live + 2 archived cards. Returns the store, the
    /// column id, the 2 live ids, and the 2 archived ids.
    fn seed_two_live_two_archived() -> (InMemoryStore, Uuid, Vec<Uuid>, Vec<Uuid>) {
        let store = InMemoryStore::new();
        let mut board = make_board("B");
        let col = make_column(board.id, "Todo", 0);
        store.upsert_board(board.clone()).unwrap();
        store.upsert_column(col.clone()).unwrap();

        let mut live = Vec::new();
        let mut archived = Vec::new();
        for i in 0..2 {
            let c = make_card(&mut board, col.id, &format!("live{i}"), i);
            live.push(c.id);
            store.upsert_card(c).unwrap();
        }
        for i in 0..2 {
            let c = make_card(&mut board, col.id, &format!("arch{i}"), 2 + i);
            archived.push(c.id);
            store.upsert_card(c.clone()).unwrap();
            // Archive the ArchiveCards way: marker + guarded delete_card no-op.
            store
                .insert_archived_card(ArchivedCard::new(c.id, board.id))
                .unwrap();
            store.delete_card(c.id).unwrap();
        }
        (store, col.id, live, archived)
    }

    #[test]
    fn test_inmem_count_filtered_liveonly_is_2() {
        let (store, col_id, _live, _archived) = seed_two_live_two_archived();
        assert_eq!(
            store
                .count_cards_in_column_filtered(col_id, ArchivedFilter::LiveOnly)
                .unwrap(),
            2,
            "LiveOnly counts only the 2 live cards"
        );
    }

    #[test]
    fn test_inmem_count_filtered_archivedonly_is_2() {
        let (store, col_id, _live, _archived) = seed_two_live_two_archived();
        assert_eq!(
            store
                .count_cards_in_column_filtered(col_id, ArchivedFilter::ArchivedOnly)
                .unwrap(),
            2,
            "ArchivedOnly counts only the 2 archived cards"
        );
    }

    #[test]
    fn test_inmem_count_filtered_include_is_4() {
        let (store, col_id, _live, _archived) = seed_two_live_two_archived();
        assert_eq!(
            store
                .count_cards_in_column_filtered(col_id, ArchivedFilter::Include)
                .unwrap(),
            4,
            "Include counts all 4 cards"
        );
    }

    #[test]
    fn test_inmem_list_filtered_liveonly_returns_live_ids() {
        let (store, col_id, mut live, _archived) = seed_two_live_two_archived();
        let mut got: Vec<Uuid> = store
            .list_cards_by_column_filtered(col_id, ArchivedFilter::LiveOnly)
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        got.sort();
        live.sort();
        assert_eq!(got, live, "LiveOnly lists only the 2 live cards");
    }

    #[test]
    fn test_inmem_list_filtered_archivedonly_returns_archived_ids() {
        let (store, col_id, _live, mut archived) = seed_two_live_two_archived();
        let mut got: Vec<Uuid> = store
            .list_cards_by_column_filtered(col_id, ArchivedFilter::ArchivedOnly)
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        got.sort();
        archived.sort();
        assert_eq!(
            got, archived,
            "ArchivedOnly lists only the 2 archived cards"
        );
    }

    #[test]
    fn test_inmem_list_filtered_include_returns_all_ids() {
        let (store, col_id, live, archived) = seed_two_live_two_archived();
        let mut want: Vec<Uuid> = live.into_iter().chain(archived).collect();
        let mut got: Vec<Uuid> = store
            .list_cards_by_column_filtered(col_id, ArchivedFilter::Include)
            .unwrap()
            .into_iter()
            .map(|c| c.id)
            .collect();
        got.sort();
        want.sort();
        assert_eq!(got, want, "Include lists all 4 cards");
    }

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
