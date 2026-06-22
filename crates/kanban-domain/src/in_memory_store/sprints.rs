use uuid::Uuid;

use super::InMemoryStore;
use crate::{KanbanResult, Sprint};

impl InMemoryStore {
    pub(super) fn get_sprint_impl(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        let state = self.read_state()?;
        Ok(state.sprints.get(&id).cloned())
    }

    pub(super) fn list_sprints_by_board_impl(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        let state = self.read_state()?;
        let mut sprints: Vec<Sprint> = state
            .sprints
            .values()
            .filter(|s| s.board_id == board_id)
            .cloned()
            .collect();
        sprints.sort_by_key(|s| s.sprint_number);
        Ok(sprints)
    }

    pub(super) fn list_all_sprints_impl(&self) -> KanbanResult<Vec<Sprint>> {
        let state = self.read_state()?;
        let mut sprints: Vec<Sprint> = state.sprints.values().cloned().collect();
        sprints.sort_by_key(|s| s.sprint_number);
        Ok(sprints)
    }

    pub(super) fn upsert_sprint_impl(&self, sprint: Sprint) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        state.sprints.insert(sprint.id, sprint);
        Ok(())
    }

    pub(super) fn delete_sprint_impl(&self, id: Uuid) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        state.sprints.remove(&id);
        Ok(())
    }

    pub(super) fn delete_sprints_by_board_impl(&self, board_id: Uuid) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        state.sprints.retain(|_, s| s.board_id != board_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_store::DataStore;
    use crate::in_memory_store::test_support::make_board;

    #[test]
    fn test_upsert_and_get_sprint() {
        let store = InMemoryStore::new();
        let board = make_board("B");
        let sprint = Sprint::new(board.id, 1, None, None::<String>);
        let sprint_id = sprint.id;
        store.upsert_sprint(sprint).unwrap();

        let fetched = store.get_sprint(sprint_id).unwrap().unwrap();
        assert_eq!(fetched.id, sprint_id);
        assert_eq!(fetched.sprint_number, 1);
    }

    #[test]
    fn test_list_sprints_by_board() {
        let store = InMemoryStore::new();
        let board1 = make_board("B1");
        let board2 = make_board("B2");
        let s1 = Sprint::new(board1.id, 1, None, None::<String>);
        let s2 = Sprint::new(board1.id, 2, None, None::<String>);
        let s3 = Sprint::new(board2.id, 1, None, None::<String>);
        store.upsert_sprint(s1).unwrap();
        store.upsert_sprint(s2).unwrap();
        store.upsert_sprint(s3).unwrap();

        let sprints = store.list_sprints_by_board(board1.id).unwrap();
        assert_eq!(sprints.len(), 2);
        assert!(sprints.iter().all(|s| s.board_id == board1.id));
    }

    #[test]
    fn test_delete_sprints_by_board() {
        let store = InMemoryStore::new();
        let board1 = make_board("B1");
        let board2 = make_board("B2");
        let s1 = Sprint::new(board1.id, 1, None, None::<String>);
        let s2 = Sprint::new(board2.id, 1, None, None::<String>);
        let s2_id = s2.id;
        store.upsert_sprint(s1).unwrap();
        store.upsert_sprint(s2).unwrap();

        store.delete_sprints_by_board(board1.id).unwrap();

        assert!(store.list_sprints_by_board(board1.id).unwrap().is_empty());
        assert!(store.get_sprint(s2_id).unwrap().is_some());
    }
}
