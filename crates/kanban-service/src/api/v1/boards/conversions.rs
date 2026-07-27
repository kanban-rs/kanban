//! Wire-to-domain conversions for the board request DTOs. Kept separate from the
//! struct definitions in `requests.rs`: the wire shape and the mapping policy
//! (server-managed exclusion, create-then-update, true-replace) change for
//! different reasons. Each conversion destructures and constructs exhaustively
//! (no `..`) so a new field is a compile error.

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

/// Shared by both `into_new_board` impls below — the two request structs have
/// identical content fields and differ only in `completion_column_id`.
struct BoardContentFields {
    name: String,
    description: Option<String>,
    sprint_prefix: Option<String>,
    card_prefix: Option<String>,
    task_sort_field: Option<super::super::SortFieldDto>,
    task_sort_order: Option<super::super::SortOrderDto>,
    sprint_duration_days: Option<u32>,
    task_list_view: Option<super::super::TaskListViewDto>,
}

fn new_board_from_content(
    content: BoardContentFields,
    completion_column_id: Option<Uuid>,
) -> NewBoard {
    let BoardContentFields {
        name,
        description,
        sprint_prefix,
        card_prefix,
        task_sort_field,
        task_sort_order,
        sprint_duration_days,
        task_list_view,
    } = content;
    NewBoard {
        name,
        description,
        sprint_prefix,
        card_prefix,
        task_sort_field: task_sort_field.map(Into::into),
        task_sort_order: task_sort_order.map(Into::into),
        sprint_duration_days,
        task_list_view: task_list_view.map(Into::into),
        completion_column_id,
    }
}

impl CreateBoardRequest {
    /// Split the identity (optional client id) from the content spec. The
    /// service mints the id when `None` and calls `Board::create(spec, id, now)`.
    /// Exhaustive destructure — no `..` — so a new field is a compile error.
    /// `completion_column_id` has no source field here (see the struct doc) —
    /// always `None`, since a board created this way always has zero columns.
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
        } = self;
        let spec = new_board_from_content(
            BoardContentFields {
                name,
                description,
                sprint_prefix,
                card_prefix,
                task_sort_field,
                task_sort_order,
                sprint_duration_days,
                task_list_view,
            },
            None,
        );
        (id, spec)
    }
}

impl ReplaceBoardRequest {
    /// Full-replace content spec for the `PUT /v1/boards/:id` create-or-replace
    /// seam. `completion_column_id` carries straight through — legitimate on
    /// the replace arm (an existing board may already have columns); the
    /// service still rejects it at the specific call site where this same
    /// spec ends up creating a brand-new board (zero columns regardless of
    /// which request type supplied it).
    pub fn into_new_board(self) -> NewBoard {
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
        } = self;
        new_board_from_content(
            BoardContentFields {
                name,
                description,
                sprint_prefix,
                card_prefix,
                task_sort_field: Some(task_sort_field),
                task_sort_order: Some(task_sort_order),
                sprint_duration_days,
                task_list_view: Some(task_list_view),
            },
            completion_column_id,
        )
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
    fn test_create_board_request_into_new_board_maps_all_content_fields() {
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
    }

    #[test]
    fn test_create_board_request_into_new_board_always_omits_completion_column_id() {
        // No source field exists to carry a value through -- this pins that
        // the resulting spec's completion_column_id is unconditionally None.
        let req = CreateBoardRequest {
            id: None,
            name: "B".to_string(),
            description: None,
            sprint_prefix: None,
            card_prefix: None,
            task_sort_field: None,
            task_sort_order: None,
            sprint_duration_days: None,
            task_list_view: None,
        };
        let (_id, spec) = req.into_new_board();
        assert_eq!(spec.completion_column_id, None);
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

    #[test]
    fn test_replace_board_request_into_new_board_carries_completion_column_id() {
        let col = Uuid::new_v4();
        let req = ReplaceBoardRequest {
            name: "Roadmap".to_string(),
            description: None,
            sprint_prefix: None,
            card_prefix: None,
            task_sort_field: SortFieldDto::Priority,
            task_sort_order: SortOrderDto::Ascending,
            sprint_duration_days: None,
            task_list_view: TaskListViewDto::GroupedByColumn,
            completion_column_id: Some(col),
        };
        let spec = req.into_new_board();
        assert_eq!(spec.name, "Roadmap");
        assert_eq!(spec.task_sort_field, Some(SortField::Priority));
        assert_eq!(spec.task_sort_order, Some(SortOrder::Ascending));
        assert_eq!(spec.task_list_view, Some(TaskListView::GroupedByColumn));
        assert_eq!(spec.completion_column_id, Some(col));
    }
}
