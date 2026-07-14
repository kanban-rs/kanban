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
        boards.sort_by(|a, b| b.metadata.archived_at.cmp(&a.metadata.archived_at));
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

        store.insert_archived_board(Archived::now(board)).unwrap();
        assert_eq!(store.get_archived_board(id).unwrap().unwrap().entity.id, id);
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
    fn test_list_archived_boards_returns_most_recently_archived_first() {
        let store = InMemoryStore::new();
        let older = make_board("older");
        let newer = make_board("newer");
        store
            .insert_archived_board(Archived::at(older, Utc.timestamp_opt(1_000, 0).unwrap()))
            .unwrap();
        store
            .insert_archived_board(Archived::at(newer, Utc.timestamp_opt(2_000, 0).unwrap()))
            .unwrap();

        let listed = store.list_archived_boards().unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].entity.name, "newer");
        assert_eq!(listed[1].entity.name, "older");
    }
}
