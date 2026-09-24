use super::*;

impl Model {
    pub fn board_columns_state(&self, board_id: Uuid) -> LoadState<&[Column]> {
        scoped_state(&self.columns_by_board, board_id)
    }

    pub fn board_sprints_state(&self, board_id: Uuid) -> LoadState<&[Sprint]> {
        scoped_state(&self.sprints_by_board, board_id)
    }

    pub fn column_by_id_state(&self, id: Uuid) -> LoadState<&Column> {
        if let Some(state) = self.columns_by_id.get(&id) {
            return state.as_ref();
        }
        for state in self.columns_by_board.values() {
            if let LoadState::Loaded(columns) = state {
                if let Some(column) = columns.iter().find(|c| c.id == id) {
                    return LoadState::Loaded(column);
                }
            }
        }
        LoadState::NotLoaded
    }

    pub fn sprint_by_id_state(&self, id: Uuid) -> LoadState<&Sprint> {
        if let Some(state) = self.sprints_by_id.get(&id) {
            return state.as_ref();
        }
        for state in self.sprints_by_board.values() {
            if let LoadState::Loaded(sprints) = state {
                if let Some(sprint) = sprints.iter().find(|s| s.id == id) {
                    return LoadState::Loaded(sprint);
                }
            }
        }
        LoadState::NotLoaded
    }

    pub fn column_id_status(&self, id: Uuid) -> LoadState<&Column> {
        self.columns_by_id
            .get(&id)
            .map(|s| s.as_ref())
            .unwrap_or(LoadState::NotLoaded)
    }

    pub fn sprint_id_status(&self, id: Uuid) -> LoadState<&Sprint> {
        self.sprints_by_id
            .get(&id)
            .map(|s| s.as_ref())
            .unwrap_or(LoadState::NotLoaded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Column, Snapshot, Sprint};

    #[test]
    fn test_board_columns_state_is_not_loaded_before_load_from_snapshot() {
        let m = Model::default();
        assert!(m.board_columns_state(Uuid::new_v4()).is_not_loaded());
    }

    #[test]
    fn test_board_columns_state_is_loaded_and_empty_after_an_empty_snapshot() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let board_id = board.id;
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![board],
            ..Default::default()
        });
        let state = m.board_columns_state(board_id);
        assert!(state.is_loaded());
        assert!(state.loaded().unwrap().is_empty());
    }

    #[test]
    fn test_board_columns_state_is_loaded_after_load_from_snapshot() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col = Column::new(board.id, "Col", 0);
        let col_id = col.id;
        let board_id = board.id;
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![board],
            columns: vec![col],
            ..Default::default()
        });
        let state = m.board_columns_state(board_id);
        let loaded = state.loaded().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, col_id);
    }

    #[test]
    fn test_board_sprints_state_is_not_loaded_before_load_from_snapshot() {
        let m = Model::default();
        assert!(m.board_sprints_state(Uuid::new_v4()).is_not_loaded());
    }

    #[test]
    fn test_board_sprints_state_is_loaded_after_load_from_snapshot() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let board_id = board.id;
        let sprint = Sprint::new(board.id, 1, None, None::<String>);
        let sprint_id = sprint.id;
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![board],
            sprints: vec![sprint],
            ..Default::default()
        });
        let state = m.board_sprints_state(board_id);
        assert!(state.is_loaded());
        let loaded = state.loaded().unwrap();
        assert!(loaded.iter().any(|s| s.id == sprint_id));
    }
}
