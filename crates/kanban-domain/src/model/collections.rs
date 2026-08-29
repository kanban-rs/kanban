use super::*;

impl Model {
    pub fn columns(&self) -> &[Column] {
        self.columns.loaded_or_empty()
    }

    pub fn columns_state(&self) -> &LoadState<Vec<Column>> {
        &self.columns
    }

    pub fn sprints(&self) -> &[Sprint] {
        self.sprints.loaded_or_empty()
    }

    pub fn sprints_state(&self) -> &LoadState<Vec<Sprint>> {
        &self.sprints
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
        m.load_from_snapshot(Snapshot::default());
        assert!(m.columns_state().is_loaded());
        assert!(m.columns_state().loaded().unwrap().is_empty());
        assert!(m.columns().is_empty());
    }

    #[test]
    fn test_columns_state_is_loaded_after_load_from_snapshot() {
        let mut m = Model::default();
        let board = Board::new("B", None::<String>);
        let col = Column::new(board.id, "Col", 0);
        let col_id = col.id;
        m.load_from_snapshot(Snapshot {
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
        m.load_from_snapshot(Snapshot {
            boards: vec![board],
            sprints: vec![sprint],
            ..Default::default()
        });
        assert!(m.sprints_state().is_loaded());
        let loaded = m.sprints_state().loaded().unwrap();
        assert!(loaded.iter().any(|s| s.id == sprint_id));
    }
}
