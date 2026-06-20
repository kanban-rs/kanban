use super::patch::Patch;
use chrono::{DateTime, Utc};
use kanban_domain::{Board, BoardId, BoardUpdate, FieldUpdate, SortField, SortOrder, TaskListView};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request body for `POST /v1/boards`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBoardRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sprint_prefix: Option<String>,
    #[serde(default)]
    pub card_prefix: Option<String>,
    #[serde(default)]
    pub task_sort_field: Option<SortField>,
    #[serde(default)]
    pub task_sort_order: Option<SortOrder>,
}

/// Request body for `PATCH /v1/boards/:id` — JSON Merge Patch (RFC 7386):
/// absent field = no change, `null` = clear, value = set (see [`Patch`]).
///
/// Server-managed fields (`active_sprint_id`, board `position`) are
/// intentionally excluded from the wire contract: they are computed by the
/// server (sprint activation, board ordering) and never accepted from a client.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateBoardRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Patch::is_no_change")]
    pub description: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_no_change")]
    pub sprint_prefix: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_no_change")]
    pub card_prefix: Patch<String>,
    #[serde(default)]
    pub task_sort_field: Option<SortField>,
    #[serde(default)]
    pub task_sort_order: Option<SortOrder>,
    #[serde(default, skip_serializing_if = "Patch::is_no_change")]
    pub sprint_duration_days: Patch<u32>,
    #[serde(default)]
    pub task_list_view: Option<TaskListView>,
    #[serde(default, skip_serializing_if = "Patch::is_no_change")]
    pub completion_column_id: Patch<Uuid>,
}

impl From<UpdateBoardRequest> for BoardUpdate {
    fn from(req: UpdateBoardRequest) -> Self {
        // Exhaustive destructure (no `..`): a new request field is a compile error.
        let UpdateBoardRequest {
            name,
            description,
            sprint_prefix,
            card_prefix,
            task_sort_field,
            task_sort_order,
            sprint_duration_days,
            task_list_view,
            completion_column_id,
        } = req;
        // Exhaustive construct (no `..Default::default()`): a new BoardUpdate field
        // is a compile error, forcing a deliberate exposed/excluded decision.
        BoardUpdate {
            name,
            description: description.into(),
            sprint_prefix: sprint_prefix.into(),
            card_prefix: card_prefix.into(),
            task_sort_field,
            task_sort_order,
            sprint_duration_days: sprint_duration_days.into(),
            task_list_view,
            completion_column_id: completion_column_id.into(),
            // Server-managed — never accepted from a PATCH body:
            active_sprint_id: FieldUpdate::NoChange,
            position: None,
        }
    }
}

/// Request body for `PUT /v1/boards/:id` — full replace of the client-editable
/// fields. PUT semantics: an omitted nullable field is **cleared** (wholesale
/// replace), unlike PATCH where omitted means no-change. Server-managed fields
/// are excluded as in [`UpdateBoardRequest`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplaceBoardRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sprint_prefix: Option<String>,
    #[serde(default)]
    pub card_prefix: Option<String>,
    #[serde(default)]
    pub task_sort_field: Option<SortField>,
    #[serde(default)]
    pub task_sort_order: Option<SortOrder>,
    #[serde(default)]
    pub sprint_duration_days: Option<u32>,
    #[serde(default)]
    pub task_list_view: Option<TaskListView>,
    #[serde(default)]
    pub completion_column_id: Option<Uuid>,
}

impl From<ReplaceBoardRequest> for BoardUpdate {
    fn from(req: ReplaceBoardRequest) -> Self {
        let ReplaceBoardRequest {
            name,
            description,
            sprint_prefix,
            card_prefix,
            task_sort_field,
            task_sort_order,
            sprint_duration_days,
            task_list_view,
            completion_column_id,
        } = req;
        // PUT replace: nullable fields use `Option → FieldUpdate` (Some→Set, None→Clear).
        // Non-nullable enum fields (sort/view) carry no Clear state, so an omitted
        // value is left unchanged; clients should send them on a full replace.
        BoardUpdate {
            name: Some(name),
            description: description.into(),
            sprint_prefix: sprint_prefix.into(),
            card_prefix: card_prefix.into(),
            task_sort_field,
            task_sort_order,
            sprint_duration_days: sprint_duration_days.into(),
            task_list_view,
            completion_column_id: completion_column_id.into(),
            active_sprint_id: FieldUpdate::NoChange,
            position: None,
        }
    }
}

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
    fn test_create_board_request_serde_round_trip() {
        let req = CreateBoardRequest {
            name: "Roadmap".to_string(),
            description: Some("Q3 planning".to_string()),
            sprint_prefix: Some("SPR".to_string()),
            card_prefix: Some("KAN".to_string()),
            task_sort_field: Some(SortField::Priority),
            task_sort_order: Some(SortOrder::Descending),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: CreateBoardRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, req.name);
        assert_eq!(back.description, req.description);
        assert_eq!(back.sprint_prefix, req.sprint_prefix);
        assert_eq!(back.card_prefix, req.card_prefix);
        assert_eq!(back.task_sort_field, req.task_sort_field);
        assert_eq!(back.task_sort_order, req.task_sort_order);
    }

    #[test]
    fn test_create_board_request_minimal_omits_optionals() {
        let json = r#"{"name":"Minimal"}"#;
        let back: CreateBoardRequest = serde_json::from_str(json).unwrap();
        assert_eq!(back.name, "Minimal");
        assert_eq!(back.description, None);
        assert_eq!(back.task_sort_field, None);
    }

    #[test]
    fn test_update_board_request_merge_patch_round_trip() {
        let req = UpdateBoardRequest {
            name: Some("Renamed".to_string()),
            description: Patch::Set("new desc".to_string()),
            sprint_prefix: Patch::Clear,
            card_prefix: Patch::NoChange,
            task_sort_field: Some(SortField::CreatedAt),
            task_sort_order: None,
            sprint_duration_days: Patch::Set(14),
            task_list_view: Some(TaskListView::GroupedByColumn),
            completion_column_id: Patch::Set(Uuid::nil()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: UpdateBoardRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, Some("Renamed".to_string()));
        assert_eq!(back.description, Patch::Set("new desc".to_string()));
        assert_eq!(back.sprint_prefix, Patch::Clear);
        assert_eq!(back.card_prefix, Patch::NoChange);
        assert_eq!(back.sprint_duration_days, Patch::Set(14));
        assert_eq!(back.task_list_view, Some(TaskListView::GroupedByColumn));
        assert_eq!(back.completion_column_id, Patch::Set(Uuid::nil()));
    }

    #[test]
    fn test_update_board_request_absent_is_no_change_null_is_clear() {
        let back: UpdateBoardRequest = serde_json::from_str(r#"{"description":null}"#).unwrap();
        assert_eq!(back.name, None);
        assert_eq!(back.description, Patch::Clear); // explicit null → clear
        assert_eq!(back.sprint_prefix, Patch::NoChange); // absent → no change
        assert_eq!(back.completion_column_id, Patch::NoChange);
    }

    #[test]
    fn test_update_board_request_into_board_update_excludes_server_fields() {
        let req = UpdateBoardRequest {
            name: Some("N".to_string()),
            description: Patch::Clear,
            sprint_prefix: Patch::Set("S".to_string()),
            card_prefix: Patch::NoChange,
            task_sort_field: Some(SortField::Priority),
            task_sort_order: Some(SortOrder::Ascending),
            sprint_duration_days: Patch::Set(7),
            task_list_view: Some(TaskListView::Flat),
            completion_column_id: Patch::NoChange,
        };
        let update: BoardUpdate = req.into();
        assert_eq!(update.name, Some("N".to_string()));
        assert_eq!(update.description, FieldUpdate::Clear);
        assert_eq!(update.sprint_prefix, FieldUpdate::Set("S".to_string()));
        assert_eq!(update.card_prefix, FieldUpdate::NoChange);
        assert_eq!(update.sprint_duration_days, FieldUpdate::Set(7));
        // Server-managed fields are forced to their no-op values:
        assert_eq!(update.active_sprint_id, FieldUpdate::NoChange);
        assert_eq!(update.position, None);
    }

    #[test]
    fn test_replace_board_request_clears_omitted_nullable_fields() {
        // PUT with only the required name: omitted nullable fields clear (wholesale replace).
        let req: ReplaceBoardRequest = serde_json::from_str(r#"{"name":"Fresh"}"#).unwrap();
        let update: BoardUpdate = req.into();
        assert_eq!(update.name, Some("Fresh".to_string()));
        assert_eq!(update.description, FieldUpdate::Clear);
        assert_eq!(update.sprint_prefix, FieldUpdate::Clear);
        assert_eq!(update.completion_column_id, FieldUpdate::Clear);
        assert_eq!(update.active_sprint_id, FieldUpdate::NoChange);
        assert_eq!(update.position, None);
    }

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
