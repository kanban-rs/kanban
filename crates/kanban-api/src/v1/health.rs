#[cfg(test)]
mod tests {
    use super::HealthResponse;
    use uuid::Uuid;

    #[test]
    fn test_health_response_without_a_version_field_decodes_as_none() {
        let id = Uuid::new_v4();
        let body = format!(r#"{{"status":"ok","instance_id":"{id}"}}"#);
        let parsed: HealthResponse = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed.status, "ok");
        assert_eq!(parsed.instance_id, id);
        assert_eq!(parsed.version, None);
    }

    #[test]
    fn test_health_response_ok_serializes_status_instance_id_and_version() {
        let id = Uuid::new_v4();
        let health = HealthResponse::ok(id, "0.11.0");
        let value = serde_json::to_value(&health).unwrap();
        assert_eq!(value["status"], "ok");
        assert_eq!(value["instance_id"], id.to_string());
        assert_eq!(value["version"], "0.11.0");

        let round_tripped: HealthResponse = serde_json::from_value(value).unwrap();
        assert_eq!(round_tripped, health);
    }
}
