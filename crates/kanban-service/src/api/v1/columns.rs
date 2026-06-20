#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::FieldUpdate;

    #[test]
    fn test_create_column_request_serde_round_trip() {
        let req = CreateColumnRequest {
            name: "In Review".to_string(),
            wip_limit: Some(3),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: CreateColumnRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, req.name);
        assert_eq!(back.wip_limit, req.wip_limit);
    }

    #[test]
    fn test_create_column_request_minimal_omits_wip_limit() {
        let json = r#"{"name":"Backlog"}"#;
        let back: CreateColumnRequest = serde_json::from_str(json).unwrap();
        assert_eq!(back.name, "Backlog");
        assert_eq!(back.wip_limit, None);
    }

    #[test]
    fn test_update_column_request_three_state_wip_limit_round_trip() {
        let req = UpdateColumnRequest {
            name: Some("Done".to_string()),
            position: Some(4),
            wip_limit: FieldUpdate::Clear,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: UpdateColumnRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, Some("Done".to_string()));
        assert_eq!(back.position, Some(4));
        assert_eq!(back.wip_limit, FieldUpdate::Clear);
    }

    #[test]
    fn test_update_column_request_defaults_to_no_change() {
        let json = r#"{}"#;
        let back: UpdateColumnRequest = serde_json::from_str(json).unwrap();
        assert_eq!(back.name, None);
        assert_eq!(back.position, None);
        assert_eq!(back.wip_limit, FieldUpdate::NoChange);
    }

    #[test]
    fn test_reorder_column_request_serde_round_trip() {
        let req = ReorderColumnRequest { position: 2 };
        let json = serde_json::to_string(&req).unwrap();
        let back: ReorderColumnRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.position, req.position);
    }
}
