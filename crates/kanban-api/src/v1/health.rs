use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Body of `GET /health`. Shared by `kanban-server` and `HttpBackend`'s
/// open probe so the field names cannot drift.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub instance_id: Uuid,
    /// The server's `KANBAN_VERSION`. Absent from servers that predate the
    /// version handshake, which decode as `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

impl HealthResponse {
    pub fn ok(instance_id: Uuid, version: &str) -> Self {
        Self {
            status: "ok".to_string(),
            instance_id,
            version: Some(version.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
