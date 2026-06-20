#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{FieldUpdate, SortField, SortOrder};
    use uuid::Uuid;

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
    fn test_update_board_request_three_state_fields_round_trip() {
        let req = UpdateBoardRequest {
            name: Some("Renamed".to_string()),
            description: FieldUpdate::Set("new desc".to_string()),
            sprint_prefix: FieldUpdate::Clear,
            card_prefix: FieldUpdate::NoChange,
            task_sort_field: Some(SortField::CreatedAt),
            task_sort_order: None,
            completion_column_id: FieldUpdate::Set(Uuid::nil()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: UpdateBoardRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, Some("Renamed".to_string()));
        assert_eq!(back.description, FieldUpdate::Set("new desc".to_string()));
        assert_eq!(back.sprint_prefix, FieldUpdate::Clear);
        assert_eq!(back.card_prefix, FieldUpdate::NoChange);
        assert_eq!(back.completion_column_id, FieldUpdate::Set(Uuid::nil()));
    }

    #[test]
    fn test_update_board_request_defaults_to_no_change() {
        let json = r#"{}"#;
        let back: UpdateBoardRequest = serde_json::from_str(json).unwrap();
        assert_eq!(back.name, None);
        assert_eq!(back.description, FieldUpdate::NoChange);
        assert_eq!(back.completion_column_id, FieldUpdate::NoChange);
    }
}
