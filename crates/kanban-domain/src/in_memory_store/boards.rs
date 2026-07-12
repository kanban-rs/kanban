use uuid::Uuid;

use super::ordering::sort_by_position;
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
        sort_by_position(&mut boards);
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

    #[test]
    fn test_list_boards_orders_equal_position_by_created_at() {
        use chrono::{TimeZone, Utc};
        let store = InMemoryStore::new();
        let mut first = make_board("First");
        first.position = 0;
        first.created_at = Utc.timestamp_opt(1_000, 0).unwrap();
        let mut second = make_board("Second");
        second.position = 0;
        second.created_at = Utc.timestamp_opt(2_000, 0).unwrap();

        store.upsert_board(second).unwrap();
        store.upsert_board(first).unwrap();

        let boards = store.list_boards().unwrap();

        assert_eq!(
            boards[0].name, "First",
            "boards with equal position must order deterministically by created_at"
        );
        assert_eq!(boards[1].name, "Second");
    }
}
