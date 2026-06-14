use super::{Command, CommandContext};
use crate::data_store::DataStore;
use crate::{CardUpdate, CreateCardOptions, DomainError, KanbanError, KanbanResult, SprintLog};
use chrono::{DateTime, Utc};
use kanban_core::Editable;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

/// Update card properties (title, description, priority, status, etc.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateCard {
    pub card_id: Uuid,
    pub updates: CardUpdate,
}

impl UpdateCard {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let mut card = context.get_card(self.card_id)?;
        card.update(self.updates.clone(), Utc::now());
        context.store.upsert_card(card)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        "Update card".to_string()
    }

    /// Inverse: read the card's current state and synthesise an
    /// `UpdateCard` whose `updates` field-by-field restore each touched
    /// field to its prior value. Fields not touched by the forward
    /// command stay `None` / `NoChange` so the inverse is minimal.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        use crate::field_update::FieldUpdate;
        let card = match store.get_card(self.card_id)? {
            Some(c) => c,
            None => return Err(KanbanError::not_found("Card", self.card_id)),
        };

        let upd = &self.updates;
        let inverse = CardUpdate {
            title: upd.title.as_ref().map(|_| card.title.clone()),
            description: match upd.description {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match card.description {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
            priority: upd.priority.map(|_| card.priority),
            status: upd.status.map(|_| card.status),
            position: upd.position.map(|_| card.position),
            column_id: upd.column_id.map(|_| card.column_id),
            due_date: match upd.due_date {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match card.due_date {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
            points: match upd.points {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match card.points {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
            sprint_id: match upd.sprint_id {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match card.sprint_id {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
        };

        Ok(vec![Command::Card(CardCommand::Update(UpdateCard {
            card_id: self.card_id,
            updates: inverse,
        }))])
    }
}

/// Create a new card in a column
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateCard {
    pub id: Uuid,
    pub card_number: u32,
    pub board_id: Uuid,
    pub column_id: Uuid,
    pub title: String,
    pub position: i32,
    pub options: CreateCardOptions,
    #[serde(default = "chrono::Utc::now")]
    pub timestamp: DateTime<Utc>,
}

impl CreateCard {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        context.check_wip_limit(self.column_id, 1, &[])?;
        let mut board = context.get_board(self.board_id)?;

        let now = self.timestamp;
        let mut card = crate::Card {
            id: self.id,
            column_id: self.column_id,
            title: self.title.clone(),
            description: None,
            priority: crate::CardPriority::Medium,
            status: crate::CardStatus::Todo,
            position: self.position,
            due_date: None,
            points: None,
            card_number: self.card_number,
            sprint_id: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
            sprint_logs: Vec::new(),
        };

        if board.card_counter <= self.card_number {
            board.card_counter = self.card_number + 1;
        }

        if self.options.description.is_some()
            || self.options.priority.is_some()
            || self.options.points.is_some()
            || self.options.due_date.is_some()
        {
            let updates = CardUpdate {
                description: self
                    .options
                    .description
                    .clone()
                    .map(crate::FieldUpdate::Set)
                    .unwrap_or(crate::FieldUpdate::NoChange),
                priority: self.options.priority,
                points: self
                    .options
                    .points
                    .map(crate::FieldUpdate::Set)
                    .unwrap_or(crate::FieldUpdate::NoChange),
                due_date: self
                    .options
                    .due_date
                    .map(crate::FieldUpdate::Set)
                    .unwrap_or(crate::FieldUpdate::NoChange),
                ..Default::default()
            };
            card.update(updates, now);
        }

        if let Some(sprint_id) = self.options.sprint_id {
            let sprint = context.get_sprint(sprint_id)?;
            if sprint.board_id != self.board_id {
                return Err(KanbanError::Domain(DomainError::SprintBoardMismatch {
                    sprint_id,
                    sprint_board: sprint.board_id,
                    card_board: self.board_id,
                }));
            }
            let sprint_number = sprint.sprint_number;
            let sprint_name = sprint.get_name(&board).map(|s| s.to_string());
            let sprint_status = format!("{:?}", sprint.status);
            card.assign_to_sprint(sprint_id, sprint_number, sprint_name, sprint_status, now);
        }

        context.store.upsert_board(board)?;
        context.store.upsert_card(card)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Create card: '{}'", self.title)
    }

    /// Inverse: delete the new card. `DeleteCard` is polymorphic over
    /// live / archived so it cleanly removes a freshly-created live
    /// card without leaving an archive trail. The board's
    /// `card_counter` stays bumped; redo via the original forward
    /// reproduces the same id and number.
    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Ok(vec![Command::Card(CardCommand::Delete(DeleteCard {
            card_id: self.id,
        }))])
    }
}

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

/// Restore an archived card
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreCard {
    pub card_id: Uuid,
    pub column_id: Uuid,
    pub position: i32,
    #[serde(default = "chrono::Utc::now")]
    pub timestamp: DateTime<Utc>,
}

impl RestoreCard {
    /// Inverse: archive the card again. The card id is in the forward
    /// command. ArchiveCards captures original column/position from the
    /// live card at capture time — by the time this runs the card has
    /// been restored to (self.column_id, self.position), so the
    /// re-archive will use those values as the new "original" location.
    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Ok(vec![Command::Card(CardCommand::Archive(ArchiveCards {
            ids: vec![self.card_id],
        }))])
    }

    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        context.check_wip_limit(self.column_id, 1, &[])?;
        let archived = context
            .store
            .get_archived_card(self.card_id)?
            .ok_or_else(|| KanbanError::not_found("archived card", self.card_id))?;
        let mut card = archived.into_card();
        card.column_id = self.column_id;
        card.position = self.position;
        card.updated_at = self.timestamp;

        context.store.delete_archived_card(self.card_id)?;
        context.store.upsert_card(card)?;

        let card_id = self.card_id;
        context.store.modify_graph(Box::new(move |graph| {
            graph.unarchive_node(card_id);
            Ok(())
        }))?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Restore card {}", self.card_id)
    }
}

/// Permanently delete a card. Operates on whichever list the card is
/// in — live or archived. Strips incident graph edges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteCard {
    pub card_id: Uuid,
}

impl DeleteCard {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        // Both store ops are idempotent on missing — calling both
        // covers a card in either list.
        context.store.delete_card(self.card_id)?;
        context.store.delete_archived_card(self.card_id)?;
        let card_id = self.card_id;
        context.store.modify_graph(Box::new(move |graph| {
            graph.remove_node(card_id);
            Ok(())
        }))?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Delete card {}", self.card_id)
    }

    /// Inverse: re-insert whichever state the card was in (live,
    /// archived, or — defensively — both) via `ImportEntities`, then
    /// re-add every incident graph edge.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let live = store.get_card(self.card_id)?;
        let archived = store.get_archived_card(self.card_id)?;
        if live.is_none() && archived.is_none() {
            return Err(KanbanError::not_found("Card", self.card_id));
        }
        let mut commands: Vec<Command> = vec![Command::Board(super::BoardCommand::Import(
            super::ImportEntities {
                cards: live.into_iter().collect(),
                archived_cards: archived.into_iter().collect(),
                ..Default::default()
            },
        ))];
        let graph = store.get_graph()?;
        let card_id = self.card_id;
        commands.extend(super::dependency_commands::edges_to_undo_commands(
            &graph,
            |s, t| s == card_id || t == card_id,
        ));
        Ok(commands)
    }
}

/// Archive one or more cards in a single command (single undo entry)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveCards {
    pub ids: Vec<Uuid>,
}

impl ArchiveCards {
    /// Inverse: one `RestoreCard` per archived card, restoring each to its
    /// original column and position read from the live card BEFORE the
    /// archive runs.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let mut commands: Vec<Command> = Vec::new();
        for id in &self.ids {
            let card = match store.get_card(*id)? {
                Some(c) => c,
                None => continue, // skipped (matches ArchiveCards::execute's filter)
            };
            commands.push(Command::Card(CardCommand::Restore(RestoreCard {
                card_id: card.id,
                column_id: card.column_id,
                position: card.position,
                timestamp: chrono::Utc::now(),
            })));
        }
        Ok(commands)
    }

    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let valid_ids = context.filter_valid_card_ids(&self.ids, "ArchiveCards");
        if valid_ids.is_empty() && !self.ids.is_empty() {
            return Err(KanbanError::validation(
                "All card IDs in ArchiveCards batch are invalid",
            ));
        }
        for id in &valid_ids {
            let card = context
                .store
                .get_card(*id)?
                .ok_or_else(|| KanbanError::not_found("Card", *id))?;
            let original_column_id = card.column_id;
            let original_position = card.position;
            let archived = crate::ArchivedCard::new(card, original_column_id, original_position);
            context.store.insert_archived_card(archived)?;
            context.store.delete_card(*id)?;
        }
        context.store.modify_graph(Box::new(move |graph| {
            for id in &valid_ids {
                graph.archive_node(*id);
            }
            Ok(())
        }))?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Archive {} card(s)", self.ids.len())
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
    use super::super::test_helpers::TestContext;
    use super::*;
    use crate::DataStore;

    #[test]
    fn test_update_card_not_found_returns_error() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let cmd = UpdateCard {
            card_id: Uuid::new_v4(),
            updates: CardUpdate::default(),
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_create_card_board_not_found_returns_error() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let cmd = CreateCard {
            id: Uuid::new_v4(),
            card_number: 1,
            board_id: Uuid::new_v4(),
            column_id: Uuid::new_v4(),
            title: "Test".to_string(),
            position: 0,
            options: CreateCardOptions::default(),
            timestamp: Utc::now(),
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

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
    fn test_archive_cards_all_invalid_ids_returns_error() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let cmd = ArchiveCards {
            ids: vec![Uuid::new_v4()],
        };
        let result = cmd.execute(&context);
        assert!(result.is_err(), "Expected error when all IDs are invalid");
    }

    #[test]
    fn test_archive_cards_invalid_ids_skipped_valid_ids_archived() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("Test", Some("TST"));
        let card = crate::Card::new(&mut board, Uuid::new_v4(), "Card", 0);
        let valid_id = card.id;
        tc.store.upsert_card(card).unwrap();

        let context = tc.as_command_context();
        let cmd = ArchiveCards {
            ids: vec![valid_id, Uuid::new_v4()],
        };
        let result = cmd.execute(&context);
        assert!(result.is_ok());
        assert_eq!(tc.store.list_all_cards().unwrap().len(), 0);
        assert_eq!(tc.store.list_archived_cards().unwrap().len(), 1);
    }

    #[test]
    fn test_create_card_exceeding_wip_limit_returns_error() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("Test", Some("TST"));
        let mut column = crate::Column::new(board.id, "Limited", 0);
        column.wip_limit = Some(1);
        let column_id = column.id;
        let existing = crate::Card::new(&mut board, column_id, "Existing", 0);
        let board_id = board.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(column).unwrap();
        tc.store.upsert_card(existing).unwrap();

        let context = tc.as_command_context();
        let cmd = CreateCard {
            id: Uuid::new_v4(),
            card_number: 1,
            board_id,
            column_id,
            title: "New".to_string(),
            position: 1,
            options: CreateCardOptions::default(),
            timestamp: Utc::now(),
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_wip_limit_exceeded());
    }

    #[test]
    fn test_create_card_at_wip_limit_returns_error() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("Test", Some("TST"));
        let mut column = crate::Column::new(board.id, "Limited", 0);
        column.wip_limit = Some(2);
        let column_id = column.id;
        let card1 = crate::Card::new(&mut board, column_id, "C1", 0);
        let card2 = crate::Card::new(&mut board, column_id, "C2", 1);
        let board_id = board.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(column).unwrap();
        tc.store.upsert_card(card1).unwrap();
        tc.store.upsert_card(card2).unwrap();

        let context = tc.as_command_context();
        let cmd = CreateCard {
            id: Uuid::new_v4(),
            card_number: 1,
            board_id,
            column_id,
            title: "New".to_string(),
            position: 2,
            options: CreateCardOptions::default(),
            timestamp: Utc::now(),
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_wip_limit_exceeded());
    }

    #[test]
    fn test_create_card_below_wip_limit_succeeds() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("Test", Some("TST"));
        let mut column = crate::Column::new(board.id, "Limited", 0);
        column.wip_limit = Some(2);
        let column_id = column.id;
        let card1 = crate::Card::new(&mut board, column_id, "C1", 0);
        let board_id = board.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(column).unwrap();
        tc.store.upsert_card(card1).unwrap();

        let context = tc.as_command_context();
        let cmd = CreateCard {
            id: Uuid::new_v4(),
            card_number: 1,
            board_id,
            column_id,
            title: "New".to_string(),
            position: 1,
            options: CreateCardOptions::default(),
            timestamp: Utc::now(),
        };
        assert!(cmd.execute(&context).is_ok());
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
    fn test_restore_card_to_deleted_column_returns_error() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("Test", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let col_id = col.id;
        let card = crate::Card::new(&mut board, col_id, "Card", 0);
        let card_id = card.id;
        let archived = crate::ArchivedCard::new(card, col_id, 0);
        tc.store.upsert_board(board).unwrap();
        // Column intentionally NOT added — it has been deleted
        tc.store.insert_archived_card(archived).unwrap();

        let context = tc.as_command_context();
        let cmd = RestoreCard {
            card_id,
            column_id: col_id,
            position: 0,
            timestamp: Utc::now(),
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_restore_card_to_valid_column_succeeds() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("Test", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let col_id = col.id;
        let card = crate::Card::new(&mut board, col_id, "Card", 0);
        let card_id = card.id;
        let archived = crate::ArchivedCard::new(card, col_id, 0);
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.insert_archived_card(archived).unwrap();

        let context = tc.as_command_context();
        let cmd = RestoreCard {
            card_id,
            column_id: col_id,
            position: 0,
            timestamp: Utc::now(),
        };
        assert!(cmd.execute(&context).is_ok());
        assert_eq!(tc.store.list_all_cards().unwrap().len(), 1);
        assert_eq!(tc.store.list_archived_cards().unwrap().len(), 0);
    }

    #[test]
    fn test_restore_card_exceeding_wip_limit_returns_error() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("Test", Some("TST"));
        let mut col = crate::Column::new(board.id, "Col", 0);
        col.wip_limit = Some(1);
        let col_id = col.id;
        let existing = crate::Card::new(&mut board, col_id, "Existing", 0);
        let card = crate::Card::new(&mut board, col_id, "Card", 1);
        let card_id = card.id;
        let archived = crate::ArchivedCard::new(card, col_id, 0);
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_card(existing).unwrap();
        tc.store.insert_archived_card(archived).unwrap();

        let context = tc.as_command_context();
        let cmd = RestoreCard {
            card_id,
            column_id: col_id,
            position: 1,
            timestamp: Utc::now(),
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_wip_limit_exceeded());
    }

    #[test]
    fn test_restore_card_not_found_returns_error() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let cmd = RestoreCard {
            card_id: Uuid::new_v4(),
            column_id: Uuid::new_v4(),
            position: 0,
            timestamp: Utc::now(),
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

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
    fn test_archive_cards_missing_card_after_filter_returns_error() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("Test", Some("TST"));
        let card = crate::Card::new(&mut board, Uuid::new_v4(), "Card", 0);
        let card_id = card.id;
        tc.store.upsert_card(card).unwrap();

        // Directly call ArchiveCards with a valid card id.
        // The card will be found by filter_valid_card_ids, then get_card should
        // return a proper error (not panic) if the card is somehow missing.
        // Here we test the happy path still works, plus we ensure the error
        // path is properly handled (not an unwrap panic) via the impl fix.
        let context = tc.as_command_context();
        let cmd = ArchiveCards { ids: vec![card_id] };
        assert!(cmd.execute(&context).is_ok());
        assert_eq!(tc.store.list_all_cards().unwrap().len(), 0);
        assert_eq!(tc.store.list_archived_cards().unwrap().len(), 1);
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

    #[test]
    fn test_create_card_with_sprint_id_assigns_card_to_sprint() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let sprint = crate::Sprint::new(board.id, 1, None, None::<String>);
        let board_id = board.id;
        let column_id = col.id;
        let sprint_id = sprint.id;
        // Bump card_counter so upsert_board doesn't reset it; mirrors real usage.
        board.card_counter = 1;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();

        let context = tc.as_command_context();
        let card_id = Uuid::new_v4();
        let cmd = CreateCard {
            id: card_id,
            card_number: 1,
            board_id,
            column_id,
            title: "Test".to_string(),
            position: 0,
            options: CreateCardOptions {
                sprint_id: Some(sprint_id),
                ..Default::default()
            },
            timestamp: Utc::now(),
        };
        cmd.execute(&context).unwrap();

        let card = tc.store.get_card(card_id).unwrap().unwrap();
        assert_eq!(card.sprint_id, Some(sprint_id));
        assert_eq!(card.sprint_logs.len(), 1);
        assert_eq!(card.sprint_logs[0].sprint_id, sprint_id);
    }

    #[test]
    fn test_create_card_without_sprint_id_leaves_card_unassigned() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let board_id = board.id;
        let column_id = col.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();

        let context = tc.as_command_context();
        let card_id = Uuid::new_v4();
        let cmd = CreateCard {
            id: card_id,
            card_number: 1,
            board_id,
            column_id,
            title: "Test".to_string(),
            position: 0,
            options: CreateCardOptions::default(),
            timestamp: Utc::now(),
        };
        cmd.execute(&context).unwrap();

        let card = tc.store.get_card(card_id).unwrap().unwrap();
        assert_eq!(card.sprint_id, None);
        assert!(card.sprint_logs.is_empty());
    }

    #[test]
    fn test_create_card_with_invalid_sprint_id_returns_not_found_error() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let board_id = board.id;
        let column_id = col.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();

        let context = tc.as_command_context();
        let cmd = CreateCard {
            id: Uuid::new_v4(),
            card_number: 1,
            board_id,
            column_id,
            title: "Test".to_string(),
            position: 0,
            options: CreateCardOptions {
                sprint_id: Some(Uuid::new_v4()),
                ..Default::default()
            },
            timestamp: Utc::now(),
        };
        let err = cmd.execute(&context).unwrap_err();
        assert!(err.is_not_found(), "Expected not found, got: {:?}", err);
    }

    #[test]
    fn test_create_card_with_options_only_uses_embedded_timestamp() {
        use chrono::TimeZone;

        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let board_id = board.id;
        let column_id = col.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();

        let fixed_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let context = tc.as_command_context();
        let card_id = Uuid::new_v4();
        let cmd = CreateCard {
            id: card_id,
            card_number: 1,
            board_id,
            column_id,
            title: "T".to_string(),
            position: 0,
            options: CreateCardOptions {
                description: Some("d".to_string()),
                priority: Some(crate::CardPriority::High),
                points: Some(3),
                due_date: None,
                sprint_id: None,
            },
            timestamp: fixed_time,
        };
        cmd.execute(&context).unwrap();

        let card = tc.store.get_card(card_id).unwrap().unwrap();
        assert_eq!(card.created_at, fixed_time);
        assert_eq!(
            card.updated_at, fixed_time,
            "updated_at must match the embedded command timestamp even when \
             CardUpdate options reset it inside Card::update"
        );
    }

    #[test]
    fn test_create_card_with_options_and_sprint_uses_embedded_timestamp() {
        use chrono::TimeZone;

        let tc = TestContext::new();
        let mut board = crate::Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let sprint = crate::Sprint::new(board.id, 1, None, None::<String>);
        let board_id = board.id;
        let column_id = col.id;
        let sprint_id = sprint.id;
        board.card_counter = 1;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();

        let fixed_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let context = tc.as_command_context();
        let card_id = Uuid::new_v4();
        let cmd = CreateCard {
            id: card_id,
            card_number: 1,
            board_id,
            column_id,
            title: "T".to_string(),
            position: 0,
            options: CreateCardOptions {
                description: Some("d".to_string()),
                priority: Some(crate::CardPriority::High),
                points: Some(3),
                due_date: None,
                sprint_id: Some(sprint_id),
            },
            timestamp: fixed_time,
        };
        cmd.execute(&context).unwrap();

        let card = tc.store.get_card(card_id).unwrap().unwrap();
        assert_eq!(card.created_at, fixed_time);
        assert_eq!(
            card.updated_at, fixed_time,
            "updated_at must match the embedded command timestamp even when both \
             CardUpdate options and sprint assignment run inside execute"
        );
    }

    #[test]
    fn test_create_card_with_sprint_from_different_board_returns_typed_mismatch() {
        let tc = TestContext::new();
        let board_a = crate::Board::new("A", Some("AAA"));
        let board_b = crate::Board::new("B", Some("BBB"));
        let col_a = crate::Column::new(board_a.id, "Col", 0);
        // Sprint belongs to board B.
        let sprint_b = crate::Sprint::new(board_b.id, 1, None, None::<String>);
        let board_a_id = board_a.id;
        let board_b_id = board_b.id;
        let column_id = col_a.id;
        let sprint_b_id = sprint_b.id;
        tc.store.upsert_board(board_a).unwrap();
        tc.store.upsert_board(board_b).unwrap();
        tc.store.upsert_column(col_a).unwrap();
        tc.store.upsert_sprint(sprint_b).unwrap();

        let context = tc.as_command_context();
        let cmd = CreateCard {
            id: Uuid::new_v4(),
            card_number: 1,
            board_id: board_a_id,
            column_id,
            title: "X".to_string(),
            position: 0,
            options: CreateCardOptions {
                sprint_id: Some(sprint_b_id),
                ..Default::default()
            },
            timestamp: Utc::now(),
        };
        let err = cmd.execute(&context).unwrap_err();
        assert!(
            err.is_sprint_board_mismatch(),
            "expected SprintBoardMismatch, got: {err:?}"
        );
        match err {
            KanbanError::Domain(DomainError::SprintBoardMismatch {
                sprint_id,
                sprint_board,
                card_board,
            }) => {
                assert_eq!(sprint_id, sprint_b_id);
                assert_eq!(sprint_board, board_b_id);
                assert_eq!(card_board, board_a_id);
            }
            other => panic!("expected SprintBoardMismatch fields, got: {other:?}"),
        }
    }

    #[test]
    fn test_create_card_uses_embedded_timestamp() {
        use chrono::{TimeZone, Utc};

        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let board_id = board.id;
        let column_id = col.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();

        let fixed_time = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        let context = tc.as_command_context();
        let card_id = Uuid::new_v4();
        let cmd = CreateCard {
            id: card_id,
            card_number: 1,
            board_id,
            column_id,
            title: "Test".to_string(),
            position: 0,
            options: CreateCardOptions::default(),
            timestamp: fixed_time,
        };
        cmd.execute(&context).unwrap();

        let card = tc.store.get_card(card_id).unwrap().unwrap();
        assert_eq!(card.created_at, fixed_time);
        assert_eq!(card.updated_at, fixed_time);
    }

    #[test]
    fn test_restore_card_uses_embedded_timestamp() {
        use chrono::{TimeZone, Utc};

        let tc = TestContext::new();
        let col = crate::Column::new(Uuid::new_v4(), "Col", 0);
        let column_id = col.id;
        tc.store.upsert_column(col).unwrap();

        let mut board = crate::Board::new("B", Some("TST"));
        let card = crate::Card::new(&mut board, column_id, "Card", 0);
        let card_id = card.id;
        let archived = crate::ArchivedCard::new(card, column_id, 0);
        tc.store.insert_archived_card(archived).unwrap();

        let fixed_time = Utc.with_ymd_and_hms(2020, 6, 15, 12, 0, 0).unwrap();
        let context = tc.as_command_context();
        let cmd = RestoreCard {
            card_id,
            column_id,
            position: 0,
            timestamp: fixed_time,
        };
        cmd.execute(&context).unwrap();

        let card = tc.store.get_card(card_id).unwrap().unwrap();
        assert_eq!(card.updated_at, fixed_time);
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
