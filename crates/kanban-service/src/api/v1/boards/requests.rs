use super::super::{Patch, SortFieldDto, SortOrderDto, TaskListViewDto};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request body for `POST /v1/boards` (and `PUT /v1/boards/:id` create arm).
///
/// Carries every client-settable CREATE field plus an optional client-supplied
/// `id` for idempotent PUT-create. The service mints the id when absent and
/// funnels the content through `NewBoard` + `Board::create`; server-managed
/// fields (counters, `position`, `active_sprint_id`) are never on the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBoardRequest {
    /// Client-supplied id (idempotent PUT-create); read by the service tier.
    #[serde(default)]
    pub id: Option<Uuid>,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sprint_prefix: Option<String>,
    #[serde(default)]
    pub card_prefix: Option<String>,
    #[serde(default)]
    pub task_sort_field: Option<SortFieldDto>,
    #[serde(default)]
    pub task_sort_order: Option<SortOrderDto>,
    #[serde(default)]
    pub sprint_duration_days: Option<u32>,
    #[serde(default)]
    pub task_list_view: Option<TaskListViewDto>,
    #[serde(default)]
    pub completion_column_id: Option<Uuid>,
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
    pub task_sort_field: Option<SortFieldDto>,
    #[serde(default)]
    pub task_sort_order: Option<SortOrderDto>,
    #[serde(default, skip_serializing_if = "Patch::is_no_change")]
    pub sprint_duration_days: Patch<u32>,
    #[serde(default)]
    pub task_list_view: Option<TaskListViewDto>,
    #[serde(default, skip_serializing_if = "Patch::is_no_change")]
    pub completion_column_id: Patch<Uuid>,
}

/// Request body for `PUT /v1/boards/:id` — a true full replace per
/// [RFC 9110 §9.3.4](https://www.rfc-editor.org/info/rfc9110/): the body is the
/// complete client-editable state. Nullable fields are cleared when omitted;
/// the non-nullable fields (`name`, `task_sort_field`, `task_sort_order`,
/// `task_list_view`) are **required** — omitting one is a 400, since a partial
/// body is a PATCH, not a PUT. Server-managed fields are excluded as in
/// [`UpdateBoardRequest`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceBoardRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub sprint_prefix: Option<String>,
    #[serde(default)]
    pub card_prefix: Option<String>,
    pub task_sort_field: SortFieldDto,
    pub task_sort_order: SortOrderDto,
    #[serde(default)]
    pub sprint_duration_days: Option<u32>,
    pub task_list_view: TaskListViewDto,
    #[serde(default)]
    pub completion_column_id: Option<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_board_request_serde_round_trip_includes_new_fields() {
        let id = Uuid::new_v4();
        let col = Uuid::new_v4();
        let req = CreateBoardRequest {
            id: Some(id),
            name: "Roadmap".to_string(),
            description: Some("Q3 planning".to_string()),
            sprint_prefix: Some("SPR".to_string()),
            card_prefix: Some("KAN".to_string()),
            task_sort_field: Some(SortFieldDto::Priority),
            task_sort_order: Some(SortOrderDto::Descending),
            sprint_duration_days: Some(14),
            task_list_view: Some(TaskListViewDto::GroupedByColumn),
            completion_column_id: Some(col),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: CreateBoardRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, Some(id));
        assert_eq!(back.name, req.name);
        assert_eq!(back.description, req.description);
        assert_eq!(back.task_sort_field, req.task_sort_field);
        assert_eq!(back.task_sort_order, req.task_sort_order);
        assert_eq!(back.sprint_duration_days, Some(14));
        assert_eq!(back.task_list_view, Some(TaskListViewDto::GroupedByColumn));
        assert_eq!(back.completion_column_id, Some(col));
    }

    #[test]
    fn test_create_board_request_minimal_omits_optionals() {
        let json = r#"{"name":"Minimal"}"#;
        let back: CreateBoardRequest = serde_json::from_str(json).unwrap();
        assert_eq!(back.id, None);
        assert_eq!(back.name, "Minimal");
        assert_eq!(back.description, None);
        assert_eq!(back.task_sort_field, None);
        assert_eq!(back.sprint_duration_days, None);
        assert_eq!(back.task_list_view, None);
        assert_eq!(back.completion_column_id, None);
    }

    #[test]
    fn test_update_board_request_merge_patch_round_trip() {
        let req = UpdateBoardRequest {
            name: Some("Renamed".to_string()),
            description: Patch::Set("new desc".to_string()),
            sprint_prefix: Patch::Clear,
            card_prefix: Patch::NoChange,
            task_sort_field: Some(SortFieldDto::CreatedAt),
            task_sort_order: None,
            sprint_duration_days: Patch::Set(14),
            task_list_view: Some(TaskListViewDto::GroupedByColumn),
            completion_column_id: Patch::Set(Uuid::nil()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: UpdateBoardRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.description, Patch::Set("new desc".to_string()));
        assert_eq!(back.sprint_prefix, Patch::Clear);
        assert_eq!(back.task_sort_field, Some(SortFieldDto::CreatedAt));
        assert_eq!(back.task_list_view, Some(TaskListViewDto::GroupedByColumn));
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
    fn test_update_board_request_omits_no_change_patch_fields_on_serialize() {
        // Guards the Patch footgun: every Patch field must carry
        // skip_serializing_if, so a default (all-NoChange) request omits them
        // rather than emitting null (= clear).
        let v = serde_json::to_value(UpdateBoardRequest::default()).unwrap();
        for field in [
            "description",
            "sprint_prefix",
            "card_prefix",
            "sprint_duration_days",
            "completion_column_id",
        ] {
            assert!(
                v.get(field).is_none(),
                "NoChange patch field `{field}` must be omitted, got: {v}"
            );
        }
    }

    #[test]
    fn test_replace_board_request_requires_non_nullable_fields() {
        // A partial body is a PATCH, not a PUT: missing the required non-nullable
        // fields must fail to deserialize (→ 400).
        let result: Result<ReplaceBoardRequest, _> = serde_json::from_str(r#"{"name":"Fresh"}"#);
        assert!(result.is_err());
    }
}
