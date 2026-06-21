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
        acs.sort_by(|a, b| a.archived_at.cmp(&b.archived_at));
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
        acs.sort_by(|a, b| a.archived_at.cmp(&b.archived_at));
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
