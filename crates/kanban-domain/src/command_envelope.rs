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

impl From<Command> for CommandEnvelope {
    fn from(command: Command) -> Self {
        Self::wrap(command, ClientId::nil())
    }
}
