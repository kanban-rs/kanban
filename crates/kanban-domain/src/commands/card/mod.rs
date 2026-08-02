use super::{Command, CommandContext};
use crate::data_store::DataStore;
use crate::KanbanResult;
use serde::{Deserialize, Serialize};

mod lifecycle;
mod metadata;
mod positioning;
mod sprint_binding;

pub use lifecycle::*;
pub use metadata::*;
pub use positioning::*;
pub use sprint_binding::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum CardCommand {
    Create(CreateCard),
    Update(UpdateCard),
    Move(MoveCard),
    Restore(RestoreCard),
    Delete(DeleteCard),
    Archive(ArchiveCards),
    AssignToSprint(AssignCardsToSprint),
    UnassignFromSprint(UnassignCardFromSprint),
    ApplyMetadata(ApplyCardMetadata),
    CompactPositions(CompactColumnPositions),
    /// Synthetic: restore a card's sprint binding and sprint_logs to a
    /// captured pre-state. Emitted by Assign/Unassign inverses; not a
    /// user-facing command.
    RestoreSprintAttachment(RestoreCardSprintAttachment),
}

impl CardCommand {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        match self {
            CardCommand::Create(c) => c.execute(context),
            CardCommand::Update(c) => c.execute(context),
            CardCommand::Move(c) => c.execute(context),
            CardCommand::Restore(c) => c.execute(context),
            CardCommand::Delete(c) => c.execute(context),
            CardCommand::Archive(c) => c.execute(context),
            CardCommand::AssignToSprint(c) => c.execute(context),
            CardCommand::UnassignFromSprint(c) => c.execute(context),
            CardCommand::ApplyMetadata(c) => c.execute(context),
            CardCommand::CompactPositions(c) => c.execute(context),
            CardCommand::RestoreSprintAttachment(c) => c.execute(context),
        }
    }

    pub fn description(&self) -> String {
        match self {
            CardCommand::Create(c) => c.description(),
            CardCommand::Update(c) => c.description(),
            CardCommand::Move(c) => c.description(),
            CardCommand::Restore(c) => c.description(),
            CardCommand::Delete(c) => c.description(),
            CardCommand::Archive(c) => c.description(),
            CardCommand::AssignToSprint(c) => c.description(),
            CardCommand::UnassignFromSprint(c) => c.description(),
            CardCommand::ApplyMetadata(c) => c.description(),
            CardCommand::CompactPositions(c) => c.description(),
            CardCommand::RestoreSprintAttachment(c) => c.description(),
        }
    }

    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        match self {
            CardCommand::Update(c) => c.capture_inverse(store),
            CardCommand::Move(c) => c.capture_inverse(store),
            CardCommand::UnassignFromSprint(c) => c.capture_inverse(store),
            CardCommand::ApplyMetadata(c) => c.capture_inverse(store),
            CardCommand::Archive(c) => c.capture_inverse(store),
            CardCommand::AssignToSprint(c) => c.capture_inverse(store),
            CardCommand::CompactPositions(c) => c.capture_inverse(store),
            CardCommand::Create(c) => c.capture_inverse(store),
            CardCommand::Restore(c) => c.capture_inverse(store),
            CardCommand::Delete(c) => c.capture_inverse(store),
            CardCommand::RestoreSprintAttachment(c) => c.capture_inverse(store),
        }
    }
}
