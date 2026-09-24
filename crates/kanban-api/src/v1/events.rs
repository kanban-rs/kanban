use crate::InvalidationDto;
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

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Board,
    Column,
    Card,
    Sprint,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Created,
    Updated,
    Deleted,
}

impl ChangeKind {
    pub fn created_or_updated(created: bool) -> Self {
        if created {
            Self::Created
        } else {
            Self::Updated
        }
    }
}

/// SSE frame emitted by kanban-server on every successful mutation.
/// A client suppresses its own echoes by skipping frames whose `issued_by`
/// equals the UUID it sends in `X-Kanban-Client-Id`; a client that sends no
/// header instead falls back to filtering on `writer_instance_id`.
///
/// `entity_type`/`entity_id`/`kind` are `None` when the emitter cannot name
/// what changed (an external process wrote the file). Otherwise they name the
/// single entity that changed and how, and are retained for existing
/// consumers.
///
/// `invalidation` names the mutation's full blast radius: every entity a
/// consumer must treat as stale, not just the single entity above. `None`
/// means the emitter could not describe the change; a consumer must then
/// treat it the same as an explicit `InvalidationDto::All`.
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
    #[serde(default)]
    pub entity_type: Option<EntityType>,
    #[serde(default)]
    pub entity_id: Option<Uuid>,
    #[serde(default)]
    pub kind: Option<ChangeKind>,
    #[serde(default)]
    pub invalidation: Option<InvalidationDto>,
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
            entity_type: None,
            entity_id: None,
            kind: None,
            invalidation: None,
        }
    }

    /// Construct stamped with the current time (convenience for production use).
    pub fn now(writer_instance_id: Uuid, correlation_id: Uuid, issued_by: ClientId) -> Self {
        Self::new(writer_instance_id, correlation_id, issued_by, Utc::now())
    }

    /// Construct stamped with the current time, carrying an explicit (possibly
    /// absent) entity identity.
    #[allow(clippy::too_many_arguments)]
    pub fn for_entity(
        writer_instance_id: Uuid,
        correlation_id: Uuid,
        issued_by: ClientId,
        entity_type: Option<EntityType>,
        entity_id: Option<Uuid>,
        kind: Option<ChangeKind>,
    ) -> Self {
        Self {
            writer_instance_id,
            detected_at: Utc::now(),
            correlation_id,
            issued_by,
            entity_type,
            entity_id,
            kind,
            invalidation: None,
        }
    }

    pub fn with_invalidation(mut self, invalidation: InvalidationDto) -> Self {
        self.invalidation = Some(invalidation);
        self
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

    #[test]
    fn test_change_event_frame_carries_entity_identity() {
        let card_id = Uuid::new_v4();
        let frame = ChangeEventFrame::for_entity(
            Uuid::nil(),
            Uuid::nil(),
            ClientId::nil(),
            Some(EntityType::Card),
            Some(card_id),
            Some(ChangeKind::Created),
        );
        let json = serde_json::to_string(&frame).unwrap();
        let parsed: ChangeEventFrame = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.entity_type, Some(EntityType::Card));
        assert_eq!(parsed.entity_id, Some(card_id));
        assert_eq!(parsed.kind, Some(ChangeKind::Created));
    }

    #[test]
    fn test_change_event_frame_deserializes_without_entity_fields() {
        let json = r#"{"writer_instance_id":"00000000-0000-0000-0000-000000000000","detected_at":"1970-01-01T00:00:00Z"}"#;
        let parsed: ChangeEventFrame = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.correlation_id, Uuid::nil());
        assert_eq!(parsed.issued_by, ClientId::nil());
        assert!(parsed.entity_type.is_none());
        assert!(parsed.entity_id.is_none());
        assert!(parsed.kind.is_none());
    }

    #[test]
    fn test_change_event_frame_entity_fields_serialize_as_snake_case() {
        let card_id = Uuid::new_v4();
        let card_frame = ChangeEventFrame::for_entity(
            Uuid::nil(),
            Uuid::nil(),
            ClientId::nil(),
            Some(EntityType::Card),
            Some(card_id),
            Some(ChangeKind::Created),
        );
        let v = serde_json::to_value(&card_frame).unwrap();
        assert_eq!(v["entity_type"], "card");
        assert_eq!(v["kind"], "created");
        assert_eq!(v["entity_id"], card_id.to_string());

        let board_id = Uuid::new_v4();
        let board_frame = ChangeEventFrame::for_entity(
            Uuid::nil(),
            Uuid::nil(),
            ClientId::nil(),
            Some(EntityType::Board),
            Some(board_id),
            Some(ChangeKind::Deleted),
        );
        let v = serde_json::to_value(&board_frame).unwrap();
        assert_eq!(v["entity_type"], "board");
        assert_eq!(v["kind"], "deleted");
        assert_eq!(v["entity_id"], board_id.to_string());
    }

    #[test]
    fn test_change_event_frame_carries_the_mutation_invalidation() {
        let board_id = Uuid::new_v4();
        let ids = crate::EntityIdsDto {
            boards: vec![board_id],
            ..Default::default()
        };
        let dto = crate::InvalidationDto::Entities(ids);
        let frame = ChangeEventFrame::for_entity(
            Uuid::nil(),
            Uuid::nil(),
            ClientId::nil(),
            Some(EntityType::Board),
            Some(board_id),
            Some(ChangeKind::Deleted),
        )
        .with_invalidation(dto.clone());
        let json = serde_json::to_string(&frame).unwrap();
        let parsed: ChangeEventFrame = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.invalidation, Some(dto));
        let value = serde_json::to_value(&frame).unwrap();
        assert_eq!(value["invalidation"]["scope"], "entities");
    }

    #[test]
    fn test_change_event_frame_deserializes_without_the_invalidation_field() {
        let json = r#"{"writer_instance_id":"00000000-0000-0000-0000-000000000000","detected_at":"1970-01-01T00:00:00Z"}"#;
        let parsed: ChangeEventFrame = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.correlation_id, Uuid::nil());
        assert_eq!(parsed.issued_by, ClientId::nil());
        assert!(parsed.invalidation.is_none());

        let card_id = Uuid::new_v4();
        let json_with_entity = format!(
            r#"{{"writer_instance_id":"00000000-0000-0000-0000-000000000000","detected_at":"1970-01-01T00:00:00Z","entity_type":"card","entity_id":"{card_id}","kind":"created"}}"#
        );
        let parsed_with_entity: ChangeEventFrame = serde_json::from_str(&json_with_entity).unwrap();
        assert_eq!(parsed_with_entity.entity_type, Some(EntityType::Card));
        assert_eq!(parsed_with_entity.entity_id, Some(card_id));
        assert_eq!(parsed_with_entity.kind, Some(ChangeKind::Created));
        assert!(parsed_with_entity.invalidation.is_none());
    }

    #[test]
    fn test_change_event_frame_unscoped_serializes_entity_fields_as_null() {
        let frame = ChangeEventFrame::for_entity(
            Uuid::nil(),
            Uuid::nil(),
            ClientId::nil(),
            None,
            None,
            None,
        );
        let v = serde_json::to_value(&frame).unwrap();
        assert_eq!(v.get("entity_type"), Some(&serde_json::Value::Null));
        assert_eq!(v.get("entity_id"), Some(&serde_json::Value::Null));
        assert_eq!(v.get("kind"), Some(&serde_json::Value::Null));
    }
}
