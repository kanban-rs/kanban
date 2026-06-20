use chrono::{DateTime, Utc};
use kanban_domain::{Board, BoardId, SortField, SortOrder, TaskListView};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response body for board reads. Omits internal allocation state
/// (`card_counter`, `next_sprint_number`, `sprint_counters`, `sprint_names`,
/// `sprint_name_used_count`); `active_sprint_id`/`position` are read-only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardResponse {
    pub id: BoardId,
    pub name: String,
    pub description: Option<String>,
    pub sprint_prefix: Option<String>,
    pub card_prefix: Option<String>,
    pub task_sort_field: SortField,
    pub task_sort_order: SortOrder,
    pub sprint_duration_days: Option<u32>,
    pub task_list_view: TaskListView,
    pub active_sprint_id: Option<Uuid>,
    pub completion_column_id: Option<Uuid>,
    pub position: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Board> for BoardResponse {
    fn from(b: Board) -> Self {
        Self {
            id: b.id,
            name: b.name,
            description: b.description,
            sprint_prefix: b.sprint_prefix,
            card_prefix: b.card_prefix,
            task_sort_field: b.task_sort_field,
            task_sort_order: b.task_sort_order,
            sprint_duration_days: b.sprint_duration_days,
            task_list_view: b.task_list_view,
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
    fn test_board_response_omits_internal_allocation_state() {
        let board = Board::new("Test", Some("KAN"));
        let resp = BoardResponse::from(board.clone());
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
    }
}
