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
        if let Some(position) = self.updates.position {
            if position < 0 {
                return Err(KanbanError::validation(format!(
                    "column position must be >= 0, got {position}"
                )));
            }
        }
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
            default_status: self.updates.default_status.map(|_| column.default_status),
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
    #[serde(default)]
    pub default_status: Option<crate::card::CardStatus>,
}

impl CreateColumn {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        // Funnel construction through the factory (no `Column::new` + post-patch).
        // The frozen command shape carries only id/board_id/name/position/
        // default_status, so `wip_limit` defaults to `None`; the rich-spec
        // create path (which honours a client `wip_limit`) lives in the
        // service tier via `Column::create` dispatched through the import
        // command.
        let spec = crate::NewColumn {
            board_id: self.board_id,
            name: self.name.clone(),
            wip_limit: None,
            default_status: self.default_status,
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
    /// with an UpdateColumn that restores it; if the column was in the board's
    /// completion configuration (which the forward delete prunes), follow up
    /// with an UpdateBoard that restores the full ordered list.
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
            default_status: column.default_status,
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
        if let Some(board) = store.get_board(column.board_id)? {
            if board.is_completion_column(self.column_id) {
                commands.push(Command::Board(super::BoardCommand::Update(
                    super::board_commands::UpdateBoard {
                        board_id: board.id,
                        updates: crate::BoardUpdate {
                            completion_column_ids: Some(board.completion_column_ids.clone()),
                            ..Default::default()
                        },
                    },
                )));
            }
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

        // Prune the column from its board's completion configuration BEFORE the
        // row delete, so every backend agrees: SQLite would drop the join row by
        // cascade anyway, but JSON/in-memory hold the list on the board and
        // would otherwise keep a dangling id.
        if let Some(column) = context.store.get_column(self.column_id)? {
            if let Some(board) = context.store.get_board(column.board_id)? {
                if board.is_completion_column(self.column_id) {
                    let mut board = board;
                    let ids = board
                        .completion_column_ids
                        .iter()
                        .copied()
                        .filter(|id| *id != self.column_id)
                        .collect();
                    board.update_completion_column_ids(ids);
                    context.store.upsert_board(board)?;
                }
            }
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
