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
        // Reference-marker model: the board head stays in `boards` while archived;
        // an `archived_boards` marker hides it from the LIVE list (parity with the
        // SQLite `NOT EXISTS (board_archival …)` filter). `get_board` stays
        // unfiltered so archived boards remain fetchable by id.
        let mut boards: Vec<Board> = state
            .boards
            .values()
            .filter(|b| !state.archived_boards.contains_key(&b.id))
            .cloned()
            .collect();
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
        // Parity with SQLite (`AND NOT EXISTS board_archival`): the bare `delete_board`
        // removes a LIVE board head only. An archived board's head stays put behind its
        // marker; permanent-delete of an archived board is owned by `delete_archived_board`
        // (marker + row). Guarding here prevents a bare delete from orphaning an archived
        // board's still-present subtree and marker.
        if state.archived_boards.contains_key(&id) {
            return Ok(());
        }
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
        // Many boards sharing one position (the tie that arises from the
        // len()-based position assignment colliding after a delete, or from
        // imported/legacy data). Insert in reverse chronological order so that
        // neither insertion order nor HashMap iteration order can incidentally
        // satisfy the assertion: with N=16 the odds of the old position-only
        // sort passing by luck are 1/16! (~5e-14).
        let store = InMemoryStore::new();
        let n = 16;
        for k in (0..n).rev() {
            let mut b = make_board(&format!("b{k:02}"));
            b.position = 0;
            b.created_at = Utc.timestamp_opt(1_000 + k as i64, 0).unwrap();
            store.upsert_board(b).unwrap();
        }

        let names: Vec<String> = store
            .list_boards()
            .unwrap()
            .iter()
            .map(|b| b.name.clone())
            .collect();
        let expected: Vec<String> = (0..n).map(|k| format!("b{k:02}")).collect();

        assert_eq!(
            names, expected,
            "equal-position boards must come back fully ordered by created_at"
        );
    }
}
