use uuid::Uuid;

use super::ordering::sort_by_position;
use super::InMemoryStore;
use kanban_domain::{Column, KanbanResult};

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
        sort_by_position(&mut cols);
        Ok(cols)
    }

    pub(super) fn list_all_columns_impl(&self) -> KanbanResult<Vec<Column>> {
        let state = self.read_state()?;
        let mut cols: Vec<Column> = state.columns.values().cloned().collect();
        sort_by_position(&mut cols);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::in_memory_store::test_support::{make_board, make_column};
    use kanban_domain::data_store::DataStore;

    #[test]
    fn test_upsert_and_get_column() {
        let store = InMemoryStore::new();
        let board = make_board("B");
        let col = make_column(board.id, "Col", 0);
        let col_id = col.id;
        store.upsert_column(col.clone()).unwrap();

        let fetched = store.get_column(col_id).unwrap().unwrap();
        assert_eq!(fetched.id, col_id);
        assert_eq!(fetched.name, "Col");
    }

    #[test]
    fn test_list_columns_by_board_filters_correctly() {
        let store = InMemoryStore::new();
        let board1 = make_board("B1");
        let board2 = make_board("B2");
        let col1 = make_column(board1.id, "C1", 0);
        let col2 = make_column(board1.id, "C2", 1);
        let col3 = make_column(board2.id, "C3", 0);
        store.upsert_column(col1).unwrap();
        store.upsert_column(col2).unwrap();
        store.upsert_column(col3).unwrap();

        let cols = store.list_columns_by_board(board1.id).unwrap();
        assert_eq!(cols.len(), 2);
        assert!(cols.iter().all(|c| c.board_id == board1.id));
    }

    #[test]
    fn test_delete_columns_by_board() {
        let store = InMemoryStore::new();
        let board1 = make_board("B1");
        let board2 = make_board("B2");
        let col1 = make_column(board1.id, "C1", 0);
        let col2 = make_column(board2.id, "C2", 0);
        let col2_id = col2.id;
        store.upsert_column(col1).unwrap();
        store.upsert_column(col2).unwrap();

        store.delete_columns_by_board(board1.id).unwrap();

        assert!(store.list_columns_by_board(board1.id).unwrap().is_empty());
        assert!(store.get_column(col2_id).unwrap().is_some());
    }

    #[test]
    fn test_list_columns_by_board_orders_equal_position_by_created_at() {
        use chrono::{TimeZone, Utc};
        // See boards.rs: many equal-position columns inserted in reverse so the
        // old position-only sort cannot pass by luck (1/16!).
        let store = InMemoryStore::new();
        let board = make_board("B");
        let n = 16;
        for k in (0..n).rev() {
            let mut c = make_column(board.id, &format!("c{k:02}"), 0);
            c.created_at = Utc.timestamp_opt(1_000 + k as i64, 0).unwrap();
            store.upsert_column(c).unwrap();
        }

        let names: Vec<String> = store
            .list_columns_by_board(board.id)
            .unwrap()
            .iter()
            .map(|c| c.name.clone())
            .collect();
        let expected: Vec<String> = (0..n).map(|k| format!("c{k:02}")).collect();

        assert_eq!(
            names, expected,
            "equal-position columns must come back fully ordered by created_at"
        );
    }
}
