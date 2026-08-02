use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::board::BoardId;
use crate::field_update::FieldUpdate;

pub type ColumnId = Uuid;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Column {
    pub id: ColumnId,
    pub board_id: BoardId,
    pub name: String,
    pub position: i32,
    pub wip_limit: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Column {
    pub fn new(board_id: BoardId, name: impl Into<String>, position: i32) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            board_id,
            name: name.into(),
            position,
            wip_limit: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn set_wip_limit(&mut self, limit: Option<i32>) {
        self.wip_limit = limit;
        self.updated_at = Utc::now();
    }

    pub fn update_position(&mut self, position: i32) {
        self.position = position;
        self.updated_at = Utc::now();
    }

    pub fn update_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
        self.updated_at = Utc::now();
    }

    /// Update column with partial changes
    pub fn update(&mut self, updates: ColumnUpdate) {
        if let Some(name) = updates.name {
            self.name = name;
        }
        if let Some(position) = updates.position {
            self.position = position;
        }
        updates.wip_limit.apply_to(&mut self.wip_limit);
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_new_accepts_str_name_without_to_string() {
        let board_id = uuid::Uuid::new_v4();
        let column = Column::new(board_id, "To Do", 0);
        assert_eq!(column.name, "To Do");
    }

    #[test]
    fn test_column_update_name_accepts_str_without_to_string() {
        let board_id = uuid::Uuid::new_v4();
        let mut column = Column::new(board_id, "To Do", 0);
        column.update_name("In Progress");
        assert_eq!(column.name, "In Progress");
    }
}

/// Partial update struct for Column
///
/// Uses `FieldUpdate<T>` for optional fields to provide clear three-state updates.
/// See [`FieldUpdate`] documentation for usage examples.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ColumnUpdate {
    pub name: Option<String>,
    pub position: Option<i32>,
    pub wip_limit: FieldUpdate<i32>,
}
