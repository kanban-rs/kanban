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

    /// Inverse: read each still-present archived card (captured before the
    /// forward execution runs) and re-import it.
    pub fn capture_inverse(&self, store: &dyn DataStore) -> KanbanResult<Vec<Command>> {
        let mut archived_cards = Vec::new();
        for id in &self.card_ids {
            if let Some(ac) = store.get_archived_card(*id)? {
                archived_cards.push(ac);
            }
        }
        if archived_cards.is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![Command::Board(BoardCommand::Import(ImportEntities {
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
        for id in &self.archived_card_ids {
            if let Some(mut ac) = context.store.get_archived_card(*id)? {
                ac.entity.sprint_id = Some(self.sprint_id);
                context.store.delete_archived_card(ac.entity.id)?;
                context.store.insert_archived_card(ac)?;
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

#[cfg(test)]
mod tests {
    use super::super::test_helpers::TestContext;
    use super::*;
    use crate::DataStore;

    #[test]
    fn test_delete_card_edges_removes_all_edges_for_given_ids() {
        let tc = TestContext::new();
        let card_a = Uuid::new_v4();
        let card_b = Uuid::new_v4();
        let card_c = Uuid::new_v4();

        {
            let mut graph = tc.store.get_graph().unwrap();
            graph.set_block(card_a, card_b).unwrap();
            graph.set_block(card_b, card_c).unwrap();
            tc.store.set_graph(graph).unwrap();
        }
        assert_eq!(tc.store.get_graph().unwrap().len(), 2);

        let context = tc.as_command_context();
        let cmd = DeleteCardEdges {
            ids: vec![card_a, card_b],
        };
        cmd.execute(&context).unwrap();

        let graph = tc.store.get_graph().unwrap();
        assert_eq!(
            graph.len(),
            0,
            "edges incident to card_a or card_b should be removed"
        );
    }

    #[test]
    fn test_delete_card_edges_with_empty_input_is_noop() {
        let tc = TestContext::new();
        let card_a = Uuid::new_v4();
        let card_b = Uuid::new_v4();
        {
            let mut graph = tc.store.get_graph().unwrap();
            graph.set_block(card_a, card_b).unwrap();
            tc.store.set_graph(graph).unwrap();
        }

        let context = tc.as_command_context();
        let cmd = DeleteCardEdges { ids: vec![] };
        cmd.execute(&context).unwrap();

        assert_eq!(tc.store.get_graph().unwrap().len(), 1);
    }

    #[test]
    fn test_delete_cards_by_columns_removes_only_cards_in_given_columns() {
        let tc = TestContext::new();
        let mut board = crate::Board::new("B", Some("TST"));
        let col1 = crate::Column::new(board.id, "C1", 0);
        let col2 = crate::Column::new(board.id, "C2", 1);
        let col3 = crate::Column::new(board.id, "C3", 2);
        let card1 = crate::Card::new(&mut board, col1.id, "1", 0);
        let card2 = crate::Card::new(&mut board, col2.id, "2", 0);
        let card3 = crate::Card::new(&mut board, col3.id, "3", 0);
        let card3_id = card3.id;
        let col3_id = col3.id;
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_column(col1.clone()).unwrap();
        tc.store.upsert_column(col2.clone()).unwrap();
        tc.store.upsert_column(col3).unwrap();
        tc.store.upsert_card(card1).unwrap();
        tc.store.upsert_card(card2).unwrap();
        tc.store.upsert_card(card3).unwrap();

        let context = tc.as_command_context();
        let cmd = DeleteCardsByColumns {
            column_ids: vec![col1.id, col2.id],
        };
        cmd.execute(&context).unwrap();

        let remaining = tc.store.list_all_cards().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, card3_id);
        assert_eq!(remaining[0].column_id, col3_id);
    }

    #[test]
    fn test_delete_archived_cards_removes_only_listed_ids_ignoring_columns() {
        // Board-scoped id list must delete archived records even when their
        // `original_column_id` dangles (column already gone).
        let tc = TestContext::new();
        let board_id = Uuid::new_v4();
        let mut board = crate::Board::new("B", Some("TST"));
        let dangling_col = Uuid::new_v4();
        let live_col = crate::Column::new(board_id, "Live", 0);
        let card1 = crate::Card::new(&mut board, dangling_col, "1", 0);
        let card2 = crate::Card::new(&mut board, live_col.id, "2", 0);
        let keep = crate::Card::new(&mut board, live_col.id, "keep", 1);
        let arch1 = crate::ArchivedCard::new(card1, board_id, dangling_col, 0);
        let arch2 = crate::ArchivedCard::new(card2, board_id, live_col.id, 0);
        let keep_arch = crate::ArchivedCard::new(keep, board_id, live_col.id, 1);
        let arch1_id = arch1.entity.id;
        let arch2_id = arch2.entity.id;
        let keep_id = keep_arch.entity.id;
        tc.store.insert_archived_card(arch1).unwrap();
        tc.store.insert_archived_card(arch2).unwrap();
        tc.store.insert_archived_card(keep_arch).unwrap();

        let context = tc.as_command_context();
        let cmd = DeleteArchivedCards {
            card_ids: vec![arch1_id, arch2_id],
        };
        cmd.execute(&context).unwrap();

        let remaining = tc.store.list_archived_cards().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].entity.id, keep_id);
    }

    #[test]
    fn test_delete_columns_by_board_removes_all_columns_of_board() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", None::<String>);
        let board_id = board.id;
        let other_board = crate::Board::new("Other", None::<String>);
        let other_board_id = other_board.id;
        let col1 = crate::Column::new(board_id, "C1", 0);
        let col2 = crate::Column::new(board_id, "C2", 1);
        let other_col = crate::Column::new(other_board_id, "OC", 0);
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_board(other_board).unwrap();
        tc.store.upsert_column(col1).unwrap();
        tc.store.upsert_column(col2).unwrap();
        tc.store.upsert_column(other_col).unwrap();

        let context = tc.as_command_context();
        let cmd = DeleteColumnsByBoard { board_id };
        cmd.execute(&context).unwrap();

        let remaining = tc.store.list_all_columns().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].board_id, other_board_id);
    }

    #[test]
    fn test_delete_sprints_by_board_removes_all_sprints_of_board() {
        let tc = TestContext::new();
        let board = crate::Board::new("B", None::<String>);
        let board_id = board.id;
        let other_board = crate::Board::new("Other", None::<String>);
        let other_board_id = other_board.id;
        let sprint1 = crate::Sprint::new(board_id, 1, None, None::<String>);
        let sprint2 = crate::Sprint::new(board_id, 2, None, None::<String>);
        let other_sprint = crate::Sprint::new(other_board_id, 1, None, None::<String>);
        tc.store.upsert_board(board).unwrap();
        tc.store.upsert_board(other_board).unwrap();
        tc.store.upsert_sprint(sprint1).unwrap();
        tc.store.upsert_sprint(sprint2).unwrap();
        tc.store.upsert_sprint(other_sprint).unwrap();

        let context = tc.as_command_context();
        let cmd = DeleteSprintsByBoard { board_id };
        cmd.execute(&context).unwrap();

        let remaining = tc.store.list_all_sprints().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].board_id, other_board_id);
    }
}
