use super::*;

impl Model {
    pub fn columns_state(&self) -> &LoadState<Vec<Column>> {
        &self.columns
    }

    pub fn sprints_state(&self) -> &LoadState<Vec<Sprint>> {
        &self.sprints
    }

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
        match self.columns.as_ref() {
            LoadState::Loaded(columns) => match columns.iter().find(|c| c.id == id) {
                Some(column) => LoadState::Loaded(column),
                None => LoadState::Missing,
            },
            LoadState::NotLoaded => LoadState::NotLoaded,
            LoadState::Missing => LoadState::Missing,
            LoadState::Failed(e) => LoadState::Failed(e),
        }
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
        match self.sprints.as_ref() {
            LoadState::Loaded(sprints) => match sprints.iter().find(|s| s.id == id) {
                Some(sprint) => LoadState::Loaded(sprint),
                None => LoadState::Missing,
            },
            LoadState::NotLoaded => LoadState::NotLoaded,
            LoadState::Missing => LoadState::Missing,
            LoadState::Failed(e) => LoadState::Failed(e),
        }
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
    fn test_columns_state_is_not_loaded_before_load_from_snapshot() {
        let m = Model::default();
        assert!(m.columns_state().is_not_loaded());
    }

    #[test]
    fn test_columns_state_is_loaded_and_empty_after_an_empty_snapshot() {
        let mut m = Model::default();
        let _ = m.load_from_snapshot(Snapshot::default());
        assert!(m.columns_state().is_loaded());
        assert!(m.columns_state().loaded().unwrap().is_empty());
        assert!(m.columns_state().loaded_or_empty().is_empty());
    }

    #[test]
    fn test_columns_state_is_loaded_after_load_from_snapshot() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col = Column::new(board.id, "Col", 0);
        let col_id = col.id;
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![board],
            columns: vec![col],
            ..Default::default()
        });
        let loaded = m.columns_state().loaded().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, col_id);
    }

    #[test]
    fn test_sprints_state_is_not_loaded_before_load_from_snapshot() {
        let m = Model::default();
        assert!(m.sprints_state().is_not_loaded());
    }

    #[test]
    fn test_sprints_state_is_loaded_after_load_from_snapshot() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let sprint = Sprint::new(board.id, 1, None, None::<String>);
        let sprint_id = sprint.id;
        let _ = m.load_from_snapshot(Snapshot {
            boards: vec![board],
            sprints: vec![sprint],
            ..Default::default()
        });
        assert!(m.sprints_state().is_loaded());
        let loaded = m.sprints_state().loaded().unwrap();
        assert!(loaded.iter().any(|s| s.id == sprint_id));
    }
}
