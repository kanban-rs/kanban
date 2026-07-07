use super::{Command, CommandContext};
use crate::data_store::DataStore;
use crate::field_update::FieldUpdate;
use crate::ColumnUpdate;
use crate::{KanbanError, KanbanResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ColumnCommand {
    Create(CreateColumn),
    Update(UpdateColumn),
    Delete(DeleteColumn),
}

impl ColumnCommand {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        match self {
            ColumnCommand::Create(c) => c.execute(context),
            ColumnCommand::Update(c) => c.execute(context),
            ColumnCommand::Delete(c) => c.execute(context),
        }
    }

    pub fn description(&self) -> String {
        match self {
            ColumnCommand::Create(c) => c.description(),
            ColumnCommand::Update(c) => c.description(),
            ColumnCommand::Delete(c) => c.description(),
        }
    }

    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        match self {
            ColumnCommand::Create(c) => c.capture_inverse(store),
            ColumnCommand::Update(c) => c.capture_inverse(store),
            ColumnCommand::Delete(c) => c.capture_inverse(store),
        }
    }
}

/// Update column properties (name, position, wip_limit)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateColumn {
    pub column_id: Uuid,
    pub updates: ColumnUpdate,
}

impl UpdateColumn {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let mut column = context.get_column(self.column_id)?;
        column.update(self.updates.clone());
        context.store.upsert_column(column)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        "Update column".to_string()
    }

    /// Inverse: read the column's current state and synthesise an
    /// `UpdateColumn` whose `updates` field-by-field set each touched
    /// field back to its prior value. Untouched fields stay `None` /
    /// `NoChange` so the inverse is minimal.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let column = match store.get_column(self.column_id)? {
            Some(c) => c,
            // The column doesn't exist — execute() will fail with NotFound
            // and rollback will take over. No inverse to capture.
            None => return Err(KanbanError::not_found("Column", self.column_id)),
        };

        let inverse_updates = ColumnUpdate {
            name: self.updates.name.as_ref().map(|_| column.name.clone()),
            position: self.updates.position.map(|_| column.position),
            wip_limit: match self.updates.wip_limit {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                FieldUpdate::Set(_) | FieldUpdate::Clear => match column.wip_limit {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
        };

        Ok(vec![Command::Column(ColumnCommand::Update(UpdateColumn {
            column_id: self.column_id,
            updates: inverse_updates,
        }))])
    }
}

/// Create a new column
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateColumn {
    pub id: Uuid,
    pub board_id: Uuid,
    pub name: String,
    pub position: i32,
}

impl CreateColumn {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        // Funnel construction through the factory (no `Column::new` + post-patch).
        // The frozen command shape carries only id/board_id/name/position, so
        // `wip_limit` defaults to `None`; the rich-spec create path (which honours
        // a client `wip_limit`) lives in the service tier via `Column::create`
        // dispatched through the import command.
        let spec = crate::NewColumn {
            board_id: self.board_id,
            name: self.name.clone(),
            wip_limit: None,
        };
        let column = crate::Column::create(spec, self.id, self.position, Utc::now())?;
        context.store.upsert_column(column)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Create column: '{}'", self.name)
    }

    /// Inverse: delete the newly-created column. The `id` is in the
    /// command — no pre-state read needed.
    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Ok(vec![Command::Column(ColumnCommand::Delete(DeleteColumn {
            column_id: self.id,
        }))])
    }
}

/// Delete a column
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteColumn {
    pub column_id: Uuid,
}

impl DeleteColumn {
    /// Inverse: re-create the deleted column with its prior id, board, name,
    /// and position. If the column had a non-default wip_limit, follow up
    /// with an UpdateColumn that restores it.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let column = match store.get_column(self.column_id)? {
            Some(c) => c,
            None => return Err(KanbanError::not_found("Column", self.column_id)),
        };
        let mut commands = vec![Command::Column(ColumnCommand::Create(CreateColumn {
            id: column.id,
            board_id: column.board_id,
            name: column.name.clone(),
            position: column.position,
        }))];
        if let Some(wip) = column.wip_limit {
            commands.push(Command::Column(ColumnCommand::Update(UpdateColumn {
                column_id: column.id,
                updates: ColumnUpdate {
                    wip_limit: FieldUpdate::Set(wip),
                    ..Default::default()
                },
            })));
        }
        Ok(commands)
    }

    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let has_cards = context.store.count_cards_in_column(self.column_id)? > 0;
        if has_cards {
            return Err(crate::KanbanError::validation(format!(
                "Cannot delete column {}: column contains cards",
                self.column_id
            )));
        }

        // Archived cards no longer block column deletion (D2 first-class model):
        // an `ArchivedCard` carries its own `board_id` and its `original_column_id`
        // is historical, not a live FK — it may dangle after the column is gone.
        context.store.delete_column(self.column_id)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Delete column {}", self.column_id)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::TestContext;
    use super::*;

    #[test]
    fn test_update_column_not_found_returns_error() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let cmd = UpdateColumn {
            column_id: Uuid::new_v4(),
            updates: ColumnUpdate::default(),
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_create_column_command_funnels_through_factory_with_injected_id() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let board_id = Uuid::new_v4();
        let cmd = CreateColumn {
            id,
            board_id,
            name: "Factory Funnel".to_string(),
            position: 3,
        };
        cmd.execute(&context).unwrap();

        let column = tc.store.get_column(id).unwrap().unwrap();
        assert_eq!(column.id, id);
        assert_eq!(column.board_id, board_id);
        assert_eq!(column.name, "Factory Funnel");
        // Server-managed position applied verbatim by the command.
        assert_eq!(column.position, 3);
        // The factory uses a single clock for both timestamps.
        assert_eq!(column.created_at, column.updated_at);
    }

    #[test]
    fn test_delete_column_with_archived_cards_now_succeeds() {
        // Under the D2 first-class model, an archived card's `original_column_id`
        // is historical (not a live FK), so a column that only holds archived
        // cards can be deleted. Board-scoped cleanup handles the archived record.
        let tc = TestContext::new();
        let mut board = crate::Board::new("B", Some("TST"));
        let board_id = board.id;
        let col = crate::Column::new(board_id, "C", 0);
        let col_id = col.id;
        let card = crate::Card::new(&mut board, col_id, "archived", 0);
        let archived = crate::ArchivedCard::new(card, board_id, col_id, 0);
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.insert_archived_card(archived).unwrap();

        let context = tc.as_command_context();
        let cmd = DeleteColumn { column_id: col_id };
        cmd.execute(&context).unwrap();

        assert!(
            tc.store.get_column(col_id).unwrap().is_none(),
            "column with only archived cards must be deletable"
        );
        assert_eq!(
            tc.store.list_archived_cards().unwrap().len(),
            1,
            "the archived record survives the column deletion (dangling original_column_id)"
        );
    }

    #[test]
    fn test_create_column_command_rejects_negative_position_via_factory_validation() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let cmd = CreateColumn {
            id: Uuid::new_v4(),
            board_id: Uuid::new_v4(),
            name: "Bad".to_string(),
            position: -1,
        };
        // The legacy `Column::new` + id-overwrite path silently accepts a
        // negative position; routing through `Column::create` enforces the
        // non-negativity invariant, so this must now be a validation error.
        let err = cmd.execute(&context).unwrap_err();
        assert!(err.is_validation());
    }
}
