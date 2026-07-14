use super::{Command, CommandContext};
use crate::data_store::DataStore;
use crate::field_update::FieldUpdate;
use crate::KanbanResult;
use crate::{ArchivedCard, Board, Card, Column, DependencyGraph, KanbanError, NewBoard, Sprint};
use chrono::Utc;
use kanban_core::Editable;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BoardCommand {
    Create(CreateBoard),
    Update(UpdateBoard),
    SetTaskSort(SetBoardTaskSort),
    SetTaskListView(SetBoardTaskListView),
    Delete(DeleteBoard),
    ApplySettings(ApplyBoardSettings),
    Import(ImportEntities),
    /// Internal: replace a board's sprint-name pool wholesale. Used by
    /// `UpdateSprint`'s inverse to restore the pool that name allocation
    /// mutated. Not a user-facing command — accessed only via the
    /// inverse-capture path.
    RestoreSprintPool(RestoreSprintPool),
    /// Archive one or more boards: move each board head out of the live
    /// `boards` set into the discrete archived collection (C2).
    Archive(ArchiveBoards),
    /// Restore an archived board: move it back into the live `boards` set.
    Restore(RestoreBoard),
}

impl BoardCommand {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        match self {
            BoardCommand::Create(c) => c.execute(context),
            BoardCommand::Update(c) => c.execute(context),
            BoardCommand::SetTaskSort(c) => c.execute(context),
            BoardCommand::SetTaskListView(c) => c.execute(context),
            BoardCommand::Delete(c) => c.execute(context),
            BoardCommand::ApplySettings(c) => c.execute(context),
            BoardCommand::Import(c) => c.execute(context),
            BoardCommand::RestoreSprintPool(c) => c.execute(context),
            BoardCommand::Archive(c) => c.execute(context),
            BoardCommand::Restore(c) => c.execute(context),
        }
    }

    pub fn description(&self) -> String {
        match self {
            BoardCommand::Create(c) => c.description(),
            BoardCommand::Update(c) => c.description(),
            BoardCommand::SetTaskSort(c) => c.description(),
            BoardCommand::SetTaskListView(c) => c.description(),
            BoardCommand::Delete(c) => c.description(),
            BoardCommand::ApplySettings(c) => c.description(),
            BoardCommand::Import(c) => c.description(),
            BoardCommand::RestoreSprintPool(c) => c.description(),
            BoardCommand::Archive(c) => c.description(),
            BoardCommand::Restore(c) => c.description(),
        }
    }

    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        match self {
            BoardCommand::Create(c) => c.capture_inverse(store),
            BoardCommand::Update(c) => c.capture_inverse(store),
            BoardCommand::SetTaskSort(c) => c.capture_inverse(store),
            BoardCommand::SetTaskListView(c) => c.capture_inverse(store),
            BoardCommand::ApplySettings(c) => c.capture_inverse(store),
            BoardCommand::Delete(c) => c.capture_inverse(store),
            BoardCommand::Import(c) => c.capture_inverse(store),
            BoardCommand::RestoreSprintPool(c) => c.capture_inverse(store),
            BoardCommand::Archive(c) => c.capture_inverse(store),
            BoardCommand::Restore(c) => c.capture_inverse(store),
        }
    }
}

/// Internal — replace a board's sprint-name pool and used-count
/// wholesale. Emitted by `UpdateSprint::capture_inverse` to restore
/// pool state that the forward command's name allocation mutated.
///
/// Not exposed to user-facing CLI/MCP commands. `capture_inverse`
/// rejects top-level execute (the command is synthetic-only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreSprintPool {
    pub board_id: Uuid,
    pub sprint_names: Vec<String>,
    pub sprint_name_used_count: usize,
}

impl RestoreSprintPool {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let mut board = context.get_board(self.board_id)?;
        board.sprint_names = self.sprint_names.clone();
        board.sprint_name_used_count = self.sprint_name_used_count;
        context.store.upsert_board(board)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Restore sprint-name pool for board {}", self.board_id)
    }

    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Err(KanbanError::Internal(format!(
            "RestoreSprintPool is a synthetic command — it must only appear inside an inverse batch (UpdateSprint undo), never as a top-level forward command. Board id: {}",
            self.board_id
        )))
    }
}

/// Create a new board
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateBoard {
    pub id: Uuid,
    pub name: String,
    pub card_prefix: Option<String>,
    #[serde(default)]
    pub position: i32,
}

impl CreateBoard {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        // Funnel construction through the factory (no `Board::new` + post-patch).
        // The frozen command shape carries only name/card_prefix/position, so the
        // remaining create fields default and the clock is captured here; the
        // rich-spec create path lives in the service tier via `Board::create`.
        let spec = NewBoard {
            name: self.name.clone(),
            description: None,
            sprint_prefix: None,
            card_prefix: self.card_prefix.clone(),
            task_sort_field: None,
            task_sort_order: None,
            sprint_duration_days: None,
            task_list_view: None,
            completion_column_id: None,
        };
        let mut board = Board::create(spec, self.id, Utc::now())?;
        // `position` is server-managed and not part of `NewBoard`; apply post-create.
        board.position = self.position;
        context.store.upsert_board(board)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Create board: '{}'", self.name)
    }

    /// Inverse: delete the newly-created board. The `id` is already in the
    /// command, so no pre-state read from the store is required — `_store`
    /// is unused.
    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Ok(vec![Command::Board(BoardCommand::Delete(DeleteBoard {
            board_id: self.id,
        }))])
    }
}

/// Update board properties (name, description, prefixes, sort options, etc.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateBoard {
    pub board_id: Uuid,
    pub updates: crate::BoardUpdate,
}

impl UpdateBoard {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let mut board = context.get_board(self.board_id)?;
        if !matches!(self.updates.card_prefix, FieldUpdate::NoChange) && board.card_counter > 1 {
            return Err(KanbanError::validation(
                "board card_prefix cannot be changed after cards have been created",
            ));
        }
        board.update(self.updates.clone());
        context.store.upsert_board(board)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        "Update board".to_string()
    }

    /// Inverse: read the board's current state and build a BoardUpdate
    /// that reverses every field the forward command touched.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let board = match store.get_board(self.board_id)? {
            Some(b) => b,
            None => return Err(KanbanError::not_found("Board", self.board_id)),
        };
        let upd = &self.updates;
        let inverse = crate::BoardUpdate {
            name: upd.name.as_ref().map(|_| board.name.clone()),
            description: match upd.description {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match board.description {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
            sprint_prefix: match upd.sprint_prefix {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match board.sprint_prefix {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
            card_prefix: match upd.card_prefix {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match board.card_prefix {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
            task_sort_field: upd.task_sort_field.map(|_| board.task_sort_field),
            task_sort_order: upd.task_sort_order.map(|_| board.task_sort_order),
            sprint_duration_days: match upd.sprint_duration_days {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match board.sprint_duration_days {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
            task_list_view: upd.task_list_view.map(|_| board.task_list_view),
            active_sprint_id: match upd.active_sprint_id {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match board.active_sprint_id {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
            completion_column_id: match upd.completion_column_id {
                FieldUpdate::NoChange => FieldUpdate::NoChange,
                _ => match board.completion_column_id {
                    Some(v) => FieldUpdate::Set(v),
                    None => FieldUpdate::Clear,
                },
            },
            position: upd.position.map(|_| board.position),
        };
        Ok(vec![Command::Board(BoardCommand::Update(UpdateBoard {
            board_id: self.board_id,
            updates: inverse,
        }))])
    }
}

/// Update board's task sorting preference
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetBoardTaskSort {
    pub board_id: Uuid,
    pub field: crate::SortField,
    pub order: crate::SortOrder,
}

impl SetBoardTaskSort {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let mut board = context.get_board(self.board_id)?;
        board.update_task_sort(self.field, self.order);
        context.store.upsert_board(board)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Set board task sort to {:?} {:?}", self.field, self.order)
    }

    /// Inverse: another SetBoardTaskSort with the prior values.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let board = match store.get_board(self.board_id)? {
            Some(b) => b,
            None => return Err(KanbanError::not_found("Board", self.board_id)),
        };
        Ok(vec![Command::Board(BoardCommand::SetTaskSort(
            SetBoardTaskSort {
                board_id: self.board_id,
                field: board.task_sort_field,
                order: board.task_sort_order,
            },
        ))])
    }
}

/// Update board's task list view
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetBoardTaskListView {
    pub board_id: Uuid,
    pub view: crate::TaskListView,
}

impl SetBoardTaskListView {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let mut board = context.get_board(self.board_id)?;
        board.update_task_list_view(self.view);
        context.store.upsert_board(board)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Set board task list view to {:?}", self.view)
    }

    /// Inverse: another SetBoardTaskListView with the prior view.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let board = match store.get_board(self.board_id)? {
            Some(b) => b,
            None => return Err(KanbanError::not_found("Board", self.board_id)),
        };
        Ok(vec![Command::Board(BoardCommand::SetTaskListView(
            SetBoardTaskListView {
                board_id: self.board_id,
                view: board.task_list_view,
            },
        ))])
    }
}

/// Delete a board and all associated columns, cards, and sprints
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteBoard {
    pub board_id: Uuid,
}

impl DeleteBoard {
    /// Delete the board record. **Atomic only** — does not cascade to columns,
    /// cards, sprints, or graph edges. Cascade orchestration is the
    /// responsibility of the service layer (see
    /// `KanbanContext::delete_board`).
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        context.store.delete_board(self.board_id)
    }

    pub fn description(&self) -> String {
        format!("Delete board: {}", self.board_id)
    }

    /// Inverse: re-insert the deleted Board via ImportEntities. The
    /// cascade siblings (DeleteColumnsByBoard, DeleteSprintsByBoard,
    /// DeleteCardsByColumns, DeleteCardEdges) capture their own
    /// entities, so undoing the full cascade restores everything.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let board = match store.get_board(self.board_id)? {
            Some(b) => b,
            None => return Err(KanbanError::not_found("Board", self.board_id)),
        };
        Ok(vec![Command::Board(BoardCommand::Import(ImportEntities {
            boards: vec![board],
            ..Default::default()
        }))])
    }
}

/// Archive one or more boards in a single command (single undo entry). Each
/// board head moves out of the live `boards` set into the discrete archived
/// collection as `Archived<Board>`; the subtree (columns/cards/sprints/edges)
/// stays in place in the flat collections — a board is a scoping ROOT, nothing
/// else moves. Reversible via `RestoreBoard` (symmetric collection move).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveBoards {
    pub ids: Vec<Uuid>,
}

impl ArchiveBoards {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        for id in &self.ids {
            let board = context
                .store
                .get_board(*id)?
                .ok_or_else(|| KanbanError::not_found("Board", *id))?;
            context
                .store
                .insert_archived_board(crate::Archived::now(board))?;
            context.store.delete_board(*id)?;
        }
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Archive {} board(s)", self.ids.len())
    }

    /// Inverse: one `RestoreBoard` per id. `capture_inverse` runs BEFORE
    /// execute (the boards are still live), so `get_board` guards existence;
    /// the inverse `RestoreBoard` runs during undo AFTER the forward archive,
    /// when the board sits in the archived collection. No payload needed — the
    /// wrapped `Board` carries its own position.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let mut commands: Vec<Command> = Vec::new();
        for id in &self.ids {
            if store.get_board(*id)?.is_none() {
                return Err(KanbanError::not_found("Board", *id));
            }
            commands.push(Command::Board(BoardCommand::Restore(RestoreBoard {
                board_id: *id,
            })));
        }
        Ok(commands)
    }
}

/// Restore an archived board: move it back from the archived collection into
/// the live `boards` set. Symmetric inverse of `ArchiveBoards`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestoreBoard {
    pub board_id: Uuid,
}

impl RestoreBoard {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let archived = context
            .store
            .get_archived_board(self.board_id)?
            .ok_or_else(|| KanbanError::not_found("archived board", self.board_id))?;
        context.store.upsert_board(archived.into_entity())?;
        context.store.delete_archived_board(self.board_id)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Restore board {}", self.board_id)
    }

    /// Inverse: re-archive. Mirror-symmetric with `ArchiveBoards::capture_inverse`.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        if store.get_archived_board(self.board_id)?.is_none() {
            return Err(KanbanError::not_found("archived board", self.board_id));
        }
        Ok(vec![Command::Board(BoardCommand::Archive(ArchiveBoards {
            ids: vec![self.board_id],
        }))])
    }
}

/// Apply board settings from a DTO (used by JSON editor).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApplyBoardSettings {
    pub board_id: Uuid,
    pub dto: crate::editable::BoardSettingsDto,
}

impl ApplyBoardSettings {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let mut board = context.get_board(self.board_id)?;
        self.dto.clone().apply_to(&mut board);
        context.store.upsert_board(board)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Apply board settings for {}", self.board_id)
    }

    /// Inverse: snapshot the current board into a `BoardSettingsDto` via the
    /// `Editable::from_entity` impl, then re-apply that DTO via another
    /// `ApplyBoardSettings`. The DTO covers exactly the fields this command
    /// writes, so the round-trip is symmetric.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let board = match store.get_board(self.board_id)? {
            Some(b) => b,
            None => return Err(KanbanError::not_found("Board", self.board_id)),
        };
        let prior_dto = crate::editable::BoardSettingsDto::from_entity(&board);
        Ok(vec![Command::Board(BoardCommand::ApplySettings(
            ApplyBoardSettings {
                board_id: self.board_id,
                dto: prior_dto,
            },
        ))])
    }
}

/// Import entities (boards, columns, cards, etc.) into the context.
/// Used by TUI import functionality. Appends without replacing existing data.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportEntities {
    #[serde(with = "crate::board_factory::board_vec_serde")]
    pub boards: Vec<Board>,
    #[serde(with = "crate::column_factory::column_vec_serde")]
    pub columns: Vec<Column>,
    #[serde(with = "crate::card_factory::card_vec_serde")]
    pub cards: Vec<Card>,
    pub archived_cards: Vec<ArchivedCard>,
    #[serde(with = "crate::sprint_factory::sprint_vec_serde")]
    pub sprints: Vec<Sprint>,
    pub graph: Option<DependencyGraph>,
}

impl ImportEntities {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        use std::collections::HashSet;

        // Include archived boards: `list_boards` is now live-only (archived
        // boards live in a discrete collection), so dedup must also read the
        // archived set or an import could silently collide with an archived
        // board id. Safe across backends — the `list_archived_boards` default
        // returns empty (no bricking).
        let existing_board_ids: HashSet<Uuid> = context
            .store
            .list_boards()?
            .iter()
            .map(|b| b.id)
            .chain(
                context
                    .store
                    .list_archived_boards()?
                    .iter()
                    .map(|ab| ab.entity.id),
            )
            .collect();
        let existing_column_ids: HashSet<Uuid> = context
            .store
            .list_all_columns()?
            .iter()
            .map(|c| c.id)
            .collect();
        let existing_card_ids: HashSet<Uuid> = context
            .store
            .list_all_cards()?
            .iter()
            .map(|c| c.id)
            .collect();
        let existing_sprint_ids: HashSet<Uuid> = context
            .store
            .list_all_sprints()?
            .iter()
            .map(|s| s.id)
            .collect();
        let existing_archived_ids: HashSet<Uuid> = context
            .store
            .list_archived_cards()?
            .iter()
            .map(|ac| ac.entity.id)
            .collect();

        for b in &self.boards {
            if existing_board_ids.contains(&b.id) {
                return Err(crate::KanbanError::validation(format!(
                    "Duplicate board ID: {}",
                    b.id
                )));
            }
        }
        for c in &self.columns {
            if existing_column_ids.contains(&c.id) {
                return Err(crate::KanbanError::validation(format!(
                    "Duplicate column ID: {}",
                    c.id
                )));
            }
        }
        for c in &self.cards {
            if existing_card_ids.contains(&c.id) || existing_archived_ids.contains(&c.id) {
                return Err(crate::KanbanError::validation(format!(
                    "Duplicate card ID (live or archived): {}",
                    c.id
                )));
            }
        }
        for ac in &self.archived_cards {
            if existing_archived_ids.contains(&ac.entity.id)
                || existing_card_ids.contains(&ac.entity.id)
            {
                return Err(crate::KanbanError::validation(format!(
                    "Duplicate archived card ID (live or archived): {}",
                    ac.entity.id
                )));
            }
        }
        for s in &self.sprints {
            if existing_sprint_ids.contains(&s.id) {
                return Err(crate::KanbanError::validation(format!(
                    "Duplicate sprint ID: {}",
                    s.id
                )));
            }
        }

        for b in &self.boards {
            context.store.upsert_board(b.clone())?;
        }
        for c in &self.columns {
            context.store.upsert_column(c.clone())?;
        }
        for c in &self.cards {
            context.store.upsert_card(c.clone())?;
        }
        for ac in &self.archived_cards {
            context.store.insert_archived_card(ac.clone())?;
        }
        for s in &self.sprints {
            context.store.upsert_sprint(s.clone())?;
        }
        if let Some(ref graph) = self.graph {
            context.store.set_graph(graph.clone())?;
        }
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Import {} board(s)", self.boards.len())
    }

    /// Inverse: emit one delete command per imported entity. The IDs are
    /// in the forward command, so no pre-state read needed.
    ///
    /// Order matters: delete cards before columns before boards so
    /// foreign-key-style invariants stay satisfied (the in-memory store
    /// doesn't enforce them, but downstream backends may).
    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let mut commands: Vec<Command> = Vec::new();

        // Cards first.
        if !self.cards.is_empty() {
            commands.push(Command::Card(crate::commands::CardCommand::Archive(
                crate::commands::ArchiveCards {
                    ids: self.cards.iter().map(|c| c.id).collect(),
                },
            )));
        }

        // Archived cards: per-card permanent delete.
        for ac in &self.archived_cards {
            commands.push(Command::Card(crate::commands::CardCommand::Delete(
                crate::commands::DeleteCard {
                    card_id: ac.entity.id,
                },
            )));
        }

        // Sprints: per-sprint delete.
        for s in &self.sprints {
            commands.push(Command::Sprint(crate::commands::SprintCommand::Delete(
                crate::commands::DeleteSprint {
                    sprint_id: s.id,
                    timestamp: chrono::Utc::now(),
                },
            )));
        }

        // Columns: per-column delete (must be empty by the time we get
        // here — cards above were archived first).
        for c in &self.columns {
            commands.push(Command::Column(crate::commands::ColumnCommand::Delete(
                crate::commands::DeleteColumn { column_id: c.id },
            )));
        }

        // Boards last.
        for b in &self.boards {
            commands.push(Command::Board(BoardCommand::Delete(DeleteBoard {
                board_id: b.id,
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
    fn test_create_board_command_funnels_through_factory_with_injected_id() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let cmd = CreateBoard {
            id,
            name: "Factory Funnel".to_string(),
            card_prefix: Some("KAN".to_string()),
            position: 3,
        };
        cmd.execute(&context).unwrap();

        let board = tc.store.get_board(id).unwrap().unwrap();
        assert_eq!(board.id, id);
        assert_eq!(board.name, "Factory Funnel");
        assert_eq!(board.card_prefix, Some("KAN".to_string()));
        // Server-managed position applied verbatim, counters seeded by the factory:
        assert_eq!(board.position, 3);
        assert_eq!(board.card_counter, 1);
        assert_eq!(board.next_sprint_number, 1);
        // Factory uses a single clock for both timestamps:
        assert_eq!(board.created_at, board.updated_at);
    }

    #[test]
    fn test_create_board_command_rejects_blank_name_via_factory_validation() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let cmd = CreateBoard {
            id: Uuid::new_v4(),
            name: "   ".to_string(),
            card_prefix: None,
            position: 0,
        };
        let err = cmd.execute(&context).unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_update_board_not_found_returns_error() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let cmd = UpdateBoard {
            board_id: Uuid::new_v4(),
            updates: crate::BoardUpdate::default(),
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_set_board_task_sort_not_found_returns_error() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let cmd = SetBoardTaskSort {
            board_id: Uuid::new_v4(),
            field: crate::SortField::Priority,
            order: crate::SortOrder::Ascending,
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_set_board_task_list_view_not_found_returns_error() {
        let tc = TestContext::new();
        let context = tc.as_command_context();
        let cmd = SetBoardTaskListView {
            board_id: Uuid::new_v4(),
            view: crate::TaskListView::default(),
        };
        let result = cmd.execute(&context);
        assert!(result.unwrap_err().is_not_found());
    }

    #[test]
    fn test_import_entities_with_duplicate_board_id_returns_error() {
        let tc = TestContext::new();
        let b1 = Board::new("B1", None::<String>);
        let dup_id = b1.id;
        tc.store.upsert_board(b1).unwrap();

        let mut dup = Board::new("Dup", None::<String>);
        dup.id = dup_id;

        let cmd = ImportEntities {
            boards: vec![dup],
            columns: vec![],
            cards: vec![],
            archived_cards: vec![],
            sprints: vec![],
            graph: None,
        };
        let context = tc.as_command_context();
        let result = cmd.execute(&context);
        assert!(result.is_err());
        assert!(result.unwrap_err().is_validation());
    }

    #[test]
    fn test_import_entities_with_duplicate_card_id_returns_error() {
        let tc = TestContext::new();
        let mut board = Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let card = crate::Card::new(&mut board, col.id, "Card", 0);
        let dup_card_id = card.id;
        tc.store.upsert_board(board.clone()).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_card(card).unwrap();

        let mut dup_card = crate::Card::new(&mut board, Uuid::new_v4(), "Dup", 0);
        dup_card.id = dup_card_id;

        let cmd = ImportEntities {
            boards: vec![],
            columns: vec![],
            cards: vec![dup_card],
            archived_cards: vec![],
            sprints: vec![],
            graph: None,
        };
        let context = tc.as_command_context();
        let result = cmd.execute(&context);
        assert!(result.is_err());
        assert!(result.unwrap_err().is_validation());
    }

    #[test]
    fn test_import_entities_live_card_colliding_with_existing_archived_returns_error() {
        let tc = TestContext::new();
        let mut board = Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let archived = crate::Card::new(&mut board, col.id, "Archived", 0);
        let collision_id = archived.id;
        tc.store.upsert_board(board.clone()).unwrap();
        tc.store.upsert_column(col.clone()).unwrap();
        tc.store
            .insert_archived_card(crate::ArchivedCard::new(
                archived,
                uuid::Uuid::nil(),
                col.id,
                0,
            ))
            .unwrap();

        let mut imported_live = crate::Card::new(&mut board, col.id, "ImportedLive", 0);
        imported_live.id = collision_id;

        let cmd = ImportEntities {
            boards: vec![],
            columns: vec![],
            cards: vec![imported_live],
            archived_cards: vec![],
            sprints: vec![],
            graph: None,
        };
        let context = tc.as_command_context();
        let result = cmd.execute(&context);
        assert!(result.is_err());
        assert!(result.unwrap_err().is_validation());
    }

    #[test]
    fn test_import_entities_archived_card_colliding_with_existing_live_returns_error() {
        let tc = TestContext::new();
        let mut board = Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let live = crate::Card::new(&mut board, col.id, "Live", 0);
        let collision_id = live.id;
        tc.store.upsert_board(board.clone()).unwrap();
        tc.store.upsert_column(col.clone()).unwrap();
        tc.store.upsert_card(live).unwrap();

        let mut imported_archived = crate::Card::new(&mut board, col.id, "ImportedArchived", 0);
        imported_archived.id = collision_id;

        let cmd = ImportEntities {
            boards: vec![],
            columns: vec![],
            cards: vec![],
            archived_cards: vec![crate::ArchivedCard::new(
                imported_archived,
                uuid::Uuid::nil(),
                col.id,
                0,
            )],
            sprints: vec![],
            graph: None,
        };
        let context = tc.as_command_context();
        let result = cmd.execute(&context);
        assert!(result.is_err());
        assert!(result.unwrap_err().is_validation());
    }

    #[test]
    fn test_import_entities_with_duplicate_archived_card_id_returns_error() {
        let tc = TestContext::new();
        let mut board = Board::new("B", Some("TST"));
        let col = crate::Column::new(board.id, "Col", 0);
        let archived = crate::Card::new(&mut board, col.id, "Archived", 0);
        let dup_id = archived.id;
        tc.store.upsert_board(board.clone()).unwrap();
        tc.store.upsert_column(col.clone()).unwrap();
        tc.store
            .insert_archived_card(crate::ArchivedCard::new(
                archived,
                uuid::Uuid::nil(),
                col.id,
                0,
            ))
            .unwrap();

        let mut dup = crate::Card::new(&mut board, col.id, "Dup", 0);
        dup.id = dup_id;

        let cmd = ImportEntities {
            boards: vec![],
            columns: vec![],
            cards: vec![],
            archived_cards: vec![crate::ArchivedCard::new(dup, uuid::Uuid::nil(), col.id, 0)],
            sprints: vec![],
            graph: None,
        };
        let context = tc.as_command_context();
        let result = cmd.execute(&context);
        assert!(result.is_err());
        assert!(result.unwrap_err().is_validation());
    }

    #[test]
    fn test_import_entities_appends_without_replacing() {
        let tc = TestContext::new();
        let b1 = Board::new("B1", None::<String>);
        tc.store.upsert_board(b1).unwrap();

        let b2 = Board::new("B2", None::<String>);
        let col = crate::Column::new(b2.id, "Todo", 0);
        let mut b2_clone = b2.clone();
        let card = crate::Card::new(&mut b2_clone, col.id, "Card", 0);

        let cmd = ImportEntities {
            boards: vec![b2],
            columns: vec![col],
            cards: vec![card],
            archived_cards: vec![],
            sprints: vec![],
            graph: None,
        };

        let context = tc.as_command_context();
        cmd.execute(&context).unwrap();

        let boards = tc.store.list_boards().unwrap();
        assert_eq!(boards.len(), 2);
        assert!(boards.iter().any(|b| b.name == "B1"));
        assert!(boards.iter().any(|b| b.name == "B2"));
        assert_eq!(tc.store.list_all_columns().unwrap().len(), 1);
        assert_eq!(tc.store.list_all_cards().unwrap().len(), 1);
    }

    #[test]
    fn test_update_board_card_prefix_allowed_before_first_card_succeeds() {
        let tc = TestContext::new();
        let board = Board::new("B", Some("OLD"));
        let board_id = board.id;
        tc.store.upsert_board(board).unwrap();
        let context = tc.as_command_context();

        let cmd = UpdateBoard {
            board_id,
            updates: crate::BoardUpdate {
                card_prefix: FieldUpdate::Set("NEW".to_string()),
                ..Default::default()
            },
        };
        assert!(cmd.execute(&context).is_ok());
        let board = tc.store.get_board(board_id).unwrap().unwrap();
        assert_eq!(board.card_prefix, Some("NEW".to_string()));
    }

    #[test]
    fn test_update_board_card_prefix_locked_after_first_card_returns_validation_error() {
        let tc = TestContext::new();
        let mut board = Board::new("B", Some("OLD"));
        let board_id = board.id;
        let col = Column::new(board_id, "Col", 0);
        let _card = Card::new(&mut board, col.id, "C", 0);
        // card_counter is now 2 (incremented past initial 1)
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        let context = tc.as_command_context();

        let cmd = UpdateBoard {
            board_id,
            updates: crate::BoardUpdate {
                card_prefix: FieldUpdate::Set("NEW".to_string()),
                ..Default::default()
            },
        };
        let err = cmd.execute(&context).unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_update_board_clear_card_prefix_locked_after_first_card_returns_validation_error() {
        let tc = TestContext::new();
        let mut board = Board::new("B", Some("OLD"));
        let board_id = board.id;
        let col = Column::new(board_id, "Col", 0);
        let _card = Card::new(&mut board, col.id, "C", 0);
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        let context = tc.as_command_context();

        let cmd = UpdateBoard {
            board_id,
            updates: crate::BoardUpdate {
                card_prefix: FieldUpdate::Clear,
                ..Default::default()
            },
        };
        let err = cmd.execute(&context).unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_delete_board_atomic_removes_only_board_record() {
        let tc = TestContext::new();
        let board = Board::new("B", Some("TST"));
        let board_id = board.id;
        let col = Column::new(board_id, "Col", 0);
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col.clone()).unwrap();

        let context = tc.as_command_context();
        let cmd = DeleteBoard { board_id };
        cmd.execute(&context).unwrap();

        assert!(tc.store.list_boards().unwrap().is_empty());
        assert_eq!(
            tc.store.list_all_columns().unwrap().len(),
            1,
            "atomic DeleteBoard must not cascade to columns; cascade is the service's responsibility"
        );
    }

    // ===== C2: board archive / restore (collection move) =====

    /// Seed a board with one column and one card; return (board_id, column_id,
    /// card_id).
    fn seed_board_with_subtree(tc: &TestContext) -> (Uuid, Uuid, Uuid) {
        let mut board = Board::new("B", Some("TST"));
        let board_id = board.id;
        let col = crate::Column::new(board_id, "Col", 0);
        let col_id = col.id;
        let card = crate::Card::new(&mut board, col_id, "Task", 0);
        let card_id = card.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col).unwrap();
        tc.store.upsert_card(card).unwrap();
        (board_id, col_id, card_id)
    }

    #[test]
    fn test_archive_boards_moves_board_from_live_to_archived_set() {
        let tc = TestContext::new();
        let (board_id, _, _) = seed_board_with_subtree(&tc);
        let ctx = tc.as_command_context();

        ArchiveBoards {
            ids: vec![board_id],
        }
        .execute(&ctx)
        .unwrap();

        assert!(
            tc.store.list_boards().unwrap().is_empty(),
            "archived board leaves the live set"
        );
        assert!(tc.store.get_board(board_id).unwrap().is_none());
        let archived = tc.store.list_archived_boards().unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].entity.id, board_id);
    }

    #[test]
    fn test_archive_board_leaves_subtree_columns_and_cards_in_place() {
        let tc = TestContext::new();
        let (board_id, col_id, card_id) = seed_board_with_subtree(&tc);
        let ctx = tc.as_command_context();

        ArchiveBoards {
            ids: vec![board_id],
        }
        .execute(&ctx)
        .unwrap();

        assert!(
            tc.store.get_column(col_id).unwrap().is_some(),
            "column stays in the flat collection"
        );
        assert!(
            tc.store.get_card(card_id).unwrap().is_some(),
            "card stays in the flat collection"
        );
    }

    #[test]
    fn test_restore_board_moves_it_back_losslessly() {
        let tc = TestContext::new();
        let (board_id, _, _) = seed_board_with_subtree(&tc);
        let original = tc.store.get_board(board_id).unwrap().unwrap();
        let ctx = tc.as_command_context();

        ArchiveBoards {
            ids: vec![board_id],
        }
        .execute(&ctx)
        .unwrap();
        RestoreBoard { board_id }.execute(&ctx).unwrap();

        let back = tc.store.get_board(board_id).unwrap().unwrap();
        assert_eq!(back, original, "restore returns the board verbatim");
        assert!(tc.store.list_archived_boards().unwrap().is_empty());
    }

    #[test]
    fn test_archive_then_undo_restores_board_identity() {
        let tc = TestContext::new();
        let (board_id, _, _) = seed_board_with_subtree(&tc);
        let original = tc.store.get_board(board_id).unwrap().unwrap();

        let forward = ArchiveBoards {
            ids: vec![board_id],
        };
        // Undo captures the inverse BEFORE the forward runs.
        let inverse = forward.capture_inverse(&tc.store).unwrap();
        let ctx = tc.as_command_context();
        forward.execute(&ctx).unwrap();
        assert!(tc.store.get_board(board_id).unwrap().is_none());

        for cmd in inverse {
            cmd.execute(&ctx).unwrap();
        }
        let back = tc.store.get_board(board_id).unwrap().unwrap();
        assert_eq!(back, original);
        assert!(tc.store.list_archived_boards().unwrap().is_empty());
    }

    #[test]
    fn test_restore_then_undo_re_archives_board() {
        let tc = TestContext::new();
        let (board_id, _, _) = seed_board_with_subtree(&tc);
        let ctx = tc.as_command_context();
        ArchiveBoards {
            ids: vec![board_id],
        }
        .execute(&ctx)
        .unwrap();

        let forward = RestoreBoard { board_id };
        let inverse = forward.capture_inverse(&tc.store).unwrap();
        forward.execute(&ctx).unwrap();
        assert!(tc.store.get_board(board_id).unwrap().is_some());

        for cmd in inverse {
            cmd.execute(&ctx).unwrap();
        }
        assert!(
            tc.store.get_board(board_id).unwrap().is_none(),
            "re-archived by the inverse"
        );
        assert_eq!(tc.store.list_archived_boards().unwrap().len(), 1);
    }

    #[test]
    fn test_archive_missing_board_returns_not_found() {
        let tc = TestContext::new();
        let ctx = tc.as_command_context();
        let result = ArchiveBoards {
            ids: vec![Uuid::new_v4()],
        }
        .execute(&ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_import_board_colliding_with_archived_is_rejected() {
        let tc = TestContext::new();
        let (board_id, _, _) = seed_board_with_subtree(&tc);
        let ctx = tc.as_command_context();
        ArchiveBoards {
            ids: vec![board_id],
        }
        .execute(&ctx)
        .unwrap();

        // Import a fresh board whose id collides with the archived one.
        let mut colliding = Board::new("Colliding", Some("COL"));
        colliding.id = board_id;
        let cmd = ImportEntities {
            boards: vec![colliding],
            columns: vec![],
            cards: vec![],
            archived_cards: vec![],
            sprints: vec![],
            graph: None,
        };
        let result = cmd.execute(&ctx);
        assert!(
            result.is_err(),
            "must reject collision with an archived board"
        );
        assert!(result.unwrap_err().is_validation());
    }
}
