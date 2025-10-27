use crate::{Board, Card, Column, Sprint, SprintStatus};
use kanban_core::Editable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardSettingsDto {
    pub branch_prefix: Option<String>,
    pub sprint_duration_days: Option<u32>,
    pub sprint_prefix: Option<String>,
    pub sprint_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardMetadataDto {
    pub priority: String,
    pub status: String,
    pub points: Option<u8>,
    pub due_date: Option<String>,
}

impl Editable<Board> for BoardSettingsDto {
    fn from_entity(board: &Board) -> Self {
        Self {
            branch_prefix: board.branch_prefix.clone(),
            sprint_duration_days: board.sprint_duration_days,
            sprint_prefix: board.sprint_prefix.clone(),
            sprint_names: board.sprint_names.clone(),
        }
    }

    fn apply_to(self, board: &mut Board) {
        board.branch_prefix = self.branch_prefix;
        board.sprint_duration_days = self.sprint_duration_days;
        board.sprint_prefix = self.sprint_prefix;
        board.sprint_names = self.sprint_names;
        board.updated_at = chrono::Utc::now();
    }
}

impl Editable<Card> for CardMetadataDto {
    fn from_entity(card: &Card) -> Self {
        Self {
            priority: format!("{:?}", card.priority),
            status: format!("{:?}", card.status),
            points: card.points,
            due_date: card.due_date.map(|dt| dt.to_rfc3339()),
        }
    }

    fn apply_to(self, card: &mut Card) {
        if let Ok(priority) =
            serde_json::from_value::<crate::CardPriority>(serde_json::Value::String(self.priority))
        {
            card.priority = priority;
        }

        if let Ok(status) =
            serde_json::from_value::<crate::CardStatus>(serde_json::Value::String(self.status))
        {
            card.status = status;
        }

        if let Some(points) = self.points {
            card.points = Some(points);
        }

        if let Some(due_date_str) = self.due_date {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&due_date_str) {
                card.due_date = Some(dt.with_timezone(&chrono::Utc));
            }
        }

        card.updated_at = chrono::Utc::now();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMetadataDto {
    pub name: String,
    pub position: i32,
    pub wip_limit: Option<i32>,
}

impl Editable<Column> for ColumnMetadataDto {
    fn from_entity(column: &Column) -> Self {
        Self {
            name: column.name.clone(),
            position: column.position,
            wip_limit: column.wip_limit,
        }
    }

    fn apply_to(self, column: &mut Column) {
        column.name = self.name;
        column.position = self.position;
        column.wip_limit = self.wip_limit;
        column.updated_at = chrono::Utc::now();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SprintMetadataDto {
    pub status: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub prefix_override: Option<String>,
    pub name_index: Option<usize>,
}

impl Editable<Sprint> for SprintMetadataDto {
    fn from_entity(sprint: &Sprint) -> Self {
        Self {
            status: format!("{:?}", sprint.status),
            start_date: sprint.start_date.map(|dt| dt.to_rfc3339()),
            end_date: sprint.end_date.map(|dt| dt.to_rfc3339()),
            prefix_override: sprint.prefix_override.clone(),
            name_index: sprint.name_index,
        }
    }

    fn apply_to(self, sprint: &mut Sprint) {
        if let Ok(status) =
            serde_json::from_value::<SprintStatus>(serde_json::Value::String(self.status))
        {
            sprint.status = status;
        }

        if let Some(start_date_str) = self.start_date {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&start_date_str) {
                sprint.start_date = Some(dt.with_timezone(&chrono::Utc));
            }
        }

        if let Some(end_date_str) = self.end_date {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&end_date_str) {
                sprint.end_date = Some(dt.with_timezone(&chrono::Utc));
            }
        }

        sprint.prefix_override = self.prefix_override;
        sprint.name_index = self.name_index;
        sprint.updated_at = chrono::Utc::now();
    }
}
