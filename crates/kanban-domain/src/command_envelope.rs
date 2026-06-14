use crate::commands::Command;
use chrono::{DateTime, Utc};
use kanban_core::ClientId;
use uuid::Uuid;

/// A command with its execution context for audit and distributed tracing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommandEnvelope {
    pub command: Command,
    pub correlation_id: Uuid,
    pub issued_by: ClientId,
    pub timestamp: DateTime<Utc>,
}

impl CommandEnvelope {
    pub fn new(command: Command, correlation_id: Uuid, issued_by: ClientId) -> Self {
        Self {
            command,
            correlation_id,
            issued_by,
            timestamp: Utc::now(),
        }
    }

    /// Wrap a command with a generated correlation ID and the given client.
    pub fn wrap(command: Command, issued_by: ClientId) -> Self {
        Self::new(command, Uuid::new_v4(), issued_by)
    }
}

/// Wraps a command with a generated correlation ID and `ClientId::nil()` as the issuer.
/// Use [`CommandEnvelope::wrap`] when a real client identity is available.
impl From<Command> for CommandEnvelope {
    fn from(command: Command) -> Self {
        Self::wrap(command, ClientId::nil())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{BoardCommand, Command, CreateBoard};
    use chrono::Utc;
    use uuid::Uuid;

    fn make_cmd() -> Command {
        Command::Board(BoardCommand::Create(CreateBoard {
            id: Uuid::new_v4(),
            name: "Test".into(),
            card_prefix: None,
            position: 0,
        }))
    }

    #[test]
    fn test_from_command_uses_nil_client_id() {
        let envelope = CommandEnvelope::from(make_cmd());
        assert_eq!(envelope.issued_by, ClientId::nil());
    }

    #[test]
    fn test_wrap_uses_provided_client_id() {
        let client_id = ClientId::new();
        let envelope = CommandEnvelope::wrap(make_cmd(), client_id);
        assert_eq!(envelope.issued_by, client_id);
    }

    #[test]
    fn test_new_correlation_ids_are_unique() {
        let cmd = make_cmd();
        let e1 = CommandEnvelope::from(cmd.clone());
        let e2 = CommandEnvelope::from(cmd);
        assert_ne!(e1.correlation_id, e2.correlation_id);
    }

    #[test]
    fn test_timestamp_is_set_on_construction() {
        let before = Utc::now();
        let envelope = CommandEnvelope::from(make_cmd());
        let after = Utc::now();
        assert!(envelope.timestamp >= before);
        assert!(envelope.timestamp <= after);
    }

    #[test]
    fn test_serde_round_trip() {
        let client_id = ClientId::new();
        let original = CommandEnvelope::wrap(make_cmd(), client_id);
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: CommandEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.correlation_id, original.correlation_id);
        assert_eq!(deserialized.issued_by, original.issued_by);
        assert_eq!(deserialized.timestamp, original.timestamp);
    }
}
