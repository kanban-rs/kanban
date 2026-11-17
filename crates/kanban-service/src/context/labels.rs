use super::KanbanContext;
use kanban_domain::commands::{CardCommand, Command, DeleteLabel, LabelCommand, UpdateCard, UpsertLabel};
use kanban_domain::{CardUpdate, FieldUpdate, Invalidation, KanbanError, KanbanResult, Label};
use uuid::Uuid;

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

    /// Add a label to a batch of cards by id. The label is auto-registered in
    /// the global registry (empty color/description) if, and only if, it does
    /// not already exist, so an existing label's metadata is never clobbered.
    /// Runs as one undo unit; a missing card id fails the whole batch.
    pub fn assign_label_to_cards_impl(
        &mut self,
        label_name: String,
        card_ids: Vec<Uuid>,
    ) -> KanbanResult<(usize, Invalidation)> {
        if self.backend.remote_writes().is_some() {
            return Err(KanbanError::unsupported("assign_label_to_cards"));
        }
        let invalidation = self.execute_with(|store| {
            let mut cmds = Vec::new();
            if !card_ids.is_empty() && store.get_label(&label_name)?.is_none() {
                cmds.push(Command::Label(LabelCommand::Upsert(UpsertLabel {
                    label: Label::named(label_name.clone()),
                })));
            }
            for card_id in &card_ids {
                let card = store
                    .get_card(*card_id)?
                    .ok_or_else(|| KanbanError::not_found("Card", *card_id))?;
                let mut tags = card.tags.clone();
                if !tags.iter().any(|t| t == &label_name) {
                    tags.push(label_name.clone());
                }
                cmds.push(Command::Card(CardCommand::Update(UpdateCard {
                    card_id: *card_id,
                    updates: CardUpdate {
                        tags: FieldUpdate::Set(tags),
                        ..Default::default()
                    },
                })));
            }
            Ok(cmds)
        })?;
        Ok((card_ids.len(), invalidation))
    }

    /// Remove a label from a batch of cards by id. The global registry entry
    /// is left intact; a card that does not carry the label is left
    /// untouched. Runs as one undo unit; a missing card id fails the batch.
    pub fn remove_label_from_cards_impl(
        &mut self,
        label_name: String,
        card_ids: Vec<Uuid>,
    ) -> KanbanResult<(usize, Invalidation)> {
        if self.backend.remote_writes().is_some() {
            return Err(KanbanError::unsupported("remove_label_from_cards"));
        }
        let invalidation = self.execute_with(|store| {
            let mut cmds = Vec::new();
            for card_id in &card_ids {
                let card = store
                    .get_card(*card_id)?
                    .ok_or_else(|| KanbanError::not_found("Card", *card_id))?;
                let tags = card
                    .tags
                    .iter()
                    .filter(|t| t.as_str() != label_name.as_str())
                    .cloned()
                    .collect();
                if tags != card.tags {
                    cmds.push(Command::Card(CardCommand::Update(UpdateCard {
                        card_id: *card_id,
                        updates: CardUpdate {
                            tags: FieldUpdate::Set(tags),
                            ..Default::default()
                        },
                    })));
                }
            }
            Ok(cmds)
        })?;
        Ok((card_ids.len(), invalidation))
    }
}
