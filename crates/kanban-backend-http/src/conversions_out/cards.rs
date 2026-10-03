use kanban_api::{CreateCardRequest, Patch, UpdateCardRequest};
use kanban_domain::{CardUpdate, NewCard};
use uuid::Uuid;

pub(crate) fn create_card_request(id: Option<Uuid>, spec: &NewCard) -> (String, CreateCardRequest) {
    let NewCard {
        column_id,
        title,
        description,
        priority,
        due_date,
        points,
        sprint_id,
    } = spec;
    let path = format!("/v1/columns/{column_id}/cards");
    let body = CreateCardRequest {
        id,
        title: title.clone(),
        description: description.clone(),
        priority: Some((*priority).into()),
        due_date: *due_date,
        points: *points,
        sprint_id: *sprint_id,
    };
    (path, body)
}

pub(crate) fn move_card_path(id: Uuid, column_id: Uuid, position: Option<i32>) -> String {
    match position {
        Some(position) => format!("/v1/cards/{id}/move?column_id={column_id}&position={position}"),
        None => format!("/v1/cards/{id}/move?column_id={column_id}"),
    }
}

pub(crate) fn assign_card_to_sprint_path(id: Uuid, sprint_id: Uuid) -> String {
    format!("/v1/cards/{id}/assign-sprint?sprint_id={sprint_id}")
}

pub(crate) fn update_card_request(updates: &CardUpdate) -> UpdateCardRequest {
    let CardUpdate {
        title,
        description,
        priority,
        status,
        position,
        column_id,
        due_date,
        points,
        sprint_id,
    } = updates;
    UpdateCardRequest {
        title: title.clone(),
        priority: priority.map(Into::into),
        status: status.map(Into::into),
        position: *position,
        column_id: *column_id,
        description: Patch::from(description.clone()),
        due_date: Patch::from(due_date.clone()),
        points: Patch::from(points.clone()),
        sprint_id: Patch::from(sprint_id.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_api::{CardPriorityDto, CardStatusDto, Patch};
    use kanban_domain::{CardPriority, CardStatus, FieldUpdate};

    #[test]
    fn test_create_card_request_puts_column_id_in_the_path_and_always_sends_the_priority() {
        let column_id = Uuid::new_v4();
        let id = Uuid::new_v4();
        let spec = NewCard {
            column_id,
            title: "Task".to_string(),
            description: None,
            priority: CardPriority::High,
            due_date: None,
            points: None,
            sprint_id: None,
        };

        let (path, body) = create_card_request(Some(id), &spec);

        assert_eq!(path, format!("/v1/columns/{column_id}/cards"));
        assert_eq!(body.id, Some(id));
        assert_eq!(body.priority, Some(CardPriorityDto::High));
        let value = serde_json::to_value(&body).unwrap();
        assert!(value.get("column_id").is_none());
    }

    #[test]
    fn test_update_card_request_bridges_all_four_patch_fields_and_copies_the_rest() {
        let column_id = Uuid::new_v4();
        let updates = CardUpdate {
            title: Some("New".to_string()),
            description: FieldUpdate::Set("d".to_string()),
            priority: Some(CardPriority::Low),
            status: Some(CardStatus::Done),
            position: Some(2),
            column_id: Some(column_id),
            due_date: FieldUpdate::Clear,
            points: FieldUpdate::Set(3),
            sprint_id: FieldUpdate::NoChange,
        };

        let req = update_card_request(&updates);

        assert_eq!(req.title, Some("New".to_string()));
        assert_eq!(req.description, Patch::Set("d".to_string()));
        assert_eq!(req.priority, Some(CardPriorityDto::Low));
        assert_eq!(req.status, Some(CardStatusDto::Done));
        assert_eq!(req.position, Some(2));
        assert_eq!(req.column_id, Some(column_id));
        assert_eq!(req.due_date, Patch::Clear);
        assert_eq!(req.points, Patch::Set(3));
        assert_eq!(req.sprint_id, Patch::NoChange);
    }

    #[test]
    fn test_move_card_path_omits_position_when_none_so_the_server_appends() {
        let id = Uuid::new_v4();
        let column_id = Uuid::new_v4();

        let path = move_card_path(id, column_id, None);

        assert_eq!(path, format!("/v1/cards/{id}/move?column_id={column_id}"));
        assert!(!path.contains("position"));
    }

    #[test]
    fn test_move_card_path_carries_an_explicit_position() {
        let id = Uuid::new_v4();
        let column_id = Uuid::new_v4();

        let path = move_card_path(id, column_id, Some(2));

        assert_eq!(
            path,
            format!("/v1/cards/{id}/move?column_id={column_id}&position=2")
        );
    }

    #[test]
    fn test_assign_card_to_sprint_path_carries_the_sprint_id_query_param() {
        let id = Uuid::new_v4();
        let sprint_id = Uuid::new_v4();

        let path = assign_card_to_sprint_path(id, sprint_id);

        assert_eq!(
            path,
            format!("/v1/cards/{id}/assign-sprint?sprint_id={sprint_id}")
        );
    }
}
