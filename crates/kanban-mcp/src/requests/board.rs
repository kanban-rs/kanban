use rmcp::schemars;
use serde::Deserialize;

// KAN-792: the bespoke board-create DTO is gone. The board-create tool now uses
// the shared `kanban_service::api::CreateBoardRequest` (re-exported here so
// existing `crate::requests::board::CreateBoardRequest` import paths still
// resolve), converting via `into_new_board` and calling `create_board_from_spec`.
pub use kanban_service::api::CreateBoardRequest;

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
pub struct ListBoardsRequest {
    #[schemars(
        description = "Archived filter: 'exclude' (default, live only), 'only' (archived only), or 'include' (both live and archived). Archived boards carry an archived_at timestamp."
    )]
    pub archived: Option<String>,
    #[schemars(description = "Page number, 1-based (default: 1)")]
    pub page: Option<u32>,
    #[schemars(description = "Items per page (default: 50)")]
    pub page_size: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteBoardRequest {
    #[schemars(description = "UUID or name of the board to delete")]
    pub board: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ArchiveBoardRequest {
    #[schemars(description = "UUID or name of the board to archive")]
    pub board: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RestoreBoardRequest {
    #[schemars(description = "UUID or name of the archived board to restore")]
    pub board: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteArchivedBoardRequest {
    #[schemars(description = "UUID or name of the archived board to permanently delete")]
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
