use uuid::Uuid;

use super::InMemoryStore;
use crate::archival::ArchivedEntity;
use crate::{ArchivedBoard, KanbanResult};

impl InMemoryStore {
    pub(super) fn get_archived_board_impl(
        &self,
        board_id: Uuid,
    ) -> KanbanResult<Option<ArchivedBoard>> {
        let state = self.read_state()?;
        Ok(state.archived_boards.get(&board_id).cloned())
    }

    pub(super) fn list_archived_boards_impl(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        let state = self.read_state()?;
        let mut boards: Vec<ArchivedBoard> = state.archived_boards.values().cloned().collect();
        // Ascending by archived_at, matching `list_archived_cards` and the
        // snapshot order (presentation order is the UI layer's concern).
        boards.sort_by(|a, b| a.metadata.archived_at.cmp(&b.metadata.archived_at));
        Ok(boards)
    }

    pub(super) fn insert_archived_board_impl(&self, ab: ArchivedBoard) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        state.archived_boards.insert(ab.entity_id(), ab);
        Ok(())
    }

    pub(super) fn delete_archived_board_impl(&self, board_id: Uuid) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        state.archived_boards.remove(&board_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_store::DataStore;
    use crate::in_memory_store::test_support::make_board;
    use crate::Archived;
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_insert_get_list_delete_archived_board_in_memory() {
        let store = InMemoryStore::new();
        let board = make_board("B");
        let id = board.id;
        store.upsert_board(board).unwrap();

        store.insert_archived_board(Archived::now(id)).unwrap();
        assert_eq!(store.get_archived_board(id).unwrap().unwrap().entity_id, id);
        assert_eq!(store.list_archived_boards().unwrap().len(), 1);

        store.delete_archived_board(id).unwrap();
        assert!(store.get_archived_board(id).unwrap().is_none());
        assert!(store.list_archived_boards().unwrap().is_empty());
    }

    #[test]
    fn test_delete_archived_board_is_idempotent_on_missing() {
        let store = InMemoryStore::new();
        store.delete_archived_board(Uuid::new_v4()).unwrap();
    }

    #[test]
    fn test_list_archived_boards_sorted_by_archived_at_ascending() {
        // Ascending by archived_at, consistent with list_archived_cards and the
        // snapshot order. Inserted newest-first to prove the sort, not insertion.
        let store = InMemoryStore::new();
        let newer = make_board("newer");
        let older = make_board("older");
        let newer_id = newer.id;
        let older_id = older.id;
        store.upsert_board(newer).unwrap();
        store.upsert_board(older).unwrap();
        store
            .insert_archived_board(Archived::at(newer_id, Utc.timestamp_opt(2_000, 0).unwrap()))
            .unwrap();
        store
            .insert_archived_board(Archived::at(older_id, Utc.timestamp_opt(1_000, 0).unwrap()))
            .unwrap();

        let listed = store.list_archived_boards().unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].entity_id, older_id);
        assert_eq!(listed[1].entity_id, newer_id);
    }
}
