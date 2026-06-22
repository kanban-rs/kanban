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
        context.check_wip_limit(self.new_column_id, 1, &[self.card_id])?;
        let mut card = context.get_card(self.card_id)?;
        card.move_to_column(self.new_column_id, self.new_position);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_helpers::TestContext;

    #[test]
    fn test_move_card_not_found_returns_error() {
        let tc = TestContext::new();
        let column = crate::Column::new(Uuid::new_v4(), "Col", 0);
        let column_id = column.id;
        tc.store.upsert_column(column).unwrap();
        let context = tc.as_command_context();
        let cmd = MoveCard {
            card_id: Uuid::new_v4(),
            new_column_id: column_id,
            new_position: 0,
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_move_card_column_not_found_returns_error() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("Test", Some("TST"));
        let card = crate::Card::new(&mut board, Uuid::new_v4(), "Card", 0);
        let card_id = card.id;
        tc.store.upsert_card(card).unwrap();
        let context = tc.as_command_context();
        let cmd = MoveCard {
            card_id,
            new_column_id: Uuid::new_v4(),
            new_position: 0,
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_move_card_exceeding_wip_limit_returns_error() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("Test", Some("TST"));
        let src_col = crate::Column::new(board.id, "Source", 0);
        let mut dst_col = crate::Column::new(board.id, "Dest", 1);
        dst_col.wip_limit = Some(1);
        let dst_id = dst_col.id;
        let existing = crate::Card::new(&mut board, dst_id, "Existing", 0);
        let mover = crate::Card::new(&mut board, src_col.id, "Mover", 0);
        let mover_id = mover.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(src_col).unwrap();
        tc.store.upsert_column(dst_col).unwrap();
        tc.store.upsert_card(existing).unwrap();
        tc.store.upsert_card(mover).unwrap();

        let context = tc.as_command_context();
        let cmd = MoveCard {
            card_id: mover_id,
            new_column_id: dst_id,
            new_position: 1,
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_wip_limit_exceeded());
    }

    #[test]
    fn test_compact_column_positions_makes_sequential() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let column_id = col.id;
        let mut card1 = crate::Card::new(&mut board, column_id, "C1", 0);
        card1.position = 0;
        let mut card2 = crate::Card::new(&mut board, column_id, "C2", 5);
        card2.position = 5;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_card(card1).unwrap();
        tc.store.upsert_card(card2).unwrap();

        let context = tc.as_command_context();
        let cmd = CompactColumnPositions { column_id };
        cmd.execute(&context).unwrap();

        let cards = tc.store.list_cards_by_column(column_id).unwrap();
        assert_eq!(cards[0].position, 0);
        assert_eq!(cards[1].position, 1);
    }
}
