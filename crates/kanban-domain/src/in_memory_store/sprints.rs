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
