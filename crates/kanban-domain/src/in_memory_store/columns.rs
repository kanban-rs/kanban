use uuid::Uuid;

use super::InMemoryStore;
use crate::{Column, KanbanResult};

impl InMemoryStore {
    pub(super) fn get_column_impl(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        let state = self.read_state()?;
        Ok(state.columns.get(&id).cloned())
    }

    pub(super) fn list_columns_by_board_impl(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        let state = self.read_state()?;
        let mut cols: Vec<Column> = state
            .columns
            .values()
            .filter(|c| c.board_id == board_id)
            .cloned()
            .collect();
        cols.sort_by_key(|c| c.position);
        Ok(cols)
    }

    pub(super) fn list_all_columns_impl(&self) -> KanbanResult<Vec<Column>> {
        let state = self.read_state()?;
        let mut cols: Vec<Column> = state.columns.values().cloned().collect();
        cols.sort_by_key(|c| c.position);
        Ok(cols)
    }

    pub(super) fn upsert_column_impl(&self, column: Column) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        state.columns.insert(column.id, column);
        Ok(())
    }

    pub(super) fn delete_column_impl(&self, id: Uuid) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        state.columns.remove(&id);
        Ok(())
    }

    pub(super) fn delete_columns_by_board_impl(&self, board_id: Uuid) -> KanbanResult<()> {
        let mut state = self.write_state()?;
        state.columns.retain(|_, c| c.board_id != board_id);
        Ok(())
    }
}
