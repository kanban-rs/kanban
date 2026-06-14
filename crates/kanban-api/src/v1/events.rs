use chrono::{DateTime, Utc};
use kanban_core::ClientId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn default_uuid() -> Uuid {
    Uuid::nil()
}

fn default_client_id() -> ClientId {
    ClientId::nil()
}

/// WebSocket push frame emitted by kanban-server on every successful mutation.
/// Clients filter by `writer_instance_id` to ignore their own writes.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ChangeEventFrame {
    pub writer_instance_id: Uuid,
    pub detected_at: DateTime<Utc>,
    #[serde(default = "default_uuid")]
    pub correlation_id: Uuid,
    #[serde(default = "default_client_id")]
    pub issued_by: ClientId,
}

impl ChangeEventFrame {
    /// Construct with an explicit timestamp (use in tests and server code).
    pub fn new(
        writer_instance_id: Uuid,
        correlation_id: Uuid,
        issued_by: ClientId,
        detected_at: DateTime<Utc>,
    ) -> Self {
        Self {
            writer_instance_id,
            detected_at,
            correlation_id,
            issued_by,
        }
    }

    /// Construct stamped with the current time (convenience for production use).
    pub fn now(writer_instance_id: Uuid, correlation_id: Uuid, issued_by: ClientId) -> Self {
        Self::new(writer_instance_id, correlation_id, issued_by, Utc::now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_event_frame_serde_round_trip() {
        let frame = ChangeEventFrame::new(
            Uuid::nil(),
            Uuid::nil(),
            ClientId::nil(),
            chrono::DateTime::from_timestamp(0, 0).unwrap(),
        );
        let json = serde_json::to_string(&frame).unwrap();
        let parsed: ChangeEventFrame = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.writer_instance_id, frame.writer_instance_id);
        assert_eq!(parsed.correlation_id, frame.correlation_id);
        assert_eq!(parsed.issued_by, frame.issued_by);
    }

    #[test]
    fn test_change_event_frame_now_populates_all_fields() {
        let instance_id = Uuid::new_v4();
        let correlation_id = Uuid::new_v4();
        let client_id = ClientId::new();
        let frame = ChangeEventFrame::now(instance_id, correlation_id, client_id);
        assert_eq!(frame.writer_instance_id, instance_id);
        assert_eq!(frame.correlation_id, correlation_id);
        assert_eq!(frame.issued_by, client_id);
    }

    #[test]
    fn test_change_event_frame_new_uses_explicit_timestamp() {
        let ts = chrono::DateTime::from_timestamp(1_000_000, 0).unwrap();
        let frame = ChangeEventFrame::new(Uuid::nil(), Uuid::nil(), ClientId::nil(), ts);
        assert_eq!(frame.detected_at, ts);
    }

    #[test]
    fn test_change_event_frame_deserializes_without_optional_fields() {
        let json = r#"{"writer_instance_id":"00000000-0000-0000-0000-000000000000","detected_at":"1970-01-01T00:00:00Z"}"#;
        let parsed: ChangeEventFrame = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.correlation_id, Uuid::nil());
        assert_eq!(parsed.issued_by, ClientId::nil());
    }
}
