use crate::commands::Command;
use chrono::{DateTime, Utc};
use kanban_core::{AppType, ClientId, KANBAN_VERSION};
use uuid::Uuid;

/// A batch of commands executed together as one atomic transaction,
/// with shared provenance for audit and distributed tracing. One
/// `KanbanContext::execute()` call produces exactly one CommandBatch.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CommandBatch {
    pub commands: Vec<Command>,
    pub correlation_id: Uuid,
    pub issued_by: ClientId,
    pub timestamp: DateTime<Utc>,
    pub app_type: AppType,
    pub app_version: String,
    pub session_id: Uuid,
}

impl CommandBatch {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        commands: Vec<Command>,
        correlation_id: Uuid,
        issued_by: ClientId,
        timestamp: DateTime<Utc>,
        app_type: AppType,
        app_version: String,
        session_id: Uuid,
    ) -> Self {
        Self {
            commands,
            correlation_id,
            issued_by,
            timestamp,
            app_type,
            app_version,
            session_id,
        }
    }

    /// Wrap a batch of commands with a generated correlation ID, a generated
    /// session ID, unknown app type, current version and timestamp, and the
    /// given client. Convenience for tests and direct construction.
    pub fn wrap(commands: Vec<Command>, issued_by: ClientId) -> Self {
        Self::new(
            commands,
            Uuid::new_v4(),
            issued_by,
            Utc::now(),
            AppType::Unknown,
            KANBAN_VERSION.to_string(),
            Uuid::new_v4(),
        )
    }
}

/// Wraps commands with nil client, generated correlation/session, unknown app
/// type, current version and time. Use `KanbanContext::execute()` for properly
/// attributed batches.
impl From<Vec<Command>> for CommandBatch {
    fn from(commands: Vec<Command>) -> Self {
        Self::wrap(commands, ClientId::nil())
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
    fn test_from_commands_uses_nil_client_id() {
        let batch = CommandBatch::from(vec![make_cmd()]);
        assert_eq!(batch.issued_by, ClientId::nil());
    }

    #[test]
    fn test_from_commands_uses_unknown_app_type() {
        let batch = CommandBatch::from(vec![make_cmd()]);
        assert_eq!(batch.app_type, AppType::Unknown);
    }

    #[test]
    fn test_from_commands_sets_kanban_version() {
        let batch = CommandBatch::from(vec![make_cmd()]);
        assert_eq!(batch.app_version, KANBAN_VERSION);
    }

    #[test]
    fn test_from_commands_generates_non_nil_session_id() {
        let batch = CommandBatch::from(vec![make_cmd()]);
        assert_ne!(batch.session_id, Uuid::nil());
    }

    #[test]
    fn test_wrap_uses_provided_client_id() {
        let client_id = ClientId::new();
        let batch = CommandBatch::wrap(vec![make_cmd()], client_id);
        assert_eq!(batch.issued_by, client_id);
    }

    #[test]
    fn test_wrap_generates_unique_correlation_id_per_call() {
        let cmd = make_cmd();
        let b1 = CommandBatch::wrap(vec![cmd.clone()], ClientId::nil());
        let b2 = CommandBatch::wrap(vec![cmd], ClientId::nil());
        assert_ne!(
            b1.correlation_id, b2.correlation_id,
            "each wrap() call must generate a distinct correlation_id"
        );
    }

    #[test]
    fn test_timestamp_is_set_on_construction() {
        let before = Utc::now();
        let batch = CommandBatch::from(vec![make_cmd()]);
        let after = Utc::now();
        assert!(
            batch.timestamp >= before && batch.timestamp <= after,
            "timestamp {} must be within [{}, {}]",
            batch.timestamp,
            before,
            after
        );
    }

    #[test]
    fn test_new_sets_all_fields() {
        let cmd = make_cmd();
        let corr = Uuid::new_v4();
        let client = ClientId::new();
        let now = Utc::now();
        let session = Uuid::new_v4();
        let batch = CommandBatch::new(
            vec![cmd.clone()],
            corr,
            client,
            now,
            AppType::Mcp,
            "1.2.3".into(),
            session,
        );
        assert_eq!(batch.commands, vec![cmd]);
        assert_eq!(batch.correlation_id, corr);
        assert_eq!(batch.issued_by, client);
        assert_eq!(batch.timestamp, now);
        assert_eq!(batch.app_type, AppType::Mcp);
        assert_eq!(batch.app_version, "1.2.3");
        assert_eq!(batch.session_id, session);
    }

    #[test]
    fn test_wrap_preserves_all_commands() {
        let c1 = make_cmd();
        let c2 = make_cmd();
        let batch = CommandBatch::wrap(vec![c1.clone(), c2.clone()], ClientId::nil());
        assert_eq!(batch.commands, vec![c1, c2]);
    }

    #[test]
    fn test_serde_round_trip_preserves_commands_and_provenance() {
        let client_id = ClientId::new();
        let c1 = make_cmd();
        let c2 = make_cmd();
        let original = CommandBatch::wrap(vec![c1, c2], client_id);
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: CommandBatch = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, original);
        assert_eq!(
            deserialized.commands.len(),
            2,
            "both commands must survive the round-trip"
        );
    }
}
