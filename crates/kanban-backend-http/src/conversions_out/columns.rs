use kanban_api::{CreateColumnRequest, Patch, UpdateColumnRequest};
use kanban_domain::{ColumnUpdate, NewColumn};
use uuid::Uuid;

pub(crate) fn create_column_request(
    board_id: Uuid,
    spec: &NewColumn,
) -> (String, CreateColumnRequest) {
    let NewColumn {
        board_id: _,
        name,
        wip_limit,
        default_status,
    } = spec;
    let path = format!("/v1/boards/{board_id}/columns");
    let body = CreateColumnRequest {
        id: None,
        name: name.clone(),
        wip_limit: *wip_limit,
        default_status: default_status.map(Into::into),
    };
    (path, body)
}

pub(crate) fn update_column_request(updates: &ColumnUpdate) -> UpdateColumnRequest {
    let ColumnUpdate {
        name,
        position,
        wip_limit,
        default_status,
    } = updates;
    UpdateColumnRequest {
        name: name.clone(),
        position: *position,
        wip_limit: Patch::from(wip_limit.clone()),
        default_status: match default_status {
            None => Patch::NoChange,
            Some(None) => Patch::Clear,
            Some(Some(status)) => Patch::Set((*status).into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_api::{CardStatusDto, Patch};
    use kanban_domain::{CardStatus, FieldUpdate};

    #[test]
    fn test_create_column_request_puts_the_routing_board_id_in_the_path_and_never_in_the_body() {
        let routing_board_id = Uuid::new_v4();
        let spec_board_id = Uuid::new_v4();
        let spec = NewColumn {
            board_id: spec_board_id,
            name: "Doing".to_string(),
            wip_limit: Some(3),
            default_status: Some(CardStatus::Done),
        };

        let (path, body) = create_column_request(routing_board_id, &spec);

        assert_eq!(path, format!("/v1/boards/{routing_board_id}/columns"));
        assert_eq!(body.id, None);
        assert_eq!(body.wip_limit, Some(3));
        assert_eq!(body.default_status, Some(CardStatusDto::Done));
        let value = serde_json::to_value(&body).unwrap();
        assert!(value.get("board_id").is_none());
    }

    #[test]
    fn test_update_column_request_bridges_the_three_state_default_status() {
        let no_change = ColumnUpdate {
            default_status: None,
            ..Default::default()
        };
        assert_eq!(
            update_column_request(&no_change).default_status,
            Patch::NoChange
        );

        let clear = ColumnUpdate {
            default_status: Some(None),
            ..Default::default()
        };
        assert_eq!(update_column_request(&clear).default_status, Patch::Clear);

        let set = ColumnUpdate {
            default_status: Some(Some(CardStatus::Done)),
            ..Default::default()
        };
        assert_eq!(
            update_column_request(&set).default_status,
            Patch::Set(CardStatusDto::Done)
        );
    }

    #[test]
    fn test_update_column_request_bridges_wip_limit_and_copies_name_and_position() {
        let updates = ColumnUpdate {
            name: Some("Done".to_string()),
            position: Some(4),
            wip_limit: FieldUpdate::Clear,
            default_status: None,
        };

        let req = update_column_request(&updates);

        assert_eq!(req.name, Some("Done".to_string()));
        assert_eq!(req.position, Some(4));
        assert_eq!(req.wip_limit, Patch::Clear);
    }
}
