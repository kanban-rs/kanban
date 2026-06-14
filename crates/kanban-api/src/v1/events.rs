use chrono::{DateTime, Utc};
use kanban_core::ClientId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// WebSocket push frame emitted by kanban-server on every successful mutation.
/// Clients filter by `writer_instance_id` to ignore their own writes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEventFrame {
    pub writer_instance_id: Uuid,
    pub detected_at: DateTime<Utc>,
    pub correlation_id: Uuid,
    pub issued_by: ClientId,
}

impl ChangeEventFrame {
    pub fn new(writer_instance_id: Uuid, correlation_id: Uuid, issued_by: ClientId) -> Self {
        Self {
            writer_instance_id,
            detected_at: Utc::now(),
            correlation_id,
            issued_by,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_event_frame_serde_round_trip() {
        let frame = ChangeEventFrame {
            writer_instance_id: Uuid::nil(),
            detected_at: chrono::DateTime::from_timestamp(0, 0).unwrap(),
            correlation_id: Uuid::nil(),
            issued_by: ClientId::nil(),
        };
        let json = serde_json::to_string(&frame).unwrap();
        let parsed: ChangeEventFrame = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.writer_instance_id, frame.writer_instance_id);
        assert_eq!(parsed.correlation_id, frame.correlation_id);
        assert_eq!(parsed.issued_by, frame.issued_by);
    }

    #[test]
    fn test_change_event_frame_new_populates_all_fields() {
        let instance_id = Uuid::new_v4();
        let correlation_id = Uuid::new_v4();
        let client_id = ClientId::new();
        let frame = ChangeEventFrame::new(instance_id, correlation_id, client_id);
        assert_eq!(frame.writer_instance_id, instance_id);
        assert_eq!(frame.correlation_id, correlation_id);
        assert_eq!(frame.issued_by, client_id);
    }
}
