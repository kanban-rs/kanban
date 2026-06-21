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
