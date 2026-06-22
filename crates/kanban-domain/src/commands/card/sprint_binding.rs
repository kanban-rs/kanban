use super::super::{Command, CommandContext};
use super::CardCommand;
use crate::data_store::DataStore;
use crate::{KanbanError, KanbanResult, SprintLog};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Restore a card's `sprint_id`, `sprint_logs`, and `updated_at` to a
/// captured pre-state. Emitted by `AssignCardsToSprint` and
/// `UnassignCardFromSprint` inverses to round-trip the sprint-history
/// log cleanly — otherwise the inverse would push a new log entry
/// instead of removing the one the forward added.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreCardSprintAttachment {
    pub card_id: Uuid,
    pub sprint_id: Option<Uuid>,
    pub sprint_logs: Vec<SprintLog>,
    pub updated_at: DateTime<Utc>,
}

impl RestoreCardSprintAttachment {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let mut card = context.get_card(self.card_id)?;
        card.sprint_id = self.sprint_id;
        card.sprint_logs = self.sprint_logs.clone();
        card.updated_at = self.updated_at;
        context.store.upsert_card(card)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Restore sprint attachment for card {}", self.card_id)
    }

    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Err(KanbanError::Internal(format!(
            "RestoreCardSprintAttachment is a synthetic command — it must only appear inside an inverse batch (Assign/Unassign undo), never as a top-level forward command. Card id: {}",
            self.card_id
        )))
    }
}

/// Assign one or more cards to a sprint in a single command (single undo entry)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssignCardsToSprint {
    pub ids: Vec<Uuid>,
    pub sprint_id: Uuid,
}

impl AssignCardsToSprint {
    /// Inverse: per-card restore of pre-state — `sprint_id` AND
    /// `sprint_logs`. Using `RestoreSprintAttachment` instead of
    /// re-emitting Assign/Unassign avoids pushing new log entries that
    /// would bloat the card's sprint history on every undo/redo cycle.
    /// Cards skipped by the forward (not found, or already on the
    /// target sprint) are also skipped here.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let mut commands: Vec<Command> = Vec::new();
        for id in &self.ids {
            let card = match store.get_card(*id)? {
                Some(c) => c,
                None => continue,
            };
            if card.sprint_id == Some(self.sprint_id) {
                continue;
            }
            commands.push(Command::Card(CardCommand::RestoreSprintAttachment(
                RestoreCardSprintAttachment {
                    card_id: card.id,
                    sprint_id: card.sprint_id,
                    sprint_logs: card.sprint_logs.clone(),
                    updated_at: card.updated_at,
                },
            )));
        }
        Ok(commands)
    }

    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let sprint = context.get_sprint(self.sprint_id)?;
        let board = context.get_board(sprint.board_id)?;
        let sprint_number = sprint.sprint_number;
        let sprint_name = sprint.get_name(&board).map(|s| s.to_string());
        let sprint_status = format!("{:?}", sprint.status);

        let valid_ids = context.filter_valid_card_ids(&self.ids, "AssignCardsToSprint");
        let now = Utc::now();
        for id in &valid_ids {
            let mut card = context.get_card(*id)?;
            if let Some(old_sprint_id) = card.sprint_id {
                if old_sprint_id != self.sprint_id {
                    card.end_current_sprint_log();
                }
            }
            card.assign_to_sprint(
                self.sprint_id,
                sprint_number,
                sprint_name.clone(),
                sprint_status.clone(),
                now,
            );
            context.store.upsert_card(card)?;
        }
        Ok(())
    }

    pub fn description(&self) -> String {
        format!(
            "Assign {} card(s) to sprint {}",
            self.ids.len(),
            self.sprint_id
        )
    }
}

/// Unassign card from current sprint
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnassignCardFromSprint {
    pub card_id: Uuid,
    #[serde(default = "chrono::Utc::now")]
    pub timestamp: DateTime<Utc>,
}

impl UnassignCardFromSprint {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let mut card = context.get_card(self.card_id)?;
        card.end_current_sprint_log();
        card.sprint_id = None;
        card.updated_at = self.timestamp;
        context.store.upsert_card(card)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Unassign card {} from sprint", self.card_id)
    }

    /// Inverse: if the card currently has a sprint, re-assign it to that
    /// sprint via AssignCardsToSprint. The sprint log gets a fresh
    /// Inverse: restore `sprint_id`, `sprint_logs`, and `updated_at` to
    /// their pre-execute values via `RestoreSprintAttachment`. The card
    /// is captured before the forward closes the current sprint log,
    /// so the restored log vec is intact (no phantom closing entry).
    /// If the card had no sprint to begin with, the forward is a no-op
    /// and the inverse is empty.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let card = match store.get_card(self.card_id)? {
            Some(c) => c,
            None => return Err(KanbanError::not_found("Card", self.card_id)),
        };
        if card.sprint_id.is_none() {
            return Ok(vec![]);
        }
        Ok(vec![Command::Card(CardCommand::RestoreSprintAttachment(
            RestoreCardSprintAttachment {
                card_id: self.card_id,
                sprint_id: card.sprint_id,
                sprint_logs: card.sprint_logs.clone(),
                updated_at: card.updated_at,
            },
        ))])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_helpers::TestContext;

    #[test]
    fn test_assign_cards_to_sprint_validates_sprint_exists() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("Test", Some("TST"));
        let card = crate::Card::new(&mut board, Uuid::new_v4(), "Card", 0);
        let card_id = card.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_card(card).unwrap();

        let context = tc.as_command_context();
        let cmd = AssignCardsToSprint {
            ids: vec![card_id],
            sprint_id: Uuid::new_v4(),
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_assign_cards_to_sprint_invalid_ids_skipped_valid_ids_assigned() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("Test", Some("TST"));
        let card = crate::Card::new(&mut board, Uuid::new_v4(), "Card", 0);
        let valid_id = card.id;
        let sprint = crate::Sprint::new(board.id, 1, None, Some("Sprint"));
        let sprint_id = sprint.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_card(card).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();

        let context = tc.as_command_context();
        let cmd = AssignCardsToSprint {
            ids: vec![valid_id, Uuid::new_v4()],
            sprint_id,
        };
        let result = cmd.execute(&context);
        assert!(result.is_ok());
        let card = tc.store.get_card(valid_id).unwrap().unwrap();
        assert_eq!(card.sprint_id, Some(sprint_id));
    }

    #[test]
    fn test_unassign_card_from_sprint_not_found_returns_error() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let cmd = UnassignCardFromSprint {
            card_id: Uuid::new_v4(),
            timestamp: Utc::now(),
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_unassign_card_from_sprint_uses_embedded_timestamp() {
        use chrono::{TimeZone, Utc};

        let tc = TestContext::new();
        let mut board = crate::Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let mut card = crate::Card::new(&mut board, col.id, "Card", 0);
        let card_id = card.id;
        card.sprint_id = Some(Uuid::new_v4());
        tc.store.upsert_card(card).unwrap();

        let fixed_time = Utc.with_ymd_and_hms(2020, 3, 10, 8, 0, 0).unwrap();
        let context = tc.as_command_context();
        let cmd = UnassignCardFromSprint {
            card_id,
            timestamp: fixed_time,
        };
        cmd.execute(&context).unwrap();

        let card = tc.store.get_card(card_id).unwrap().unwrap();
        assert_eq!(card.updated_at, fixed_time);
    }
}
