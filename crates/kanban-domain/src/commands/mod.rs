use crate::data_store::DataStore;
use crate::{KanbanError, KanbanResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod board_commands;
pub mod card;
pub mod cascade_commands;
pub mod column_commands;
pub mod dependency_commands;
pub mod sprint_commands;

pub(crate) fn default_card_prefix() -> String {
    crate::DEFAULT_CARD_PREFIX.to_string()
}

pub(crate) fn default_sprint_prefix() -> String {
    crate::DEFAULT_SPRINT_PREFIX.to_string()
}

pub use board_commands::*;
pub use card::*;
pub use cascade_commands::{CascadeCommand, SetArchivedCardsSprint};
pub use column_commands::*;
pub use dependency_commands::*;
pub use sprint_commands::*;

/// Every domain mutation flows through this enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "domain", rename_all = "snake_case")]
pub enum Command {
    Board(BoardCommand),
    Column(ColumnCommand),
    Card(CardCommand),
    Sprint(SprintCommand),
    Dependency(DependencyCommand),
    Cascade(CascadeCommand),
}

impl Command {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        match self {
            Command::Board(cmd) => cmd.execute(context),
            Command::Column(cmd) => cmd.execute(context),
            Command::Card(cmd) => cmd.execute(context),
            Command::Sprint(cmd) => cmd.execute(context),
            Command::Dependency(cmd) => cmd.execute(context),
            Command::Cascade(cmd) => cmd.execute(context),
        }
    }

    pub fn description(&self) -> String {
        match self {
            Command::Board(cmd) => cmd.description(),
            Command::Column(cmd) => cmd.description(),
            Command::Card(cmd) => cmd.description(),
            Command::Sprint(cmd) => cmd.description(),
            Command::Dependency(cmd) => cmd.description(),
            Command::Cascade(cmd) => cmd.description(),
        }
    }

    /// Build the inverse batch by reading pre-state from `store`.
    /// Called before the forward `execute` runs.
    ///
    /// An empty `Vec` is "this forward is a no-op; nothing to undo."
    /// `Err` means the inverse cannot be captured (entity missing,
    /// store error, or the command is synthetic-only).
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        match self {
            Command::Board(cmd) => cmd.capture_inverse(store),
            Command::Column(cmd) => cmd.capture_inverse(store),
            Command::Card(cmd) => cmd.capture_inverse(store),
            Command::Sprint(cmd) => cmd.capture_inverse(store),
            Command::Dependency(cmd) => cmd.capture_inverse(store),
            Command::Cascade(cmd) => cmd.capture_inverse(store),
        }
    }
}

/// Context passed to commands for mutation.
/// Holds a reference to the DataStore which uses interior mutability.
pub struct CommandContext<'a> {
    pub store: &'a dyn DataStore,
}

impl<'a> CommandContext<'a> {
    pub fn get_board(&self, id: Uuid) -> KanbanResult<crate::Board> {
        self.store
            .get_board(id)?
            .ok_or_else(|| KanbanError::not_found("Board", id))
    }

    pub fn get_card(&self, id: Uuid) -> KanbanResult<crate::Card> {
        self.store
            .get_card(id)?
            .ok_or_else(|| KanbanError::not_found("Card", id))
    }

    pub fn get_column(&self, id: Uuid) -> KanbanResult<crate::Column> {
        self.store
            .get_column(id)?
            .ok_or_else(|| KanbanError::not_found("Column", id))
    }

    /// Canonical column-membership check for the command tier: returns the
    /// column or `NotFound`. Mirror of the service-tier
    /// `KanbanContext::require_column` so the two layers share name + behavior
    /// (KAN-248). Commands that need to validate a target column FK before
    /// mutating route through this rather than an inline `iter().any`.
    pub fn require_column(&self, id: Uuid) -> KanbanResult<crate::Column> {
        self.get_column(id)
    }

    pub fn get_sprint(&self, id: Uuid) -> KanbanResult<crate::Sprint> {
        self.store
            .get_sprint(id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", id))
    }

    pub fn filter_valid_card_ids(&self, ids: &[Uuid], command_name: &str) -> Vec<Uuid> {
        let (valid, rejected): (Vec<_>, Vec<_>) = ids
            .iter()
            .copied()
            .partition(|&id| self.store.get_card(id).ok().flatten().is_some());
        for id in &rejected {
            tracing::warn!("{}: card {} not found, skipping", command_name, id);
        }
        valid
    }

    /// Returns `WipLimitExceeded` if adding `adding` cards to `column_id` would exceed its WIP
    /// limit. Cards whose IDs appear in `exclude` are not counted toward the current occupancy.
    /// Returns `not_found` if the column does not exist.
    pub fn check_wip_limit(
        &self,
        column_id: Uuid,
        adding: usize,
        exclude: &[Uuid],
    ) -> KanbanResult<()> {
        crate::wip::check_wip_limit(self.store, column_id, adding, exclude)
    }
}
