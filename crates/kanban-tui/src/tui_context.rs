use crate::state::SaveCoordinator;
use kanban_domain::commands::Command;
use kanban_domain::KanbanResult;
use kanban_domain::{
    ArchivedCard, Board, BoardUpdate, Card, CardListFilter, CardSummary, CardUpdate, Column,
    ColumnUpdate, CreateCardOptions, GraphOperations, KanbanOperations, Sprint, SprintUpdate,
};
use kanban_service::backend::KanbanBackend;
use kanban_service::KanbanContext;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct TuiContext {
    inner: KanbanContext,
    pub save_coordinator: SaveCoordinator,
}

impl TuiContext {
    /// Build a `TuiContext` from a pre-built `KanbanContext`.
    /// The save coordinator is created based on whether the backend needs a save worker.
    #[allow(clippy::type_complexity)]
    pub fn new(
        ctx: KanbanContext,
    ) -> KanbanResult<(
        Self,
        Option<mpsc::Receiver<()>>,
        Option<mpsc::UnboundedReceiver<()>>,
    )> {
        let needs_save = ctx.backend().needs_save_worker();
        let (save_coordinator, save_rx, completion_rx) = SaveCoordinator::new(needs_save);
        let tui_ctx = Self {
            inner: ctx,
            save_coordinator,
        };
        Ok((tui_ctx, save_rx, completion_rx))
    }

    pub fn execute_command(&mut self, command: Command) -> KanbanResult<()> {
        self.execute_commands_batch(vec![command])
    }

    pub fn execute_commands_batch(&mut self, commands: Vec<Command>) -> KanbanResult<()> {
        self.inner.execute(commands)?;
        if self.save_coordinator.has_save_channel() {
            self.save_coordinator.queue_flush();
        }
        Ok(())
    }

    // --- Delegation: state methods ---

    pub fn undo(&mut self) -> KanbanResult<bool> {
        let result = self.inner.undo()?;
        if result && self.save_coordinator.has_save_channel() {
            self.save_coordinator.queue_flush();
        }
        Ok(result)
    }

    pub fn redo(&mut self) -> KanbanResult<bool> {
        let result = self.inner.redo()?;
        if result && self.save_coordinator.has_save_channel() {
            self.save_coordinator.queue_flush();
        }
        Ok(result)
    }

    pub fn can_undo(&self) -> bool {
        self.inner.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.inner.can_redo()
    }

    pub fn snapshot(&self) -> KanbanResult<kanban_domain::Snapshot> {
        self.inner.snapshot()
    }

    pub fn migrate_sprint_logs(&mut self) -> KanbanResult<usize> {
        let result = self.inner.migrate_sprint_logs()?;
        if result > 0 && self.save_coordinator.has_save_channel() {
            self.save_coordinator.queue_flush();
        }
        Ok(result)
    }

    pub fn apply_snapshot(&mut self, s: kanban_domain::Snapshot) -> KanbanResult<()> {
        self.inner.apply_snapshot(s)
    }

    pub fn mark_clean(&mut self) {
        self.inner.mark_clean()
    }

    pub fn mark_dirty(&mut self) {
        self.inner.mark_dirty()
    }

    pub fn is_dirty(&self) -> bool {
        self.inner.is_dirty()
    }

    pub fn clear_history(&mut self) -> KanbanResult<()> {
        self.inner.clear_history()
    }

    pub fn clear_conflict(&mut self) {
        self.inner.clear_conflict()
    }

    pub fn has_conflict(&self) -> bool {
        self.inner.has_conflict()
    }

    pub fn backend(&self) -> Arc<dyn KanbanBackend> {
        self.inner.backend()
    }

    pub fn replace_backend(&mut self, backend: Arc<dyn KanbanBackend>) {
        self.inner.replace_backend(backend)
    }

    pub async fn save(&self) -> KanbanResult<()> {
        self.inner.save().await
    }

    pub async fn reload(&mut self) -> KanbanResult<()> {
        self.inner.reload().await
    }

    pub fn data_store(&self) -> &dyn kanban_domain::DataStore {
        self.inner.data_store()
    }

    pub fn persistence_metadata(&self) -> Option<kanban_persistence::PersistenceMetadata> {
        self.inner.persistence_metadata()
    }

    #[cfg(any(test, feature = "test-helpers"))]
    pub fn inner_mut(&mut self) -> &mut KanbanContext {
        &mut self.inner
    }

    fn with_flush<T>(&mut self, result: KanbanResult<T>) -> KanbanResult<T> {
        if result.is_ok() && self.save_coordinator.has_save_channel() {
            self.save_coordinator.queue_flush();
        }
        result
    }
}

impl KanbanOperations for TuiContext {
    fn create_board(&mut self, name: String, card_prefix: Option<String>) -> KanbanResult<Board> {
        let r = self.inner.create_board(name, card_prefix);
        self.with_flush(r)
    }

    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.inner.list_boards()
    }

    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.inner.get_board(id)
    }

    fn update_board(&mut self, id: Uuid, updates: BoardUpdate) -> KanbanResult<Board> {
        let r = self.inner.update_board(id, updates);
        self.with_flush(r)
    }

    fn delete_board(&mut self, id: Uuid) -> KanbanResult<()> {
        let r = self.inner.delete_board(id);
        self.with_flush(r)
    }
    fn archive_board(&mut self, id: Uuid) -> KanbanResult<()> {
        let r = self.inner.archive_board(id);
        self.with_flush(r)
    }
    fn restore_board(&mut self, id: Uuid) -> KanbanResult<()> {
        let r = self.inner.restore_board(id);
        self.with_flush(r)
    }
    fn list_archived_boards(&self) -> KanbanResult<Vec<kanban_domain::ArchivedBoard>> {
        self.inner.list_archived_boards()
    }

    fn create_column(
        &mut self,
        board_id: Uuid,
        name: String,
        position: Option<i32>,
    ) -> KanbanResult<Column> {
        let r = self.inner.create_column(board_id, name, position);
        self.with_flush(r)
    }

    fn list_columns(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.inner.list_columns(board_id)
    }

    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.inner.get_column(id)
    }

    fn update_column(&mut self, id: Uuid, updates: ColumnUpdate) -> KanbanResult<Column> {
        let r = self.inner.update_column(id, updates);
        self.with_flush(r)
    }

    fn delete_column(&mut self, id: Uuid) -> KanbanResult<()> {
        let r = self.inner.delete_column(id);
        self.with_flush(r)
    }

    fn reorder_column(&mut self, id: Uuid, new_position: i32) -> KanbanResult<Column> {
        let r = self.inner.reorder_column(id, new_position);
        self.with_flush(r)
    }

    fn create_card(
        &mut self,
        board_id: Uuid,
        column_id: Uuid,
        title: String,
        options: CreateCardOptions,
    ) -> KanbanResult<Card> {
        let r = self.inner.create_card(board_id, column_id, title, options);
        self.with_flush(r)
    }

    fn list_cards(&self, filter: CardListFilter) -> KanbanResult<Vec<CardSummary>> {
        self.inner.list_cards(filter)
    }

    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        self.inner.get_card(id)
    }

    fn find_cards_by_identifier(&self, identifier: &str) -> KanbanResult<Vec<Card>> {
        self.inner.find_cards_by_identifier(identifier)
    }

    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        self.inner.list_all_cards()
    }

    fn list_all_columns(&self) -> KanbanResult<Vec<kanban_domain::Column>> {
        self.inner.list_all_columns()
    }

    fn list_all_sprints(&self) -> KanbanResult<Vec<kanban_domain::Sprint>> {
        self.inner.list_all_sprints()
    }

    fn update_card(&mut self, id: Uuid, updates: CardUpdate) -> KanbanResult<Card> {
        let r = self.inner.update_card(id, updates);
        self.with_flush(r)
    }

    fn move_card(
        &mut self,
        id: Uuid,
        column_id: Uuid,
        position: Option<i32>,
    ) -> KanbanResult<Card> {
        let r = self.inner.move_card(id, column_id, position);
        self.with_flush(r)
    }

    fn archive_card(&mut self, id: Uuid) -> KanbanResult<()> {
        let r = self.inner.archive_card(id);
        self.with_flush(r)
    }

    fn restore_card(&mut self, id: Uuid, column_id: Option<Uuid>) -> KanbanResult<Card> {
        let r = self.inner.restore_card(id, column_id);
        self.with_flush(r)
    }

    fn delete_card(&mut self, id: Uuid) -> KanbanResult<()> {
        let r = self.inner.delete_card(id);
        self.with_flush(r)
    }

    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.inner.list_archived_cards()
    }
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        self.inner.list_archived_cards_by_board(board_id)
    }

    fn assign_card_to_sprint(&mut self, card_id: Uuid, sprint_id: Uuid) -> KanbanResult<Card> {
        let r = self.inner.assign_card_to_sprint(card_id, sprint_id);
        self.with_flush(r)
    }

    fn unassign_card_from_sprint(&mut self, card_id: Uuid) -> KanbanResult<Card> {
        let r = self.inner.unassign_card_from_sprint(card_id);
        self.with_flush(r)
    }

    fn get_card_branch_name(&self, id: Uuid) -> KanbanResult<String> {
        self.inner.get_card_branch_name(id)
    }

    fn get_card_git_checkout(&self, id: Uuid) -> KanbanResult<String> {
        self.inner.get_card_git_checkout(id)
    }

    fn archive_cards(&mut self, ids: Vec<Uuid>) -> KanbanResult<usize> {
        let r = self.inner.archive_cards(ids);
        self.with_flush(r)
    }

    fn move_cards(&mut self, ids: Vec<Uuid>, column_id: Uuid) -> KanbanResult<usize> {
        let r = self.inner.move_cards(ids, column_id);
        self.with_flush(r)
    }

    fn update_cards(
        &mut self,
        updates: Vec<(Uuid, kanban_domain::CardUpdate)>,
    ) -> KanbanResult<usize> {
        let r = self.inner.update_cards(updates);
        self.with_flush(r)
    }

    fn assign_cards_to_sprint(&mut self, ids: Vec<Uuid>, sprint_id: Uuid) -> KanbanResult<usize> {
        let r = self.inner.assign_cards_to_sprint(ids, sprint_id);
        self.with_flush(r)
    }

    fn carry_over_sprint_cards(
        &mut self,
        from_sprint_id: Uuid,
        to_sprint_id: Uuid,
    ) -> KanbanResult<usize> {
        let r = self
            .inner
            .carry_over_sprint_cards(from_sprint_id, to_sprint_id);
        self.with_flush(r)
    }

    fn create_sprint(
        &mut self,
        board_id: Uuid,
        prefix: Option<String>,
        name: Option<String>,
    ) -> KanbanResult<Sprint> {
        let r = self.inner.create_sprint(board_id, prefix, name);
        self.with_flush(r)
    }

    fn list_sprints(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.inner.list_sprints(board_id)
    }

    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.inner.get_sprint(id)
    }

    fn update_sprint(&mut self, id: Uuid, updates: SprintUpdate) -> KanbanResult<Sprint> {
        let r = self.inner.update_sprint(id, updates);
        self.with_flush(r)
    }

    fn activate_sprint(&mut self, id: Uuid, duration_days: Option<i32>) -> KanbanResult<Sprint> {
        let r = self.inner.activate_sprint(id, duration_days);
        self.with_flush(r)
    }

    fn complete_sprint(&mut self, id: Uuid) -> KanbanResult<Sprint> {
        let r = self.inner.complete_sprint(id);
        self.with_flush(r)
    }

    fn cancel_sprint(&mut self, id: Uuid) -> KanbanResult<Sprint> {
        let r = self.inner.cancel_sprint(id);
        self.with_flush(r)
    }

    fn delete_sprint(&mut self, id: Uuid) -> KanbanResult<()> {
        let r = self.inner.delete_sprint(id);
        self.with_flush(r)
    }

    fn export_board(&self, board_id: Option<Uuid>) -> KanbanResult<String> {
        self.inner.export_board(board_id)
    }

    fn import_board(&mut self, data: &str) -> KanbanResult<Board> {
        let r = self.inner.import_board(data);
        self.with_flush(r)
    }
}

impl GraphOperations for TuiContext {
    fn attach_children(&mut self, parent: Uuid, children: Vec<Uuid>) -> KanbanResult<()> {
        let r = self.inner.attach_children(parent, children);
        self.with_flush(r)
    }
    fn detach_children(&mut self, parent: Uuid, children: Vec<Uuid>) -> KanbanResult<()> {
        let r = self.inner.detach_children(parent, children);
        self.with_flush(r)
    }
    fn list_children_of(&self, parent: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.inner.list_children_of(parent)
    }
    fn list_parents_of(&self, child: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.inner.list_parents_of(child)
    }
    fn block(
        &mut self,
        blocker: Uuid,
        blocked: Uuid,
        severity: kanban_domain::Severity,
    ) -> KanbanResult<()> {
        let r = self.inner.block(blocker, blocked, severity);
        self.with_flush(r)
    }
    fn unblock(&mut self, blocker: Uuid, blocked: Uuid) -> KanbanResult<()> {
        let r = self.inner.unblock(blocker, blocked);
        self.with_flush(r)
    }
    fn list_blocked_by(&self, blocker: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.inner.list_blocked_by(blocker)
    }
    fn list_blockers_of(&self, blocked: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.inner.list_blockers_of(blocked)
    }
    fn relate(&mut self, a: Uuid, b: Uuid, kind: kanban_domain::RelatesKind) -> KanbanResult<()> {
        let r = self.inner.relate(a, b, kind);
        self.with_flush(r)
    }
    fn dissociate(&mut self, a: Uuid, b: Uuid) -> KanbanResult<()> {
        let r = self.inner.dissociate(a, b);
        self.with_flush(r)
    }
    fn list_related_to(&self, card: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.inner.list_related_to(card)
    }
}
