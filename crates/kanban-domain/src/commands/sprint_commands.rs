use super::{Command, CommandContext};
use crate::data_store::DataStore;
use crate::SprintUpdate;
use crate::{KanbanError, KanbanResult, NewSprint};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SprintCommand {
    Create(CreateSprint),
    Update(UpdateSprint),
    Activate(ActivateSprint),
    Complete(CompleteSprint),
    Cancel(CancelSprint),
    Delete(DeleteSprint),
}

impl SprintCommand {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        match self {
            SprintCommand::Create(c) => c.execute(context),
            SprintCommand::Update(c) => c.execute(context),
            SprintCommand::Activate(c) => c.execute(context),
            SprintCommand::Complete(c) => c.execute(context),
            SprintCommand::Cancel(c) => c.execute(context),
            SprintCommand::Delete(c) => c.execute(context),
        }
    }

    pub fn description(&self) -> String {
        match self {
            SprintCommand::Create(c) => c.description(),
            SprintCommand::Update(c) => c.description(),
            SprintCommand::Activate(c) => c.description(),
            SprintCommand::Complete(c) => c.description(),
            SprintCommand::Cancel(c) => c.description(),
            SprintCommand::Delete(c) => c.description(),
        }
    }

    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        match self {
            SprintCommand::Activate(c) => c.capture_inverse(store),
            SprintCommand::Complete(c) => c.capture_inverse(store),
            SprintCommand::Cancel(c) => c.capture_inverse(store),
            SprintCommand::Update(c) => c.capture_inverse(store),
            SprintCommand::Create(c) => c.capture_inverse(store),
            SprintCommand::Delete(c) => c.capture_inverse(store),
        }
    }
}

/// Update sprint properties (name_index, prefix, card_prefix, status, dates)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateSprint {
    pub sprint_id: Uuid,
    pub updates: SprintUpdate,
}

impl UpdateSprint {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let mut updates = self.updates.clone();

        if !matches!(updates.card_prefix, crate::FieldUpdate::NoChange) {
            let sprint = context.get_sprint(self.sprint_id)?;
            validate_card_prefix_not_locked(self.sprint_id, context)?;
            if let crate::FieldUpdate::Set(ref new_prefix) = updates.card_prefix {
                validate_card_prefix_unique(new_prefix, self.sprint_id, sprint.board_id, context)?;
            }
        }

        if let Some(ref name) = updates.name {
            allocate_sprint_name(name.clone(), self.sprint_id, context, &mut updates)?;
        }

        let mut sprint = context.get_sprint(self.sprint_id)?;
        sprint.update(updates);
        context.store.upsert_sprint(sprint)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        "Update sprint".to_string()
    }

    /// Inverse: read the current Sprint (and Board if the forward
    /// touches `name`) and build a `SprintUpdate` whose fields reverse
    /// every touched field of the forward update.
    ///
    /// When the forward command sets `name`, the board's `sprint_names`
    /// pool and `sprint_name_used_count` are mutated as a side effect.
    /// We capture the pre-state of both and emit a multi-command inverse
    /// that restores the board's pool first (via the synthetic
    /// `RestoreSprintPool` command) and then restores the sprint's
    /// `name_index`.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        use crate::field_update::FieldUpdate;
        let sprint = match store.get_sprint(self.sprint_id)? {
            Some(s) => s,
            None => return Err(KanbanError::not_found("Sprint", self.sprint_id)),
        };

        // If the forward sets `name`, capture board pool pre-state too.
        let board_restore: Option<Command> = if self.updates.name.is_some() {
            let board = match store.get_board(sprint.board_id)? {
                Some(b) => b,
                None => return Err(KanbanError::not_found("Board", sprint.board_id)),
            };
            Some(Command::Board(super::BoardCommand::RestoreSprintPool(
                super::RestoreSprintPool {
                    board_id: board.id,
                    sprint_names: board.sprint_names.clone(),
                    sprint_name_used_count: board.sprint_name_used_count,
                },
            )))
        } else {
            None
        };

        let upd = &self.updates;
        let inverse = SprintUpdate {
            // The forward `name` allocation only mutates name_index on
            // the sprint. The board-side restore above handles the pool;
            // we restore the sprint's prior name_index here.
            name: None,
            // When forward.name is set, allocate_sprint_name mutates
            // name_index. When forward.name_index is set, it's mutated
            // directly. Either way, the inverse needs to restore the
            // prior name_index from the captured sprint snapshot.
            name_index: if upd.name.is_some()
                || matches!(upd.name_index, FieldUpdate::Set(_) | FieldUpdate::Clear)
            {
                match sprint.name_index {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                }
            } else {
                FieldUpdate::NoChange
            },
            prefix: match upd.prefix {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match sprint.prefix.clone() {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
            card_prefix: match upd.card_prefix {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match sprint.card_prefix.clone() {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
            status: upd.status.map(|_| sprint.status),
            start_date: match upd.start_date {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match sprint.start_date {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
            end_date: match upd.end_date {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match sprint.end_date {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
        };
        let sprint_restore = Command::Sprint(SprintCommand::Update(UpdateSprint {
            sprint_id: self.sprint_id,
            updates: inverse,
        }));

        // Order matters: restore the board pool first so any
        // dependencies on the name pool index see the right state, then
        // restore the sprint's name_index.
        let mut commands = Vec::new();
        if let Some(board_cmd) = board_restore {
            commands.push(board_cmd);
        }
        commands.push(sprint_restore);
        Ok(commands)
    }
}

fn validate_card_prefix_not_locked(sprint_id: Uuid, context: &CommandContext) -> KanbanResult<()> {
    let has_active = !context.store.list_cards_by_sprint(sprint_id)?.is_empty();
    let has_archived = context
        .store
        .list_archived_cards()?
        .iter()
        .any(|ac| ac.card.sprint_id == Some(sprint_id));
    if has_active || has_archived {
        return Err(KanbanError::validation(
            "sprint card_prefix cannot be changed after cards have been assigned",
        ));
    }
    Ok(())
}

fn validate_card_prefix_unique(
    new_prefix: &str,
    sprint_id: Uuid,
    board_id: Uuid,
    context: &CommandContext,
) -> KanbanResult<()> {
    let new_prefix_lower = new_prefix.to_lowercase();
    let board = context.get_board(board_id)?;

    if board
        .card_prefix
        .as_deref()
        .map(|p| p.to_lowercase())
        .as_deref()
        == Some(new_prefix_lower.as_str())
    {
        return Err(KanbanError::validation(
            "sprint card_prefix must not match the board card_prefix",
        ));
    }

    let sibling_collision = context
        .store
        .list_sprints_by_board(board_id)?
        .iter()
        .filter(|s| s.id != sprint_id)
        .any(|s| {
            s.card_prefix
                .as_deref()
                .map(|p| p.to_lowercase())
                .as_deref()
                == Some(new_prefix_lower.as_str())
        });
    if sibling_collision {
        return Err(KanbanError::validation(
            "sprint card_prefix must be unique within the board",
        ));
    }
    Ok(())
}

fn allocate_sprint_name(
    name: String,
    sprint_id: Uuid,
    context: &CommandContext,
    updates: &mut SprintUpdate,
) -> KanbanResult<()> {
    let sprint = context.get_sprint(sprint_id)?;
    let mut board = context.get_board(sprint.board_id)?;
    let idx = board.add_sprint_name_at_used_index(name);
    updates.name_index = crate::FieldUpdate::Set(idx);
    context.store.upsert_board(board)?;
    Ok(())
}

/// Create a new sprint.
///
/// Handles sprint counter initialization, number generation, and name assignment
/// internally. The effective prefix is resolved as:
///   `explicit_prefix` > `board.sprint_prefix` > `default_sprint_prefix`
///
/// If `auto_consume_name` is true and no explicit name is provided, the next
/// available sprint name from the board's name pool will be consumed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateSprint {
    pub id: Uuid,
    pub board_id: Uuid,
    pub name: Option<String>,
    pub default_sprint_prefix: String,
    /// If set, overrides both board prefix and default prefix.
    pub explicit_prefix: Option<String>,
    /// If true and `name` is None, consume next name from the board's name pool.
    /// Used by TUI; CLI/MCP pass false.
    pub auto_consume_name: bool,
}

impl CreateSprint {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let sprints_snapshot = context.store.list_sprints_by_board(self.board_id)?;

        let mut board = context.get_board(self.board_id)?;
        let effective_prefix = self
            .explicit_prefix
            .clone()
            .or_else(|| board.sprint_prefix.clone())
            .unwrap_or_else(|| self.default_sprint_prefix.clone());

        board.ensure_sprint_counter_initialized(&effective_prefix, &sprints_snapshot);
        let sprint_number = board.get_next_sprint_number(&effective_prefix);
        let name_index = match &self.name {
            Some(name) if !name.trim().is_empty() => {
                Some(board.add_sprint_name_at_used_index(name.clone()))
            }
            _ if self.auto_consume_name => board.consume_sprint_name(),
            _ => None,
        };

        // Funnel construction through the factory (no `Sprint::new` + post-patch
        // `sprint.id = ..`). The DUAL minting above stays here — this layer owns
        // the Board + store — and is carried into the spec; `Sprint::create`
        // stays Board-free and I/O-free. Board upsert (counters/pool) is ordered
        // before the sprint upsert, preserving today's persistence ordering and
        // keeping `capture_inverse` valid (delete the sprint, leave counters
        // bumped, redo reproduces the same id).
        let spec = NewSprint {
            board_id: self.board_id,
            sprint_number,
            name_index,
            prefix: Some(effective_prefix),
            card_prefix: None,
        };
        let sprint = crate::Sprint::create(spec, self.id, Utc::now())?;
        context.store.upsert_board(board)?;
        context.store.upsert_sprint(sprint)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Create sprint for board {}", self.board_id)
    }

    /// Inverse: delete the newly-created sprint. The board's
    /// sprint_counter and sprint_name_used_count stay bumped — display
    /// numbering drifts for *future* sprints only, redo of this one
    /// reproduces the same sprint id.
    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Ok(vec![Command::Sprint(SprintCommand::Delete(DeleteSprint {
            sprint_id: self.id,
            timestamp: chrono::Utc::now(),
        }))])
    }
}

/// Activate a sprint (change status to Active and set dates)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivateSprint {
    pub sprint_id: Uuid,
    pub duration_days: u32,
}

impl ActivateSprint {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let mut sprint = context.get_sprint(self.sprint_id)?;
        sprint.activate(self.duration_days);
        context.store.upsert_sprint(sprint)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Activate sprint {}", self.sprint_id)
    }

    /// Inverse: restore the sprint's prior status, start_date, end_date.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        capture_status_revert(store, self.sprint_id)
    }
}

/// Complete a sprint (change status to Completed)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompleteSprint {
    pub sprint_id: Uuid,
}

impl CompleteSprint {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let mut sprint = context.get_sprint(self.sprint_id)?;
        sprint.complete();
        context.store.upsert_sprint(sprint)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Complete sprint {}", self.sprint_id)
    }

    /// Inverse: restore the sprint's prior status.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        capture_status_revert(store, self.sprint_id)
    }
}

/// Cancel a sprint (change status to Cancelled)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CancelSprint {
    pub sprint_id: Uuid,
}

impl CancelSprint {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let mut sprint = context.get_sprint(self.sprint_id)?;
        sprint.cancel();
        context.store.upsert_sprint(sprint)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Cancel sprint {}", self.sprint_id)
    }

    /// Inverse: restore the sprint's prior status.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        capture_status_revert(store, self.sprint_id)
    }
}

/// Shared helper: build an UpdateSprint that restores the prior `status`,
/// `start_date`, and `end_date` of `sprint_id`. Used by the inverses of
/// Activate / Complete / Cancel, which all mutate exactly these fields.
fn capture_status_revert(store: &dyn DataStore, sprint_id: Uuid) -> KanbanResult<Vec<Command>> {
    use crate::field_update::FieldUpdate;
    let sprint = match store.get_sprint(sprint_id)? {
        Some(s) => s,
        None => return Err(KanbanError::not_found("Sprint", sprint_id)),
    };
    Ok(vec![Command::Sprint(SprintCommand::Update(UpdateSprint {
        sprint_id,
        updates: SprintUpdate {
            status: Some(sprint.status),
            start_date: match sprint.start_date {
                Some(v) => FieldUpdate::Set(v),
                None => FieldUpdate::Clear,
            },
            end_date: match sprint.end_date {
                Some(v) => FieldUpdate::Set(v),
                None => FieldUpdate::Clear,
            },
            ..Default::default()
        },
    }))])
}

/// Delete a sprint
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteSprint {
    pub sprint_id: Uuid,
    #[serde(default = "chrono::Utc::now")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl DeleteSprint {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        context
            .store
            .clear_sprint_from_cards(self.sprint_id, self.timestamp)?;
        context
            .store
            .clear_sprint_from_archived_cards(self.sprint_id, self.timestamp)?;
        context.store.delete_sprint(self.sprint_id)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Delete sprint {}", self.sprint_id)
    }

    /// Inverse: capture the Sprint, every live card assigned to it, and
    /// every archived card assigned to it. On undo:
    ///
    /// 1. Re-insert the Sprint via `ImportEntities`.
    /// 2. Re-assign live cards via `AssignCardsToSprint`.
    /// 3. Re-attach the sprint binding to archived cards via the
    ///    internal `SetArchivedCardsSprint` cascade primitive (there's
    ///    no other command that sets `sprint_id` on an archived card).
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let sprint = match store.get_sprint(self.sprint_id)? {
            Some(s) => s,
            None => return Err(KanbanError::not_found("Sprint", self.sprint_id)),
        };
        let assigned_card_ids: Vec<Uuid> = store
            .list_cards_by_sprint(self.sprint_id)?
            .into_iter()
            .map(|c| c.id)
            .collect();
        let archived_with_sprint: Vec<Uuid> = store
            .list_archived_cards()?
            .into_iter()
            .filter(|ac| ac.card.sprint_id == Some(self.sprint_id))
            .map(|ac| ac.card.id)
            .collect();

        let mut commands: Vec<Command> = vec![Command::Board(super::BoardCommand::Import(
            super::ImportEntities {
                sprints: vec![sprint],
                ..Default::default()
            },
        ))];
        if !assigned_card_ids.is_empty() {
            commands.push(Command::Card(super::CardCommand::AssignToSprint(
                super::AssignCardsToSprint {
                    ids: assigned_card_ids,
                    sprint_id: self.sprint_id,
                },
            )));
        }
        if !archived_with_sprint.is_empty() {
            commands.push(Command::Cascade(
                super::CascadeCommand::SetArchivedCardsSprint(super::SetArchivedCardsSprint {
                    archived_card_ids: archived_with_sprint,
                    sprint_id: self.sprint_id,
                }),
            ));
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
    fn test_update_sprint_not_found_returns_error() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let cmd = UpdateSprint {
            sprint_id: Uuid::new_v4(),
            updates: SprintUpdate::default(),
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_update_sprint_name_with_nonexistent_board_returns_error() {
        let tc = TestContext::new();
        let nonexistent_board_id = Uuid::new_v4();
        let sprint = crate::Sprint::new(nonexistent_board_id, 1, None, None::<String>);
        let sprint_id = sprint.id;
        tc.store.upsert_sprint(sprint).unwrap();

        let context = tc.as_command_context();
        let cmd = UpdateSprint {
            sprint_id,
            updates: SprintUpdate {
                name: Some("New Name".to_string()),
                ..Default::default()
            },
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_activate_sprint_not_found_returns_error() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let cmd = ActivateSprint {
            sprint_id: Uuid::new_v4(),
            duration_days: 14,
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_complete_sprint_not_found_returns_error() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let cmd = CompleteSprint {
            sprint_id: Uuid::new_v4(),
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_cancel_sprint_not_found_returns_error() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let cmd = CancelSprint {
            sprint_id: Uuid::new_v4(),
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_create_sprint_command_funnels_through_factory_with_injected_id() {
        let tc = TestContext::new();
        let board = crate::Board::new("Test", None::<String>);
        let board_id = board.id;
        tc.store.upsert_board(board).unwrap();

        let context = tc.as_command_context();
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let cmd = CreateSprint {
            id,
            board_id,
            name: None,
            default_sprint_prefix: "Sprint".to_string(),
            explicit_prefix: Some("SPR".to_string()),
            auto_consume_name: false,
        };
        cmd.execute(&context).unwrap();

        let sprint = tc.store.get_sprint(id).unwrap().unwrap();
        // Injected id carried verbatim, server values minted from the board:
        assert_eq!(sprint.id, id);
        assert_eq!(sprint.sprint_number, 1);
        assert_eq!(sprint.prefix, Some("SPR".to_string()));
        // Factory-seeded lifecycle defaults (Sprint::create):
        assert_eq!(sprint.status, crate::SprintStatus::Planning);
        assert_eq!(sprint.start_date, None);
        assert_eq!(sprint.end_date, None);
        assert_eq!(
            sprint.created_at, sprint.updated_at,
            "no observable intermediate update — one Sprint::create call"
        );
    }

    #[test]
    fn test_create_sprint_auto_consume_name_uses_name_pool() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("Test", None::<String>);
        board.sprint_names = vec!["Alpha".to_string(), "Beta".to_string()];
        let board_id = board.id;
        tc.store.upsert_board(board).unwrap();

        let context = tc.as_command_context();
        let cmd = CreateSprint {
            id: Uuid::new_v4(),
            board_id,
            name: None,
            default_sprint_prefix: "Sprint".to_string(),
            explicit_prefix: None,
            auto_consume_name: true,
        };
        cmd.execute(&context).unwrap();

        let sprints = tc.store.list_all_sprints().unwrap();
        assert_eq!(sprints.len(), 1);
        let sprint = &sprints[0];
        let board = tc.store.get_board(board_id).unwrap().unwrap();
        assert_eq!(
            sprint.get_name(&board),
            Some("Alpha"),
            "auto_consume_name should consume the first available sprint name"
        );
    }

    #[test]
    fn test_update_sprint_card_prefix_locked_after_card_assigned_returns_validation_error() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("B", Some("KAN"));
        let col = crate::Column::new(board.id, "Col", 0);
        let sprint = crate::Sprint::new(board.id, 1, None, Some("SPR"));
        let sprint_id = sprint.id;
        let mut card = crate::Card::new(&mut board, col.id, "C", 0);
        card.sprint_id = Some(sprint_id);
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();
        tc.store.upsert_card(card).unwrap();

        let context = tc.as_command_context();
        let cmd = UpdateSprint {
            sprint_id,
            updates: crate::SprintUpdate {
                card_prefix: crate::FieldUpdate::Set("NEW".to_string()),
                ..Default::default()
            },
        };
        let err = cmd.execute(&context).unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_update_sprint_card_prefix_locked_after_archived_card_assigned_returns_validation_error()
    {
        let tc = TestContext::new();
        let mut board = crate::Board::new("B", Some("KAN"));
        let col = crate::Column::new(board.id, "Col", 0);
        let sprint = crate::Sprint::new(board.id, 1, None, Some("SPR"));
        let sprint_id = sprint.id;
        let mut card = crate::Card::new(&mut board, col.id, "C", 0);
        card.sprint_id = Some(sprint_id);
        let archived = crate::ArchivedCard::new(card, uuid::Uuid::nil(), col.id, 0);
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();
        tc.store.insert_archived_card(archived).unwrap();

        let context = tc.as_command_context();
        let cmd = UpdateSprint {
            sprint_id,
            updates: crate::SprintUpdate {
                card_prefix: crate::FieldUpdate::Set("NEW".to_string()),
                ..Default::default()
            },
        };
        let err = cmd.execute(&context).unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_update_sprint_clear_card_prefix_locked_after_card_assigned_returns_validation_error() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("B", Some("KAN"));
        let col = crate::Column::new(board.id, "Col", 0);
        let sprint = crate::Sprint::new(board.id, 1, None, Some("SPR"));
        let sprint_id = sprint.id;
        let mut card = crate::Card::new(&mut board, col.id, "C", 0);
        card.sprint_id = Some(sprint_id);
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();
        tc.store.upsert_card(card).unwrap();

        let context = tc.as_command_context();
        let cmd = UpdateSprint {
            sprint_id,
            updates: crate::SprintUpdate {
                card_prefix: crate::FieldUpdate::Clear,
                ..Default::default()
            },
        };
        let err = cmd.execute(&context).unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_update_sprint_card_prefix_collides_with_board_prefix_returns_validation_error() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("KAN"));
        let board_id = board.id;
        let sprint = crate::Sprint::new(board_id, 1, None, Some("SPR"));
        let sprint_id = sprint.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();

        let context = tc.as_command_context();
        let cmd = UpdateSprint {
            sprint_id,
            updates: crate::SprintUpdate {
                card_prefix: crate::FieldUpdate::Set("KAN".to_string()),
                ..Default::default()
            },
        };
        let err = cmd.execute(&context).unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_update_sprint_card_prefix_case_insensitive_collision_returns_validation_error() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("KAN"));
        let board_id = board.id;
        let sprint = crate::Sprint::new(board_id, 1, None, Some("SPR"));
        let sprint_id = sprint.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();

        let context = tc.as_command_context();
        let cmd = UpdateSprint {
            sprint_id,
            updates: crate::SprintUpdate {
                card_prefix: crate::FieldUpdate::Set("kan".to_string()),
                ..Default::default()
            },
        };
        let err = cmd.execute(&context).unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_update_sprint_card_prefix_collides_with_sibling_sprint_returns_validation_error() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("KAN"));
        let board_id = board.id;
        let mut sprint1 = crate::Sprint::new(board_id, 1, None, None::<String>);
        sprint1.card_prefix = Some("SPR".to_string());
        let sprint2 = crate::Sprint::new(board_id, 2, None, None::<String>);
        let sprint2_id = sprint2.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_sprint(sprint1).unwrap();
        tc.store.upsert_sprint(sprint2).unwrap();

        let context = tc.as_command_context();
        let cmd = UpdateSprint {
            sprint_id: sprint2_id,
            updates: crate::SprintUpdate {
                card_prefix: crate::FieldUpdate::Set("SPR".to_string()),
                ..Default::default()
            },
        };
        let err = cmd.execute(&context).unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_delete_sprint_clears_sprint_from_cards_with_command_timestamp() {
        use chrono::{TimeZone, Utc};

        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("KAN"));
        let board_id = board.id;
        let col = crate::Column::new(board_id, "Col", 0);
        let sprint = crate::Sprint::new(board_id, 1, None, None::<String>);
        let sprint_id = sprint.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col.clone()).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();

        let mut card = crate::Card::new(&mut crate::Board::new("B", Some("KAN")), col.id, "C", 0);
        card.sprint_id = Some(sprint_id);
        let card_id = card.id;
        tc.store.upsert_card(card).unwrap();

        let fixed_time = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        let context = tc.as_command_context();
        let cmd = DeleteSprint {
            sprint_id,
            timestamp: fixed_time,
        };
        cmd.execute(&context).unwrap();

        let card = tc.store.get_card(card_id).unwrap().unwrap();
        assert_eq!(
            card.updated_at, fixed_time,
            "clear_sprint_from_cards should use the command's timestamp, not Utc::now()"
        );
        assert_eq!(card.sprint_id, None);
    }

    #[test]
    fn test_delete_sprint_uses_embedded_timestamp() {
        use chrono::{TimeZone, Utc};

        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("KAN"));
        let board_id = board.id;
        let col = crate::Column::new(board_id, "Col", 0);
        let sprint = crate::Sprint::new(board_id, 1, None, None::<String>);
        let sprint_id = sprint.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col.clone()).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();

        let card = crate::Card {
            id: Uuid::new_v4(),
            column_id: col.id,
            title: "C".to_string(),
            description: None,
            priority: crate::CardPriority::Medium,
            status: crate::CardStatus::Todo,
            position: 0,
            due_date: None,
            points: None,
            card_number: 1,
            sprint_id: Some(sprint_id),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            sprint_logs: Vec::new(),
        };
        let archived = crate::ArchivedCard::new(card, uuid::Uuid::nil(), col.id, 0);
        tc.store.insert_archived_card(archived).unwrap();

        let fixed_time = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        let context = tc.as_command_context();
        let cmd = DeleteSprint {
            sprint_id,
            timestamp: fixed_time,
        };
        cmd.execute(&context).unwrap();

        let archived_cards = tc.store.list_archived_cards().unwrap();
        assert_eq!(archived_cards.len(), 1);
        assert_eq!(archived_cards[0].card.updated_at, fixed_time);
        assert_eq!(archived_cards[0].card.sprint_id, None);
    }

    #[test]
    fn test_validate_card_prefix_not_locked_with_no_cards_returns_ok() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("KAN"));
        let sprint = crate::Sprint::new(board.id, 1, None, Some("SPR"));
        let sprint_id = sprint.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();

        let context = tc.as_command_context();
        assert!(validate_card_prefix_not_locked(sprint_id, &context).is_ok());
    }

    #[test]
    fn test_validate_card_prefix_not_locked_with_active_card_returns_validation_error() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("B", Some("KAN"));
        let col = crate::Column::new(board.id, "Col", 0);
        let sprint = crate::Sprint::new(board.id, 1, None, Some("SPR"));
        let sprint_id = sprint.id;
        let mut card = crate::Card::new(&mut board, col.id, "C", 0);
        card.sprint_id = Some(sprint_id);
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();
        tc.store.upsert_card(card).unwrap();

        let context = tc.as_command_context();
        let err = validate_card_prefix_not_locked(sprint_id, &context).unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_validate_card_prefix_unique_for_distinct_prefix_returns_ok() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("KAN"));
        let board_id = board.id;
        let sprint = crate::Sprint::new(board_id, 1, None, Some("SPR"));
        let sprint_id = sprint.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();

        let context = tc.as_command_context();
        assert!(validate_card_prefix_unique("UNIQUE", sprint_id, board_id, &context).is_ok());
    }

    #[test]
    fn test_validate_card_prefix_unique_self_does_not_collide() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("KAN"));
        let board_id = board.id;
        let sprint = crate::Sprint::new(board_id, 1, None, Some("SPR"));
        let sprint_id = sprint.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();

        let context = tc.as_command_context();
        assert!(validate_card_prefix_unique("SPR", sprint_id, board_id, &context).is_ok());
    }

    #[test]
    fn test_allocate_sprint_name_sets_name_index_and_upserts_board() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("KAN"));
        let board_id = board.id;
        let sprint = crate::Sprint::new(board_id, 1, None, None::<String>);
        let sprint_id = sprint.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();

        let context = tc.as_command_context();
        let mut updates = SprintUpdate::default();
        allocate_sprint_name("My Sprint".to_string(), sprint_id, &context, &mut updates).unwrap();

        assert!(matches!(updates.name_index, crate::FieldUpdate::Set(_)));
        let board = tc.store.get_board(board_id).unwrap().unwrap();
        assert!(board.sprint_names.contains(&"My Sprint".to_string()));
    }

    #[test]
    fn test_validate_card_prefix_not_locked_with_archived_card_returns_validation_error() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("B", Some("KAN"));
        let col = crate::Column::new(board.id, "Col", 0);
        let sprint = crate::Sprint::new(board.id, 1, None, Some("SPR"));
        let sprint_id = sprint.id;
        let mut card = crate::Card::new(&mut board, col.id, "C", 0);
        card.sprint_id = Some(sprint_id);
        let archived = crate::ArchivedCard::new(card, uuid::Uuid::nil(), col.id, 0);
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();
        tc.store.insert_archived_card(archived).unwrap();

        let context = tc.as_command_context();
        let err = validate_card_prefix_not_locked(sprint_id, &context).unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_validate_card_prefix_unique_collides_with_board_prefix_returns_validation_error() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("KAN"));
        let board_id = board.id;
        let sprint = crate::Sprint::new(board_id, 1, None, Some("SPR"));
        let sprint_id = sprint.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();

        let context = tc.as_command_context();
        let err = validate_card_prefix_unique("KAN", sprint_id, board_id, &context).unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_validate_card_prefix_unique_collides_with_sibling_sprint_returns_validation_error() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("KAN"));
        let board_id = board.id;
        let mut sprint1 = crate::Sprint::new(board_id, 1, None, None::<String>);
        sprint1.card_prefix = Some("SPR".to_string());
        let sprint2 = crate::Sprint::new(board_id, 2, None, None::<String>);
        let sprint2_id = sprint2.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_sprint(sprint1).unwrap();
        tc.store.upsert_sprint(sprint2).unwrap();

        let context = tc.as_command_context();
        let err = validate_card_prefix_unique("SPR", sprint2_id, board_id, &context).unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_update_sprint_card_prefix_unique_valid_succeeds() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", Some("KAN"));
        let board_id = board.id;
        let sprint = crate::Sprint::new(board_id, 1, None, Some("SPR"));
        let sprint_id = sprint.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_sprint(sprint).unwrap();

        let context = tc.as_command_context();
        let cmd = UpdateSprint {
            sprint_id,
            updates: crate::SprintUpdate {
                card_prefix: crate::FieldUpdate::Set("UNIQUE".to_string()),
                ..Default::default()
            },
        };
        assert!(cmd.execute(&context).is_ok());
        let sprint = tc.store.get_sprint(sprint_id).unwrap().unwrap();
        assert_eq!(sprint.card_prefix, Some("UNIQUE".to_string()));
    }
}
