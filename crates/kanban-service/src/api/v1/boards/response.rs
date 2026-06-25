use super::super::{SortFieldDto, SortOrderDto, TaskListViewDto};
use chrono::{DateTime, Utc};
use kanban_domain::Board;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response body for board reads. Omits internal allocation state
/// (`card_counter`, `next_sprint_number`, `sprint_counters`, `sprint_names`,
/// `sprint_name_used_count`); `active_sprint_id`/`position` are read-only.
/// Enums use the decoupled wire mirrors (snake_case); ids are plain `Uuid`.
/// `Deserialize` is derived intentionally (test round-trips / client use); the
/// server only serializes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub sprint_prefix: Option<String>,
    pub card_prefix: Option<String>,
    pub task_sort_field: SortFieldDto,
    pub task_sort_order: SortOrderDto,
    pub sprint_duration_days: Option<u32>,
    pub task_list_view: TaskListViewDto,
    pub active_sprint_id: Option<Uuid>,
    pub completion_column_id: Option<Uuid>,
    pub position: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<&Board> for BoardResponse {
    fn from(b: &Board) -> Self {
        Self {
            id: b.id,
            name: b.name.clone(),
            description: b.description.clone(),
            sprint_prefix: b.sprint_prefix.clone(),
            card_prefix: b.card_prefix.clone(),
            task_sort_field: b.task_sort_field.into(),
            task_sort_order: b.task_sort_order.into(),
            sprint_duration_days: b.sprint_duration_days,
            task_list_view: b.task_list_view.into(),
            active_sprint_id: b.active_sprint_id,
            completion_column_id: b.completion_column_id,
            position: b.position,
            created_at: b.created_at,
            updated_at: b.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_board_response_from_ref_omits_internal_state_and_uses_snake_case_enums() {
        let board = Board::new("Test", Some("KAN"));
        let resp = BoardResponse::from(&board);
        assert_eq!(resp.id, board.id);
        assert_eq!(resp.name, "Test");
        let json = serde_json::to_string(&resp).unwrap();
        for hidden in [
            "card_counter",
            "next_sprint_number",
            "sprint_counters",
            "sprint_names",
            "sprint_name_used_count",
        ] {
            assert!(
                !json.contains(hidden),
                "BoardResponse leaked {hidden}: {json}"
            );
        }
        // Decoupled wire enums serialize snake_case (default view is Flat):
        assert!(json.contains("\"task_list_view\":\"flat\""), "json: {json}");
    }
}
