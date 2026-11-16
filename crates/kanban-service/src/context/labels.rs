use super::KanbanContext;
use kanban_domain::commands::{Command, DeleteLabel, LabelCommand, UpsertLabel};
use kanban_domain::{Invalidation, KanbanError, KanbanResult, Label};

impl KanbanContext {
    /// Read all labels from the global registry.
    pub fn list_labels_impl(&self) -> KanbanResult<Vec<Label>> {
        self.data_store().list_labels()
    }

    /// Read a single label by its case-sensitive name.
    pub fn get_label_impl(&self, name: &str) -> KanbanResult<Option<Label>> {
        self.data_store().get_label(name)
    }

    /// Create or update a label (upsert by name).
    pub fn upsert_label_impl(&mut self, label: Label) -> KanbanResult<(Label, Invalidation)> {
        if self.backend.remote_writes().is_some() {
            return Err(KanbanError::unsupported("upsert_label"));
        }
        let cmd = Command::Label(LabelCommand::Upsert(UpsertLabel {
            label: label.clone(),
        }));
        let invalidation = self.execute(vec![cmd])?;
        Ok((label, invalidation))
    }

    /// Delete a label by name.
    pub fn delete_label_impl(&mut self, name: String) -> KanbanResult<Invalidation> {
        if self.backend.remote_writes().is_some() {
            return Err(KanbanError::unsupported("delete_label"));
        }
        let cmd = Command::Label(LabelCommand::Delete(DeleteLabel { name }));
        self.execute(vec![cmd])
    }
}
