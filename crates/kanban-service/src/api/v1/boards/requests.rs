use super::super::Patch;
use kanban_domain::{BoardUpdate, FieldUpdate, SortField, SortOrder, TaskListView};
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

impl CreateBoardRequest {
    /// Split into the `create_board(name, card_prefix)` args plus a follow-up
    /// [`BoardUpdate`] carrying the remaining create fields. The handler runs
    /// create-then-update. Present optionals become `Set`; **absent stay
    /// `NoChange`** (create never clears, unlike PUT).
    pub fn into_parts(self) -> (String, Option<String>, BoardUpdate) {
        let CreateBoardRequest {
            name,
            description,
            sprint_prefix,
            card_prefix,
            task_sort_field,
            task_sort_order,
        } = self;
        let follow_up = BoardUpdate {
            name: None,
            description: opt_set(description),
            sprint_prefix: opt_set(sprint_prefix),
            // card_prefix is consumed by create_board, not the follow-up:
            card_prefix: FieldUpdate::NoChange,
            task_sort_field,
            task_sort_order,
            sprint_duration_days: FieldUpdate::NoChange,
            task_list_view: None,
            completion_column_id: FieldUpdate::NoChange,
            active_sprint_id: FieldUpdate::NoChange,
            position: None,
        };
        (name, card_prefix, follow_up)
    }
}

/// `Option → FieldUpdate` for the **create** path: present = `Set`, absent =
/// `NoChange`. (Distinct from `FieldUpdate::from(Option)`, which clears on
/// `None` — correct only for PUT.)
fn opt_set<T>(value: Option<T>) -> FieldUpdate<T> {
    match value {
        Some(v) => FieldUpdate::Set(v),
        None => FieldUpdate::NoChange,
    }
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
    pub task_sort_field: SortField,
    pub task_sort_order: SortOrder,
    #[serde(default)]
    pub sprint_duration_days: Option<u32>,
    pub task_list_view: TaskListView,
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
        // True full replace: nullable fields use `Option → FieldUpdate` (Some→Set,
        // None→Clear); the required non-nullable fields are always `Set`.
        BoardUpdate {
            name: Some(name),
            description: description.into(),
            sprint_prefix: sprint_prefix.into(),
            card_prefix: card_prefix.into(),
            task_sort_field: Some(task_sort_field),
            task_sort_order: Some(task_sort_order),
            sprint_duration_days: sprint_duration_days.into(),
            task_list_view: Some(task_list_view),
            completion_column_id: completion_column_id.into(),
            active_sprint_id: FieldUpdate::NoChange,
            position: None,
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
    fn test_replace_board_request_requires_non_nullable_fields() {
        // A partial body is a PATCH, not a PUT: missing the required non-nullable
        // fields must fail to deserialize (→ 400).
        let result: Result<ReplaceBoardRequest, _> = serde_json::from_str(r#"{"name":"Fresh"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_replace_board_request_is_true_full_replace() {
        // Full representation, nullable fields omitted → cleared; required fields set.
        let json = r#"{
            "name":"Fresh",
            "task_sort_field":"Priority",
            "task_sort_order":"Ascending",
            "task_list_view":"Flat"
        }"#;
        let req: ReplaceBoardRequest = serde_json::from_str(json).unwrap();
        let update: BoardUpdate = req.into();
        assert_eq!(update.name, Some("Fresh".to_string()));
        // Required non-nullable fields are always set:
        assert_eq!(update.task_sort_field, Some(SortField::Priority));
        assert_eq!(update.task_sort_order, Some(SortOrder::Ascending));
        assert_eq!(update.task_list_view, Some(TaskListView::Flat));
        // Omitted nullable fields are cleared (wholesale replace):
        assert_eq!(update.description, FieldUpdate::Clear);
        assert_eq!(update.sprint_prefix, FieldUpdate::Clear);
        assert_eq!(update.completion_column_id, FieldUpdate::Clear);
        // Server-managed untouched:
        assert_eq!(update.active_sprint_id, FieldUpdate::NoChange);
        assert_eq!(update.position, None);
    }

    #[test]
    fn test_create_board_request_into_parts_splits_args_and_follow_up() {
        let req = CreateBoardRequest {
            name: "Roadmap".to_string(),
            description: Some("desc".to_string()),
            sprint_prefix: None,
            card_prefix: Some("KAN".to_string()),
            task_sort_field: Some(SortField::Priority),
            task_sort_order: None,
        };
        let (name, card_prefix, follow_up) = req.into_parts();
        assert_eq!(name, "Roadmap");
        assert_eq!(card_prefix, Some("KAN".to_string()));
        assert_eq!(follow_up.name, None); // name is the create arg, not in the update
        assert_eq!(follow_up.description, FieldUpdate::Set("desc".to_string())); // present → Set
        assert_eq!(follow_up.sprint_prefix, FieldUpdate::NoChange); // absent → NoChange, not Clear
        assert_eq!(follow_up.card_prefix, FieldUpdate::NoChange); // consumed by create arg
        assert_eq!(follow_up.task_sort_field, Some(SortField::Priority));
        assert_eq!(follow_up.active_sprint_id, FieldUpdate::NoChange);
    }
}
