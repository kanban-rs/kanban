use super::{Command, CommandContext};
use crate::data_store::DataStore;
use crate::field_update::FieldUpdate;
use crate::KanbanResult;
use crate::{
    ArchivedBoard, ArchivedCard, Board, Card, Column, DependencyGraph, KanbanError, NewBoard,
    Sprint,
};
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
            completion_column_ids: Vec::new(),
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
        let prior_completion_column_ids = board.completion_column_ids.clone();
        board.update(self.updates.clone());
        if let Some(next_ids) = self.updates.completion_column_ids.as_ref() {
            super::completion_status_sync::sync_default_status(
                context,
                &prior_completion_column_ids,
                next_ids,
            )?;
        }
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
            completion_column_ids: upd
                .completion_column_ids
                .as_ref()
                .map(|_| board.completion_column_ids.clone()),
            position: upd.position.map(|_| board.position),
        };
        let mut commands = vec![Command::Board(BoardCommand::Update(UpdateBoard {
            board_id: self.board_id,
            updates: inverse,
        }))];
        if let Some(next_ids) = upd.completion_column_ids.as_ref() {
            let touched = super::completion_status_sync::snapshot_touched_columns(
                store,
                &board.completion_column_ids,
                next_ids,
            )?;
            commands.extend(touched.into_iter().map(|(column_id, prior_status)| {
                Command::Column(super::ColumnCommand::Update(super::UpdateColumn {
                    column_id,
                    updates: crate::ColumnUpdate {
                        default_status: Some(prior_status),
                        ..Default::default()
                    },
                }))
            }));
        }
        Ok(commands)
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
    /// Delete the board record from wherever it lives — the live `boards` set
    /// OR the discrete `archived_boards` collection (both deletes are
    /// idempotent). Collection-agnostic, mirroring `DeleteCard`. **Atomic
    /// only** — does not cascade to columns, cards, sprints, or graph edges;
    /// cascade orchestration is the service layer's job (see
    /// `KanbanContext::delete_board` / `delete_archived_board`).
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        context.store.delete_board(self.board_id)?;
        context.store.delete_archived_board(self.board_id)?;
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Delete board: {}", self.board_id)
    }

    /// Inverse: re-insert the deleted board into the collection it came from —
    /// a live board via `ImportEntities.boards`, an archived board via
    /// `ImportEntities.archived_boards` (so undo of a permanent-delete restores
    /// it AS archived). The cascade siblings capture their own subtree
    /// entities, so undoing the full cascade restores everything.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        // `get_board` is unfiltered (returns archived boards too), so it can no
        // longer discriminate. The MARKER is the discriminator: a board with an
        // archival marker was archived, and its undo must re-import it AS
        // archived (board row + marker) so archived-ness survives the round-trip.
        let board = store
            .get_board(self.board_id)?
            .ok_or_else(|| KanbanError::not_found("Board", self.board_id))?;
        if let Some(marker) = store.get_archived_board(self.board_id)? {
            return Ok(vec![Command::Board(BoardCommand::Import(ImportEntities {
                boards: vec![board],
                archived_boards: vec![marker],
                ..Default::default()
            }))]);
        }
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
            // Reference-marker model: the board head STAYS live in `boards`; we
            // only record an archival marker (which hides it from live queries).
            // Fetch to guard existence (get_board is unfiltered, so an already
            // archived board also resolves — re-archiving is idempotent).
            let board = context
                .store
                .get_board(*id)?
                .ok_or_else(|| KanbanError::not_found("Board", *id))?;
            context
                .store
                .insert_archived_board(crate::Archived::now(board.id))?;
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
        // Guard existence, then drop the archival marker. Under the
        // reference-marker model the board head already lives in `boards`;
        // dropping the marker makes it visible to live queries again. Use
        // `unarchive_board`, NOT `delete_archived_board`: on a shared-row backend
        // (SQLite) the latter deletes the entity ROW and would CASCADE the
        // still-present subtree away (KAN-863). `unarchive_board` removes only
        // the marker, keeping the row + subtree.
        if context.store.get_archived_board(self.board_id)?.is_none() {
            return Err(KanbanError::not_found("archived board", self.board_id));
        }
        context.store.unarchive_board(self.board_id)?;
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
        let columns = context.store.list_columns_by_board(self.board_id)?;
        board.validate_completion_columns(&self.dto.completion_column_ids, &columns)?;
        let prior_completion_column_ids = board.completion_column_ids.clone();
        self.dto.clone().apply_to(&mut board);
        super::completion_status_sync::sync_default_status(
            context,
            &prior_completion_column_ids,
            &board.completion_column_ids,
        )?;
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
    ///
    /// Re-dispatching `ApplyBoardSettings` alone is not enough for
    /// `completion_column_ids`: its forward `execute` re-syncs every touched
    /// column's `default_status` against the DTO's list, so a column that had
    /// a deliberate non-`Done` status before the original change would land on
    /// `Todo` (the sync's default for a removed column) instead of its prior
    /// value. Snapshot every touched column here, before `execute` runs, and
    /// append an explicit restore per column so it wins over the re-sync.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let board = match store.get_board(self.board_id)? {
            Some(b) => b,
            None => return Err(KanbanError::not_found("Board", self.board_id)),
        };
        let prior_dto = crate::editable::BoardSettingsDto::from_entity(&board);
        let mut commands = vec![Command::Board(BoardCommand::ApplySettings(
            ApplyBoardSettings {
                board_id: self.board_id,
                dto: prior_dto,
            },
        ))];
        let touched = super::completion_status_sync::snapshot_touched_columns(
            store,
            &board.completion_column_ids,
            &self.dto.completion_column_ids,
        )?;
        commands.extend(touched.into_iter().map(|(column_id, prior_status)| {
            Command::Column(super::ColumnCommand::Update(super::UpdateColumn {
                column_id,
                updates: crate::ColumnUpdate {
                    default_status: Some(prior_status),
                    ..Default::default()
                },
            }))
        }));
        Ok(commands)
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
    /// Archived board records (C3a). `#[serde(default)]` keeps older
    /// command-log entries (written before this field existed) deserializable.
    #[serde(default)]
    pub archived_boards: Vec<ArchivedBoard>,
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
                    .map(|ab| ab.entity_id),
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
            .map(|ac| ac.entity_id)
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
            if existing_archived_ids.contains(&ac.entity_id)
                || existing_card_ids.contains(&ac.entity_id)
            {
                return Err(crate::KanbanError::validation(format!(
                    "Duplicate archived card ID (live or archived): {}",
                    ac.entity_id
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
        // `existing_board_ids` already spans live + archived boards, so this
        // rejects an archived-board import colliding with either.
        for ab in &self.archived_boards {
            if existing_board_ids.contains(&ab.entity_id) {
                return Err(crate::KanbanError::validation(format!(
                    "Duplicate board ID (live or archived): {}",
                    ab.entity_id
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
            context.store.insert_archived_card(*ac)?;
        }
        for ab in &self.archived_boards {
            context.store.insert_archived_board(*ab)?;
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
                    card_id: ac.entity_id,
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

        // Boards last. `DeleteBoard` is collection-agnostic, so the same
        // command undoes an imported live board or an imported archived board.
        for b in &self.boards {
            commands.push(Command::Board(BoardCommand::Delete(DeleteBoard {
                board_id: b.id,
            })));
        }
        for ab in &self.archived_boards {
            commands.push(Command::Board(BoardCommand::Delete(DeleteBoard {
                board_id: ab.entity_id,
            })));
        }

        Ok(commands)
    }
}
