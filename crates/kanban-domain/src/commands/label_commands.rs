use super::{Command, CommandContext};
use crate::data_store::DataStore;
use crate::{KanbanResult, Label};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum LabelCommand {
    Upsert(UpsertLabel),
    Delete(DeleteLabel),
}

impl LabelCommand {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        match self {
            LabelCommand::Upsert(c) => c.execute(context),
            LabelCommand::Delete(c) => c.execute(context),
        }
    }

    pub fn description(&self) -> String {
        match self {
            LabelCommand::Upsert(c) => c.description(),
            LabelCommand::Delete(c) => c.description(),
        }
    }

    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        match self {
            LabelCommand::Upsert(c) => c.capture_inverse(store),
            LabelCommand::Delete(c) => c.capture_inverse(store),
        }
    }

    pub fn touched_entities(&self) -> Option<crate::EntityIds> {
        match self {
            LabelCommand::Upsert(c) => c.touched_entities(),
            LabelCommand::Delete(c) => c.touched_entities(),
        }
    }
}

/// Create or update a label (upsert by name).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpsertLabel {
    pub label: Label,
}

impl UpsertLabel {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        context.store.upsert_label(self.label.clone())?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Upsert label '{}'", self.label.name)
    }

    /// Labels are not tracked in `EntityIds`, so always `Invalidation::All`.
    pub fn touched_entities(&self) -> Option<crate::EntityIds> {
        None
    }

    /// Inverse: if the label existed before this upsert, the inverse restores
    /// the prior state via `UpsertLabel`; if it did not exist, the inverse
    /// deletes it via `DeleteLabel`.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        match store.get_label(&self.label.name)? {
            Some(prior) => Ok(vec![Command::Label(LabelCommand::Upsert(UpsertLabel {
                label: prior,
            }))]),
            None => Ok(vec![Command::Label(LabelCommand::Delete(DeleteLabel {
                name: self.label.name.clone(),
            }))]),
        }
    }
}

/// Delete a label by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteLabel {
    pub name: String,
}

impl DeleteLabel {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        context.store.delete_label(&self.name)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Delete label '{}'", self.name)
    }

    /// Labels are not tracked in `EntityIds`, so always `Invalidation::All`.
    pub fn touched_entities(&self) -> Option<crate::EntityIds> {
        None
    }

    /// Inverse: if the label exists at capture time (before the delete runs),
    /// the inverse restores it via `UpsertLabel` with the captured state.
    /// If it does not exist, the delete is a no-op and there is nothing to
    /// undo.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        match store.get_label(&self.name)? {
            Some(label) => Ok(vec![Command::Label(LabelCommand::Upsert(UpsertLabel {
                label,
            }))]),
            None => Ok(vec![]),
        }
    }
}
