use kanban_api::{CreateBoardRequest, Patch, UpdateBoardRequest};
use kanban_domain::{BoardUpdate, FieldUpdate, KanbanError, KanbanResult, NewBoard};
use uuid::Uuid;

pub(crate) fn create_board_request(id: Option<Uuid>, spec: &NewBoard) -> CreateBoardRequest {
    let NewBoard {
        name,
        description,
        sprint_prefix,
        card_prefix,
        task_sort_field,
        task_sort_order,
        sprint_duration_days,
        task_list_view,
    } = spec;
    CreateBoardRequest {
        id,
        name: name.clone(),
        description: description.clone(),
        sprint_prefix: sprint_prefix.clone(),
        card_prefix: card_prefix.clone(),
        task_sort_field: task_sort_field.map(Into::into),
        task_sort_order: task_sort_order.map(Into::into),
        sprint_duration_days: *sprint_duration_days,
        task_list_view: task_list_view.map(Into::into),
    }
}

pub(crate) fn update_board_request(updates: &BoardUpdate) -> KanbanResult<UpdateBoardRequest> {
    let BoardUpdate {
        name,
        description,
        sprint_prefix,
        card_prefix,
        task_sort_field,
        task_sort_order,
        sprint_duration_days,
        task_list_view,
        active_sprint_id,
        position,
    } = updates;
    if !matches!(active_sprint_id, FieldUpdate::NoChange) {
        return Err(KanbanError::unsupported(
            "update_board.active_sprint_id over HTTP (server-managed field)",
        ));
    }
    if position.is_some() {
        return Err(KanbanError::unsupported(
            "update_board.position over HTTP (server-managed field)",
        ));
    }
    Ok(UpdateBoardRequest {
        name: name.clone(),
        description: Patch::from(description.clone()),
        sprint_prefix: Patch::from(sprint_prefix.clone()),
        card_prefix: Patch::from(card_prefix.clone()),
        task_sort_field: task_sort_field.map(Into::into),
        task_sort_order: task_sort_order.map(Into::into),
        sprint_duration_days: Patch::from(sprint_duration_days.clone()),
        task_list_view: task_list_view.map(Into::into),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_api::{SortFieldDto, SortOrderDto, TaskListViewDto};
    use kanban_domain::{FieldUpdate, SortField, SortOrder, TaskListView};

    #[test]
    fn test_create_board_request_carries_the_client_id_and_every_new_board_field() {
        let id = Uuid::new_v4();
        let spec = NewBoard {
            name: "Roadmap".to_string(),
            description: Some("desc".to_string()),
            sprint_prefix: Some("SPR".to_string()),
            card_prefix: Some("KAN".to_string()),
            task_sort_field: Some(SortField::Priority),
            task_sort_order: Some(SortOrder::Descending),
            sprint_duration_days: Some(21),
            task_list_view: Some(TaskListView::GroupedByColumn),
        };

        let req = create_board_request(Some(id), &spec);

        assert_eq!(req.id, Some(id));
        assert_eq!(req.name, "Roadmap");
        assert_eq!(req.description, Some("desc".to_string()));
        assert_eq!(req.sprint_prefix, Some("SPR".to_string()));
        assert_eq!(req.card_prefix, Some("KAN".to_string()));
        assert_eq!(req.task_sort_field, Some(SortFieldDto::Priority));
        assert_eq!(req.task_sort_order, Some(SortOrderDto::Descending));
        assert_eq!(req.sprint_duration_days, Some(21));
        assert_eq!(req.task_list_view, Some(TaskListViewDto::GroupedByColumn));
    }

    #[test]
    fn test_update_board_request_bridges_every_field_update_to_its_patch() {
        let updates = BoardUpdate {
            name: Some("Renamed".to_string()),
            description: FieldUpdate::Set("d".to_string()),
            sprint_prefix: FieldUpdate::Clear,
            card_prefix: FieldUpdate::NoChange,
            task_sort_field: Some(SortField::Priority),
            task_sort_order: Some(SortOrder::Ascending),
            sprint_duration_days: FieldUpdate::Set(14),
            task_list_view: Some(TaskListView::Flat),
            active_sprint_id: FieldUpdate::NoChange,
            position: None,
        };

        let req = update_board_request(&updates).unwrap();

        assert_eq!(req.name, Some("Renamed".to_string()));
        assert_eq!(req.description, Patch::Set("d".to_string()));
        assert_eq!(req.sprint_prefix, Patch::Clear);
        assert_eq!(req.card_prefix, Patch::NoChange);
        assert_eq!(req.task_sort_field, Some(SortFieldDto::Priority));
        assert_eq!(req.task_sort_order, Some(SortOrderDto::Ascending));
        assert_eq!(req.sprint_duration_days, Patch::Set(14));
        assert_eq!(req.task_list_view, Some(TaskListViewDto::Flat));
    }

    #[test]
    fn test_update_board_request_with_active_sprint_id_set_returns_unsupported_not_silent_drop() {
        let updates = BoardUpdate {
            active_sprint_id: FieldUpdate::Set(Uuid::new_v4()),
            ..Default::default()
        };

        let err = update_board_request(&updates).unwrap_err();

        assert!(err.is_unsupported());
        assert_eq!(
            err.to_string(),
            KanbanError::unsupported(
                "update_board.active_sprint_id over HTTP (server-managed field)"
            )
            .to_string()
        );
    }

    #[test]
    fn test_update_board_request_with_active_sprint_id_cleared_returns_unsupported_not_silent_drop()
    {
        let updates = BoardUpdate {
            active_sprint_id: FieldUpdate::Clear,
            ..Default::default()
        };

        let err = update_board_request(&updates).unwrap_err();

        assert!(err.is_unsupported());
    }

    #[test]
    fn test_update_board_request_with_position_set_returns_unsupported_not_silent_drop() {
        let updates = BoardUpdate {
            position: Some(3),
            ..Default::default()
        };

        let err = update_board_request(&updates).unwrap_err();

        assert!(err.is_unsupported());
    }
}
