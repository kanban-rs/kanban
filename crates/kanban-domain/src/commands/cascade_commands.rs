//! Cascade primitives.
//!
//! These commands are deliberately atomic and bypass the per-entity validation
//! that the standalone delete commands (e.g. [`DeleteColumn`](super::DeleteColumn))
//! enforce. They are intended to be composed by the cascade helpers in
//! [`super::cascade`] and executed as a single `KanbanContext::execute(...)`
//! batch so the whole cascade is one undo unit with snapshot/rollback.
//!
//! **Do not construct these commands directly outside the cascade module.** The
//! canonical entry points are the helpers in [`super::cascade`] which encode the
//! ordering invariants (graph edges → cards → archived → columns → sprints →
//! board) that make the bypassed validations safe.

use super::dependency_commands::edges_to_undo_commands;
use super::{BoardCommand, Command, CommandContext, ImportEntities};
use crate::data_store::DataStore;
use crate::{KanbanError, KanbanResult};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum CascadeCommand {
    DeleteCardEdges(DeleteCardEdges),
    DeleteCardsByColumns(DeleteCardsByColumns),
    DeleteArchivedCards(DeleteArchivedCards),
    DeleteColumnsByBoard(DeleteColumnsByBoard),
    DeleteSprintsByBoard(DeleteSprintsByBoard),
    /// Internal: set `sprint_id` on a list of archived cards. Used by
    /// `DeleteSprint`'s inverse to restore the binding that
    /// `clear_sprint_from_archived_cards` cleared. Not a user-facing
    /// command — accessed only via the inverse-capture path.
    SetArchivedCardsSprint(SetArchivedCardsSprint),
}

impl CascadeCommand {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        match self {
            CascadeCommand::DeleteCardEdges(c) => c.execute(context),
            CascadeCommand::DeleteCardsByColumns(c) => c.execute(context),
            CascadeCommand::DeleteArchivedCards(c) => c.execute(context),
            CascadeCommand::DeleteColumnsByBoard(c) => c.execute(context),
            CascadeCommand::DeleteSprintsByBoard(c) => c.execute(context),
            CascadeCommand::SetArchivedCardsSprint(c) => c.execute(context),
        }
    }

    pub fn description(&self) -> String {
        match self {
            CascadeCommand::DeleteCardEdges(c) => c.description(),
            CascadeCommand::DeleteCardsByColumns(c) => c.description(),
            CascadeCommand::DeleteArchivedCards(c) => c.description(),
            CascadeCommand::DeleteColumnsByBoard(c) => c.description(),
            CascadeCommand::DeleteSprintsByBoard(c) => c.description(),
            CascadeCommand::SetArchivedCardsSprint(c) => c.description(),
        }
    }

    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        match self {
            CascadeCommand::DeleteCardEdges(c) => c.capture_inverse(store),
            CascadeCommand::DeleteCardsByColumns(c) => c.capture_inverse(store),
            CascadeCommand::DeleteArchivedCards(c) => c.capture_inverse(store),
            CascadeCommand::DeleteColumnsByBoard(c) => c.capture_inverse(store),
            CascadeCommand::DeleteSprintsByBoard(c) => c.capture_inverse(store),
            CascadeCommand::SetArchivedCardsSprint(c) => c.capture_inverse(store),
        }
    }
}

/// Remove all dependency-graph edges for a batch of card IDs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteCardEdges {
    pub ids: Vec<Uuid>,
}

impl DeleteCardEdges {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        let ids = self.ids.clone();
        context.store.modify_graph(Box::new(move |graph| {
            for id in &ids {
                graph.remove_node(*id);
            }
            Ok(())
        }))
    }

    pub fn description(&self) -> String {
        format!("Remove {} card(s) from dependency graph", self.ids.len())
    }

    /// Inverse: capture every active edge involving any id in self.ids and
    /// emit the matching Add* / SetParent command for each.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let id_set: std::collections::HashSet<_> = self.ids.iter().copied().collect();
        let graph = store.get_graph()?;
        Ok(edges_to_undo_commands(&graph, |s, t| {
            id_set.contains(&s) || id_set.contains(&t)
        }))
    }
}

/// Delete all active cards belonging to the given columns.
///
/// Bypasses per-card validation. The dependency graph must be cleaned up
/// separately (see [`DeleteCardEdges`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteCardsByColumns {
    pub column_ids: Vec<Uuid>,
}

impl DeleteCardsByColumns {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        context.store.delete_cards_by_columns(&self.column_ids)
    }

    pub fn description(&self) -> String {
        format!("Delete all cards in {} column(s)", self.column_ids.len())
    }

    /// Inverse: capture every live card in the target columns and emit an
    /// `ImportEntities` that re-inserts them (the cascade's outer
    /// transaction already removed them by the time undo runs).
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let cards = store.list_cards_by_columns(&self.column_ids)?;
        if cards.is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![Command::Board(BoardCommand::Import(ImportEntities {
            cards,
            ..Default::default()
        }))])
    }
}

/// Delete a specific set of archived cards by id.
///
/// The board-delete cascade uses this to remove archived cards gathered by the
/// first-class `board_id` field (which catches records whose `original_column_id`
/// dangles because the column was deleted after archival). It does not re-derive
/// the target set from (possibly stale) column membership, so no record is leaked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteArchivedCards {
    pub card_ids: Vec<Uuid>,
}

impl DeleteArchivedCards {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        for id in &self.card_ids {
            context.store.delete_archived_card(*id)?;
        }
        Ok(())
    }

    pub fn description(&self) -> String {
        format!("Delete {} archived card(s)", self.card_ids.len())
    }

    /// Inverse: reference-marker model. `delete_archived_card` removes BOTH the
    /// marker and the underlying LIVE card row, so undo must re-import both — the
    /// live card (into `cards`) and its marker (into `archived_cards`) — to be the
    /// identity over the full card, including its edges. Captured before the
    /// forward execution runs, while both still exist.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let mut cards = Vec::new();
        let mut archived_cards = Vec::new();
        for id in &self.card_ids {
            if let Some(ac) = store.get_archived_card(*id)? {
                if let Some(card) = store.get_card(*id)? {
                    cards.push(card);
                }
                archived_cards.push(ac);
            }
        }
        if archived_cards.is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![Command::Board(BoardCommand::Import(ImportEntities {
            cards,
            archived_cards,
            ..Default::default()
        }))])
    }
}

/// Delete all columns belonging to the given board.
///
/// Bypasses the emptiness checks in [`super::DeleteColumn`]. The caller is
/// responsible for removing cards beforehand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteColumnsByBoard {
    pub board_id: Uuid,
}

impl DeleteColumnsByBoard {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        context.store.delete_columns_by_board(self.board_id)
    }

    pub fn description(&self) -> String {
        format!("Delete all columns in board {}", self.board_id)
    }

    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let columns = store.list_columns_by_board(self.board_id)?;
        if columns.is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![Command::Board(BoardCommand::Import(ImportEntities {
            columns,
            ..Default::default()
        }))])
    }
}

/// Delete all sprints belonging to the given board.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteSprintsByBoard {
    pub board_id: Uuid,
}

impl DeleteSprintsByBoard {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        context.store.delete_sprints_by_board(self.board_id)
    }

    pub fn description(&self) -> String {
        format!("Delete all sprints in board {}", self.board_id)
    }

    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let sprints = store.list_sprints_by_board(self.board_id)?;
        if sprints.is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![Command::Board(BoardCommand::Import(ImportEntities {
            sprints,
            ..Default::default()
        }))])
    }
}

/// Set `sprint_id` on every archived card in `archived_card_ids`.
/// Internal — only used by KAN-191 inverse-command capture (DeleteSprint
/// undo) to restore the binding that `clear_sprint_from_archived_cards`
/// cleared during forward execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetArchivedCardsSprint {
    pub archived_card_ids: Vec<Uuid>,
    pub sprint_id: Uuid,
}

impl SetArchivedCardsSprint {
    pub fn execute(&self, context: &CommandContext) -> KanbanResult<()> {
        // F3a (KAN-872): an archived card is an ordinary editable card, so
        // re-attach the sprint by editing the LIVE card rather than the old
        // delete-then-reinsert dance over the embedded `Archived::entity`. This
        // drops a `.entity` consumer ahead of the F3b collapse and stops abusing
        // the permanent-delete path as a transient step.
        for id in &self.archived_card_ids {
            if let Some(mut card) = context.store.get_card(*id)? {
                card.sprint_id = Some(self.sprint_id);
                context.store.upsert_card(card)?;
            }
        }
        Ok(())
    }

    pub fn description(&self) -> String {
        format!(
            "Re-attach sprint {} to {} archived card(s)",
            self.sprint_id,
            self.archived_card_ids.len()
        )
    }

    /// Synthetic-only. Rejects top-level execute so misuse fails
    /// loudly instead of producing a silently-broken undo entry.
    pub fn capture_inverse(&self, _store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        Err(KanbanError::Internal(format!(
            "SetArchivedCardsSprint is a synthetic command — it must only \
             appear inside an inverse batch (DeleteSprint undo), never as a \
             top-level forward command. Got {} card id(s) bound to sprint {}.",
            self.archived_card_ids.len(),
            self.sprint_id
        )))
    }
}
