use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateBoardRequest {
    #[schemars(description = "Name of the board")]
    pub name: String,
    #[schemars(description = "Optional card prefix (e.g., 'KAN' for KAN-1, KAN-2, etc.)")]
    pub card_prefix: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetBoardRequest {
    #[schemars(description = "UUID or name of the board to retrieve")]
    pub board: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateBoardRequest {
    #[schemars(description = "UUID or name of the board to update")]
    pub board: String,
    #[schemars(description = "New name (optional)")]
    pub name: Option<String>,
    #[schemars(description = "New description (optional)")]
    pub description: Option<String>,
    #[schemars(description = "New sprint prefix (optional)")]
    pub sprint_prefix: Option<String>,
    #[schemars(description = "New card prefix (optional)")]
    pub card_prefix: Option<String>,
    #[schemars(
        description = "Default sort field for the board's task list. Valid: points, priority, created_at, updated_at, due_date, status, position, default. 'default' orders by card number. Date fields and points place None values last in ascending order."
    )]
    pub task_sort_field: Option<String>,
    #[schemars(description = "Default sort direction. Valid: asc, desc")]
    pub task_sort_order: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteBoardRequest {
    #[schemars(description = "UUID or name of the board to delete")]
    pub board: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_board_request_accepts_task_sort_field_and_order() {
        let json = serde_json::json!({
            "board": "B",
            "task_sort_field": "due-date",
            "task_sort_order": "desc",
        });
        let req: UpdateBoardRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.task_sort_field.as_deref(), Some("due-date"));
        assert_eq!(req.task_sort_order.as_deref(), Some("desc"));
    }
}
