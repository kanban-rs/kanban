use uuid::Uuid;

use super::InMemoryStore;
use crate::{Board, KanbanResult};

impl InMemoryStore {
    pub(super) fn get_board_impl(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        let state = self.read_state()?;
        Ok(state.boards.get(&id).cloned())
    }

    pub(super) fn list_boards_impl(&self) -> KanbanResult<Vec<Board>> {
        let state = self.read_state()?;
        let mut boards: Vec<Board> = state.boards.values().cloned().collect();
        boards.sort_by_key(|b| b.position);
        Ok(boards)
    }

    pub(super) fn upsert_board_impl(&self, board: Board) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        state.boards.insert(board.id, board);
        Ok(())
    }

    pub(super) fn delete_board_impl(&self, id: Uuid) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        state.boards.remove(&id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_store::DataStore;
    use crate::in_memory_store::test_support::make_board;

    #[test]
    fn test_upsert_and_get_board() {
        let store = InMemoryStore::new();
        let board = make_board("Test Board");
        let id = board.id;
        store.upsert_board(board.clone()).unwrap();

        let fetched = store.get_board(id).unwrap().unwrap();
        assert_eq!(fetched.id, id);
        assert_eq!(fetched.name, "Test Board");
    }

    #[test]
    fn test_list_boards_empty() {
        let store = InMemoryStore::new();
        let boards = store.list_boards().unwrap();
        assert!(boards.is_empty());
    }

    #[test]
    fn test_delete_board_removes_it() {
        let store = InMemoryStore::new();
        let board = make_board("To Delete");
        let id = board.id;
        store.upsert_board(board).unwrap();
        store.delete_board(id).unwrap();
        assert!(store.get_board(id).unwrap().is_none());
    }
}
