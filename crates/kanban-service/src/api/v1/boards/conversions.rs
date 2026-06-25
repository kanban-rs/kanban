//! Wire-to-domain conversions for the board request DTOs. Kept separate from the
//! struct definitions in `requests.rs`: the wire shape and the mapping policy
//! (server-managed exclusion, create-then-update, true-replace) change for
//! different reasons. Each conversion destructures and constructs exhaustively
//! (no `..`) so a new field is a compile error.

use super::super::conv::set_or_no_change;
use super::requests::{CreateBoardRequest, ReplaceBoardRequest, UpdateBoardRequest};
use kanban_domain::{BoardUpdate, FieldUpdate, NewBoard};
use uuid::Uuid;

impl From<UpdateBoardRequest> for BoardUpdate {
    fn from(req: UpdateBoardRequest) -> Self {
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
        BoardUpdate {
            name,
            description: description.into(),
            sprint_prefix: sprint_prefix.into(),
            card_prefix: card_prefix.into(),
            task_sort_field: task_sort_field.map(Into::into),
            task_sort_order: task_sort_order.map(Into::into),
            sprint_duration_days: sprint_duration_days.into(),
            task_list_view: task_list_view.map(Into::into),
            completion_column_id: completion_column_id.into(),
            // Server-managed — never accepted from a PATCH body:
            active_sprint_id: FieldUpdate::NoChange,
            position: None,
        }
    }
}

impl CreateBoardRequest {
    /// Split the identity (optional client id) from the content spec. The
    /// service mints the id when `None` and calls `Board::create(spec, id, now)`.
    /// Exhaustive destructure — no `..` — so a new field is a compile error.
    pub fn into_new_board(self) -> (Option<Uuid>, NewBoard) {
        let CreateBoardRequest {
            id,
            name,
            description,
            sprint_prefix,
            card_prefix,
            task_sort_field,
            task_sort_order,
            sprint_duration_days,
            task_list_view,
            completion_column_id,
        } = self;
        let spec = NewBoard {
            name,
            description,
            sprint_prefix,
            card_prefix,
            task_sort_field: task_sort_field.map(Into::into),
            task_sort_order: task_sort_order.map(Into::into),
            sprint_duration_days,
            task_list_view: task_list_view.map(Into::into),
            completion_column_id,
        };
        (id, spec)
    }

    /// Split into the `create_board(name, card_prefix)` args plus a follow-up
    /// [`BoardUpdate`] for the remaining create fields. The handler runs
    /// create-then-update. Present optionals become `Set`; absent stay
    /// `NoChange` (create never clears, unlike PUT).
    #[deprecated(note = "use into_new_board + Board::create; removed in KAN-769 slice D")]
    pub fn into_parts(self) -> (String, Option<String>, BoardUpdate) {
        let CreateBoardRequest {
            id: _,
            name,
            description,
            sprint_prefix,
            card_prefix,
            task_sort_field,
            task_sort_order,
            sprint_duration_days,
            task_list_view,
            completion_column_id,
        } = self;
        let follow_up = BoardUpdate {
            name: None,
            description: set_or_no_change(description),
            sprint_prefix: set_or_no_change(sprint_prefix),
            // card_prefix is consumed by create_board, not the follow-up:
            card_prefix: FieldUpdate::NoChange,
            task_sort_field: task_sort_field.map(Into::into),
            task_sort_order: task_sort_order.map(Into::into),
            sprint_duration_days: set_or_no_change(sprint_duration_days),
            task_list_view: task_list_view.map(Into::into),
            completion_column_id: set_or_no_change(completion_column_id),
            active_sprint_id: FieldUpdate::NoChange,
            position: None,
        };
        (name, card_prefix, follow_up)
    }
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
        // True full replace: nullable fields map Option→FieldUpdate (Some→Set,
        // None→Clear); the required non-nullable fields are always `Set`.
        BoardUpdate {
            name: Some(name),
            description: description.into(),
            sprint_prefix: sprint_prefix.into(),
            card_prefix: card_prefix.into(),
            task_sort_field: Some(task_sort_field.into()),
            task_sort_order: Some(task_sort_order.into()),
            sprint_duration_days: sprint_duration_days.into(),
            task_list_view: Some(task_list_view.into()),
            completion_column_id: completion_column_id.into(),
            active_sprint_id: FieldUpdate::NoChange,
            position: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{Patch, SortFieldDto, SortOrderDto, TaskListViewDto};
    use super::*;
    use kanban_domain::{SortField, SortOrder, TaskListView};

    #[test]
    fn test_update_board_request_into_board_update_maps_enums_and_excludes_server_fields() {
        let req = UpdateBoardRequest {
            name: Some("N".to_string()),
            description: Patch::Clear,
            sprint_prefix: Patch::Set("S".to_string()),
            card_prefix: Patch::NoChange,
            task_sort_field: Some(SortFieldDto::Priority),
            task_sort_order: Some(SortOrderDto::Ascending),
            sprint_duration_days: Patch::Set(7),
            task_list_view: Some(TaskListViewDto::Flat),
            completion_column_id: Patch::NoChange,
        };
        let update: BoardUpdate = req.into();
        assert_eq!(update.name, Some("N".to_string()));
        assert_eq!(update.description, FieldUpdate::Clear);
        assert_eq!(update.sprint_prefix, FieldUpdate::Set("S".to_string()));
        assert_eq!(update.sprint_duration_days, FieldUpdate::Set(7));
        assert_eq!(update.task_sort_field, Some(SortField::Priority));
        assert_eq!(update.task_sort_order, Some(SortOrder::Ascending));
        assert_eq!(update.task_list_view, Some(TaskListView::Flat));
        assert_eq!(update.active_sprint_id, FieldUpdate::NoChange);
        assert_eq!(update.position, None);
    }

    #[test]
    fn test_replace_board_request_is_true_full_replace() {
        let json = r#"{
            "name":"Fresh",
            "task_sort_field":"priority",
            "task_sort_order":"ascending",
            "task_list_view":"flat"
        }"#;
        let req: ReplaceBoardRequest = serde_json::from_str(json).unwrap();
        let update: BoardUpdate = req.into();
        assert_eq!(update.name, Some("Fresh".to_string()));
        assert_eq!(update.task_sort_field, Some(SortField::Priority));
        assert_eq!(update.task_sort_order, Some(SortOrder::Ascending));
        assert_eq!(update.task_list_view, Some(TaskListView::Flat));
        // Omitted nullable fields cleared (wholesale replace):
        assert_eq!(update.description, FieldUpdate::Clear);
        assert_eq!(update.sprint_prefix, FieldUpdate::Clear);
        assert_eq!(update.completion_column_id, FieldUpdate::Clear);
        assert_eq!(update.active_sprint_id, FieldUpdate::NoChange);
        assert_eq!(update.position, None);
    }

    #[test]
    #[allow(deprecated)]
    fn test_create_board_request_into_parts_splits_args_and_follow_up() {
        let req = CreateBoardRequest {
            id: None,
            name: "Roadmap".to_string(),
            description: Some("desc".to_string()),
            sprint_prefix: None,
            card_prefix: Some("KAN".to_string()),
            task_sort_field: Some(SortFieldDto::Priority),
            task_sort_order: None,
            sprint_duration_days: None,
            task_list_view: None,
            completion_column_id: None,
        };
        let (name, card_prefix, follow_up) = req.into_parts();
        assert_eq!(name, "Roadmap");
        assert_eq!(card_prefix, Some("KAN".to_string()));
        assert_eq!(follow_up.name, None);
        assert_eq!(follow_up.description, FieldUpdate::Set("desc".to_string()));
        assert_eq!(follow_up.sprint_prefix, FieldUpdate::NoChange); // absent → NoChange, not Clear
        assert_eq!(follow_up.card_prefix, FieldUpdate::NoChange); // consumed by create arg
        assert_eq!(follow_up.task_sort_field, Some(SortField::Priority));
        assert_eq!(follow_up.active_sprint_id, FieldUpdate::NoChange);
    }

    #[test]
    fn test_create_board_request_into_new_board_maps_all_content_fields() {
        let col = Uuid::new_v4();
        let req = CreateBoardRequest {
            id: None,
            name: "Roadmap".to_string(),
            description: Some("desc".to_string()),
            sprint_prefix: Some("SPR".to_string()),
            card_prefix: Some("KAN".to_string()),
            task_sort_field: Some(SortFieldDto::Priority),
            task_sort_order: Some(SortOrderDto::Descending),
            sprint_duration_days: Some(21),
            task_list_view: Some(TaskListViewDto::GroupedByColumn),
            completion_column_id: Some(col),
        };
        let (_id, spec) = req.into_new_board();
        assert_eq!(spec.name, "Roadmap");
        assert_eq!(spec.description, Some("desc".to_string()));
        assert_eq!(spec.sprint_prefix, Some("SPR".to_string()));
        assert_eq!(spec.card_prefix, Some("KAN".to_string()));
        assert_eq!(spec.task_sort_field, Some(SortField::Priority));
        assert_eq!(spec.task_sort_order, Some(SortOrder::Descending));
        assert_eq!(spec.sprint_duration_days, Some(21));
        assert_eq!(spec.task_list_view, Some(TaskListView::GroupedByColumn));
        assert_eq!(spec.completion_column_id, Some(col));
    }

    #[test]
    fn test_create_board_request_into_new_board_carries_optional_id() {
        let id = Uuid::new_v4();
        let with_id = CreateBoardRequest {
            id: Some(id),
            name: "B".to_string(),
            description: None,
            sprint_prefix: None,
            card_prefix: None,
            task_sort_field: None,
            task_sort_order: None,
            sprint_duration_days: None,
            task_list_view: None,
            completion_column_id: None,
        };
        let (carried, _) = with_id.into_new_board();
        assert_eq!(carried, Some(id));

        let without_id = CreateBoardRequest {
            id: None,
            name: "B".to_string(),
            description: None,
            sprint_prefix: None,
            card_prefix: None,
            task_sort_field: None,
            task_sort_order: None,
            sprint_duration_days: None,
            task_list_view: None,
            completion_column_id: None,
        };
        let (carried, _) = without_id.into_new_board();
        assert_eq!(carried, None);
    }

    #[test]
    fn test_create_board_request_minimal_into_new_board_defaults_optionals() {
        let req: CreateBoardRequest = serde_json::from_str(r#"{"name":"M"}"#).unwrap();
        let (id, spec) = req.into_new_board();
        assert_eq!(id, None);
        assert_eq!(spec.name, "M");
        assert_eq!(spec.description, None);
        assert_eq!(spec.task_sort_field, None);
        assert_eq!(spec.task_sort_order, None);
        assert_eq!(spec.sprint_duration_days, None);
        assert_eq!(spec.task_list_view, None);
        assert_eq!(spec.completion_column_id, None);
    }
}
