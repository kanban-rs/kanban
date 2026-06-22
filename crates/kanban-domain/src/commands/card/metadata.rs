use super::super::{Command, CommandContext};
use super::{CardCommand, UpdateCard};
use crate::data_store::DataStore;
use crate::{CardUpdate, KanbanError, KanbanResult};
use kanban_core::Editable;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Apply card metadata from a DTO (used by JSON editor).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApplyCardMetadata {
    pub card_id: Uuid,
    pub dto: crate::editable::CardMetadataDto,
}

impl ApplyCardMetadata {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let mut card = context.get_card(self.card_id)?;
        self.dto.clone().apply_to(&mut card);
        context.store.upsert_card(card)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Apply card metadata for {}", self.card_id)
    }

    /// Inverse: emit an `UpdateCard` (not another `ApplyCardMetadata`)
    /// because `CardMetadataDto.apply_to` is asymmetric — it can set
    /// `points` / `due_date` but `None` in the DTO means "don't change",
    /// so it can't clear those fields. `UpdateCard` with
    /// `FieldUpdate::Set`/`FieldUpdate::Clear` covers the full reversal.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        use crate::field_update::FieldUpdate;
        let card = match store.get_card(self.card_id)? {
            Some(c) => c,
            None => return Err(KanbanError::not_found("Card", self.card_id)),
        };
        let updates = CardUpdate {
            // priority / status are always written by apply_to (when the
            // DTO string parses); restore them unconditionally.
            priority: Some(card.priority),
            status: Some(card.status),
            // points / due_date are only written by apply_to when Some
            // in the DTO. Restore unconditionally too — it's cheap and
            // correct.
            points: match card.points {
                Some(v) => FieldUpdate::Set(v),
                None => FieldUpdate::Clear,
            },
            due_date: match card.due_date {
                Some(v) => FieldUpdate::Set(v),
                None => FieldUpdate::Clear,
            },
            ..Default::default()
        };
        Ok(vec![Command::Card(CardCommand::Update(UpdateCard {
            card_id: self.card_id,
            updates,
        }))])
    }
}
