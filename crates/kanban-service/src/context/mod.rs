use crate::backend::KanbanBackend;
use kanban_core::{AppConfig, AppType};
use kanban_domain::{
    ArchivedCard, Board, BoardUpdate, Card, CardListFilter, CardSummary, CardUpdate, Column,
    ColumnUpdate, CreateCardOptions, KanbanOperations, KanbanResult, Sprint, SprintUpdate,
};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

mod boards;
pub use boards::BoardCreateOutcome;
mod cards;
pub use cards::CardCreateOutcome;
mod cards_batch;
mod cards_batch_detailed;
mod columns;
pub use columns::ColumnCreateOutcome;
mod core;
mod filters;
mod graph;
mod persistence;
mod sprints;
pub use sprints::SprintCreateOutcome;
mod undo;

#[derive(Debug, Clone, Serialize)]
pub struct BatchOperationResult {
    pub succeeded: Vec<Uuid>,
    pub failed: Vec<BatchOperationFailure>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchOperationFailure {
    pub id: Uuid,
    pub error: String,
}

/// Service layer: wraps a pluggable [`KanbanBackend`] with undo/redo history
/// and a unified async `save()` / `reload()` interface.
///
/// Construction is always zero-I/O — data is fetched lazily on the first
/// read, either directly (SQLite, reads are always live) or via a one-time
/// cache-fill on first access (JSON).
///
/// # Undo / Redo model
///
/// Every undoable command captures an **inverse** at execute time. The
/// `(forward, inverse)` pair lives on the per-session [`UndoStack`].
/// Undo executes the inverse against current state through the normal
/// command-execute path — no snapshot apply, no replay. Redo re-executes
/// the forward batch.
///
/// `execute` also appends the forward batch to the `CommandStore` audit
/// log (`backend.append_batch`). The audit log is informational — it
/// records what happened; it does not drive undo. Audit-log UI is KAN-36.
pub struct KanbanContext {
    pub(super) backend: Arc<dyn KanbanBackend>,
    pub(super) app_config: AppConfig,
    /// Per-session inverse-command undo state.
    pub(super) undo_stack: crate::undo_stack::UndoStack,
    pub(super) dirty: bool,
    pub(super) conflict_pending: bool,
    /// Generated once at open_deferred; stable for this context's lifetime.
    pub(super) session_id: Uuid,
    /// Which application surface owns this context. Default: Unknown.
    pub(super) app_type: AppType,
}

// The `KanbanOperations` trait impl must live in a single block (Rust forbids
// splitting a trait impl across files), so it forwards to the entity-focused
// inherent methods in `boards`/`columns`/`cards`/`cards_batch`/`sprints`.
impl KanbanOperations for KanbanContext {
    fn create_board(&mut self, name: String, card_prefix: Option<String>) -> KanbanResult<Board> {
        KanbanContext::create_board_impl(self, name, card_prefix)
    }
    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        KanbanContext::list_boards_impl(self)
    }
    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        KanbanContext::get_board_impl(self, id)
    }
    fn update_board(&mut self, id: Uuid, updates: BoardUpdate) -> KanbanResult<Board> {
        KanbanContext::update_board_impl(self, id, updates)
    }
    fn delete_board(&mut self, id: Uuid) -> KanbanResult<()> {
        KanbanContext::delete_board_impl(self, id)
    }

    fn create_column(
        &mut self,
        board_id: Uuid,
        name: String,
        position: Option<i32>,
    ) -> KanbanResult<Column> {
        KanbanContext::create_column_impl(self, board_id, name, position)
    }
    fn list_columns(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        KanbanContext::list_columns_impl(self, board_id)
    }
    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        KanbanContext::get_column_impl(self, id)
    }
    fn update_column(&mut self, id: Uuid, updates: ColumnUpdate) -> KanbanResult<Column> {
        KanbanContext::update_column_impl(self, id, updates)
    }
    fn delete_column(&mut self, id: Uuid) -> KanbanResult<()> {
        KanbanContext::delete_column_impl(self, id)
    }
    fn reorder_column(&mut self, id: Uuid, new_position: i32) -> KanbanResult<Column> {
        KanbanContext::reorder_column_impl(self, id, new_position)
    }

    fn create_card(
        &mut self,
        board_id: Uuid,
        column_id: Uuid,
        title: String,
        options: CreateCardOptions,
    ) -> KanbanResult<Card> {
        KanbanContext::create_card_impl(self, board_id, column_id, title, options)
    }
    fn list_cards(&self, filter: CardListFilter) -> KanbanResult<Vec<CardSummary>> {
        KanbanContext::list_cards_impl(self, filter)
    }
    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        KanbanContext::get_card_impl(self, id)
    }
    fn find_cards_by_identifier(&self, identifier: &str) -> KanbanResult<Vec<Card>> {
        KanbanContext::find_cards_by_identifier_impl(self, identifier)
    }
    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        KanbanContext::list_all_cards_impl(self)
    }
    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        KanbanContext::list_all_columns_impl(self)
    }
    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        KanbanContext::list_all_sprints_impl(self)
    }
    fn update_card(&mut self, id: Uuid, updates: CardUpdate) -> KanbanResult<Card> {
        KanbanContext::update_card_impl(self, id, updates)
    }
    fn move_card(
        &mut self,
        id: Uuid,
        column_id: Uuid,
        position: Option<i32>,
    ) -> KanbanResult<Card> {
        KanbanContext::move_card_impl(self, id, column_id, position)
    }
    fn archive_card(&mut self, id: Uuid) -> KanbanResult<()> {
        KanbanContext::archive_card_impl(self, id)
    }
    fn restore_card(&mut self, id: Uuid, column_id: Option<Uuid>) -> KanbanResult<Card> {
        KanbanContext::restore_card_impl(self, id, column_id)
    }
    fn delete_card(&mut self, id: Uuid) -> KanbanResult<()> {
        KanbanContext::delete_card_impl(self, id)
    }
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        KanbanContext::list_archived_cards_impl(self)
    }
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        KanbanContext::list_archived_cards_by_board_impl(self, board_id)
    }

    fn assign_card_to_sprint(&mut self, card_id: Uuid, sprint_id: Uuid) -> KanbanResult<Card> {
        KanbanContext::assign_card_to_sprint_impl(self, card_id, sprint_id)
    }
    fn unassign_card_from_sprint(&mut self, card_id: Uuid) -> KanbanResult<Card> {
        KanbanContext::unassign_card_from_sprint_impl(self, card_id)
    }

    fn get_card_branch_name(&self, id: Uuid) -> KanbanResult<String> {
        KanbanContext::get_card_branch_name_impl(self, id)
    }
    fn get_card_git_checkout(&self, id: Uuid) -> KanbanResult<String> {
        KanbanContext::get_card_git_checkout_impl(self, id)
    }

    fn archive_cards(&mut self, ids: Vec<Uuid>) -> KanbanResult<usize> {
        KanbanContext::archive_cards_impl(self, ids)
    }
    fn move_cards(&mut self, ids: Vec<Uuid>, column_id: Uuid) -> KanbanResult<usize> {
        KanbanContext::move_cards_impl(self, ids, column_id)
    }
    fn update_cards(&mut self, updates: Vec<(Uuid, CardUpdate)>) -> KanbanResult<usize> {
        KanbanContext::update_cards_impl(self, updates)
    }
    fn assign_cards_to_sprint(&mut self, ids: Vec<Uuid>, sprint_id: Uuid) -> KanbanResult<usize> {
        KanbanContext::assign_cards_to_sprint_impl(self, ids, sprint_id)
    }
    fn carry_over_sprint_cards(
        &mut self,
        from_sprint_id: Uuid,
        to_sprint_id: Uuid,
    ) -> KanbanResult<usize> {
        KanbanContext::carry_over_sprint_cards_impl(self, from_sprint_id, to_sprint_id)
    }

    fn create_sprint(
        &mut self,
        board_id: Uuid,
        prefix: Option<String>,
        name: Option<String>,
    ) -> KanbanResult<Sprint> {
        KanbanContext::create_sprint_impl(self, board_id, prefix, name)
    }
    fn list_sprints(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        KanbanContext::list_sprints_impl(self, board_id)
    }
    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        KanbanContext::get_sprint_impl(self, id)
    }
    fn update_sprint(&mut self, id: Uuid, updates: SprintUpdate) -> KanbanResult<Sprint> {
        KanbanContext::update_sprint_impl(self, id, updates)
    }
    fn activate_sprint(&mut self, id: Uuid, duration_days: Option<i32>) -> KanbanResult<Sprint> {
        KanbanContext::activate_sprint_impl(self, id, duration_days)
    }
    fn complete_sprint(&mut self, id: Uuid) -> KanbanResult<Sprint> {
        KanbanContext::complete_sprint_impl(self, id)
    }
    fn cancel_sprint(&mut self, id: Uuid) -> KanbanResult<Sprint> {
        KanbanContext::cancel_sprint_impl(self, id)
    }
    fn delete_sprint(&mut self, id: Uuid) -> KanbanResult<()> {
        KanbanContext::delete_sprint_impl(self, id)
    }

    fn export_board(&self, board_id: Option<Uuid>) -> KanbanResult<String> {
        KanbanContext::export_board_impl(self, board_id)
    }
    fn import_board(&mut self, data: &str) -> KanbanResult<Board> {
        KanbanContext::import_board_impl(self, data)
    }
}
