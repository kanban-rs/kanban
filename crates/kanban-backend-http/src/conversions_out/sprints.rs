use kanban_api::{ActivateSprintRequest, CreateSprintRequest, Patch, UpdateSprintRequest};
use kanban_domain::{FieldUpdate, KanbanError, KanbanResult, SprintUpdate};
use uuid::Uuid;

pub(crate) fn create_sprint_request(
    id: Option<Uuid>,
    name: Option<&str>,
    prefix: Option<&str>,
) -> CreateSprintRequest {
    CreateSprintRequest {
        id,
        name: name.map(str::to_string),
        prefix: prefix.map(str::to_string),
        card_prefix: None,
    }
}

pub(crate) fn update_sprint_request(updates: &SprintUpdate) -> KanbanResult<UpdateSprintRequest> {
    let SprintUpdate {
        name,
        name_index,
        prefix,
        card_prefix,
        status,
        start_date,
        end_date,
    } = updates;
    if !matches!(name_index, FieldUpdate::NoChange) {
        return Err(KanbanError::unsupported(
            "update_sprint.name_index over HTTP (server-managed field)",
        ));
    }
    if status.is_some() {
        return Err(KanbanError::unsupported(
            "update_sprint.status over HTTP (server-managed field)",
        ));
    }
    Ok(UpdateSprintRequest {
        name: name.clone(),
        prefix: Patch::from(prefix.clone()),
        card_prefix: Patch::from(card_prefix.clone()),
        start_date: Patch::from(start_date.clone()),
        end_date: Patch::from(end_date.clone()),
    })
}

pub(crate) fn activate_sprint_request(duration_days: Option<i32>) -> ActivateSprintRequest {
    ActivateSprintRequest { duration_days }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activate_request_body_omits_duration_days_when_the_caller_passed_none() {
        let req = activate_sprint_request(None);
        let value = serde_json::to_value(&req).unwrap();

        let duration_days = value.get("duration_days");
        assert!(
            duration_days.is_none() || duration_days == Some(&serde_json::Value::Null),
            "expected duration_days absent or null, got {duration_days:?}"
        );

        let req = activate_sprint_request(Some(7));
        let value = serde_json::to_value(&req).unwrap();
        assert_eq!(value.get("duration_days"), Some(&serde_json::json!(7)));
    }

    #[test]
    fn test_create_sprint_request_never_carries_a_number_or_index() {
        let req = create_sprint_request(Some(Uuid::new_v4()), Some("Sprint 1"), Some("SPR"));

        let value = serde_json::to_value(&req).unwrap();
        let mut keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();

        assert_eq!(keys, vec!["card_prefix", "id", "name", "prefix"]);
    }

    #[test]
    fn test_create_sprint_request_carries_id_name_and_prefix() {
        let id = Uuid::new_v4();

        let req = create_sprint_request(Some(id), Some("Sprint 1"), Some("SPR"));

        assert_eq!(req.id, Some(id));
        assert_eq!(req.name, Some("Sprint 1".to_string()));
        assert_eq!(req.prefix, Some("SPR".to_string()));
        assert_eq!(req.card_prefix, None);
    }

    #[test]
    fn test_update_sprint_request_declines_name_index_as_server_managed() {
        let updates = SprintUpdate {
            name_index: FieldUpdate::Set(3),
            ..Default::default()
        };

        let err = update_sprint_request(&updates).unwrap_err();

        assert!(err.is_unsupported());
        assert_eq!(
            err.to_string(),
            KanbanError::unsupported("update_sprint.name_index over HTTP (server-managed field)")
                .to_string()
        );
    }

    #[test]
    fn test_update_sprint_request_declines_status_as_server_managed() {
        let updates = SprintUpdate {
            status: Some(kanban_domain::SprintStatus::Active),
            ..Default::default()
        };

        let err = update_sprint_request(&updates).unwrap_err();

        assert!(err.is_unsupported());
        assert_eq!(
            err.to_string(),
            KanbanError::unsupported("update_sprint.status over HTTP (server-managed field)")
                .to_string()
        );
    }

    #[test]
    fn test_update_sprint_request_bridges_dates() {
        let start = chrono::Utc::now();
        let updates = SprintUpdate {
            start_date: FieldUpdate::Set(start),
            end_date: FieldUpdate::Clear,
            ..Default::default()
        };

        let req = update_sprint_request(&updates).unwrap();

        assert_eq!(req.start_date, Patch::Set(start));
        assert_eq!(req.end_date, Patch::Clear);
    }

    #[test]
    fn test_update_sprint_request_bridges_name_prefix_and_card_prefix() {
        let updates = SprintUpdate {
            name: Some("Renamed".to_string()),
            prefix: FieldUpdate::Set("SPR".to_string()),
            card_prefix: FieldUpdate::Clear,
            ..Default::default()
        };

        let req = update_sprint_request(&updates).unwrap();

        assert_eq!(req.name, Some("Renamed".to_string()));
        assert_eq!(req.prefix, Patch::Set("SPR".to_string()));
        assert_eq!(req.card_prefix, Patch::Clear);
    }
}
