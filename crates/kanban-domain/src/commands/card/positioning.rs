use super::super::{Command, CommandContext};
use super::CardCommand;
use crate::data_store::DataStore;
use crate::{KanbanError, KanbanResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Move card to a different column
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveCard {
    pub card_id: Uuid,
    pub new_column_id: Uuid,
    pub new_position: i32,
}

impl MoveCard {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        // Canonical target-column membership check (KAN-248): reject a move to a
        // non-existent column up front via the shared helper.
        let column = context.require_column(self.new_column_id)?;
        context.check_wip_limit(self.new_column_id, 1, &[self.card_id])?;
        let mut card = context.get_card(self.card_id)?;
        card.move_to_column(self.new_column_id, self.new_position);
        // Keep board_id in sync with the target column's board -- cross-board
        // moves are intentionally permitted, not guarded against (KAN-963).
        card.board_id = column.board_id;
        context.store.upsert_card(card)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!(
            "Move card {} to column {}",
            self.card_id, self.new_column_id
        )
    }

    /// Inverse: another MoveCard pointing back to the card's current
    /// (column_id, position).
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let card = match store.get_card(self.card_id)? {
            Some(c) => c,
            None => return Err(KanbanError::not_found("Card", self.card_id)),
        };
        Ok(vec![Command::Card(CardCommand::Move(MoveCard {
            card_id: self.card_id,
            new_column_id: card.column_id,
            new_position: card.position,
        }))])
    }
}

/// Compact card positions in a column to be sequential (0, 1, 2, ...).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompactColumnPositions {
    pub column_id: Uuid,
}

impl CompactColumnPositions {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let cards = context.store.list_cards_by_column(self.column_id)?;
        for (i, mut card) in cards.into_iter().enumerate() {
            if card.position != i as i32 {
                card.position = i as i32;
                context.store.upsert_card(card)?;
            }
        }
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Compact positions in column {}", self.column_id)
    }

    /// Inverse: for each card in the column, emit a MoveCard back to its
    /// original position. Compaction is lossy without pre-state capture
    /// (multiple gappy arrangements compact to the same result), so this
    /// is the only way to reverse it.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let cards = store.list_cards_by_column(self.column_id)?;
        let mut commands: Vec<Command> = Vec::new();
        for card in cards {
            commands.push(Command::Card(CardCommand::Move(MoveCard {
                card_id: card.id,
                new_column_id: card.column_id,
                new_position: card.position,
            })));
        }
        Ok(commands)
    }
}
