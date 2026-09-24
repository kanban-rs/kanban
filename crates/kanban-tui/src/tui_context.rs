use crate::state::SaveCoordinator;
use kanban_domain::commands::Command;
use kanban_domain::KanbanResult;
use kanban_domain::{
    ArchivedCard, Board, BoardCreateOutcome, BoardListFilter, BoardUpdate, Card, CardCreateOutcome,
    CardListFilter, CardSummary, CardUpdate, Column, ColumnCreateOutcome, ColumnUpdate,
    CreateCardOptions, GraphOperations, Invalidation, KanbanOperations, MutationOperations,
    NewBoard, NewCard, NewColumn, RelatesKind, Severity, Sprint, SprintCreateOutcome, SprintUpdate,
    UndoOperations,
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

    pub fn execute_command(
        &mut self,
        command: Command,
    ) -> KanbanResult<kanban_domain::Invalidation> {
        self.execute_commands_batch(vec![command])
    }

    pub fn execute_commands_batch(
        &mut self,
        commands: Vec<Command>,
    ) -> KanbanResult<kanban_domain::Invalidation> {
        let inv = self.inner.execute(commands)?;
        if self.save_coordinator.has_save_channel() {
            self.save_coordinator.queue_flush();
        }
        Ok(inv)
    }

    pub fn execute_with(
        &mut self,
        build: impl FnOnce(&dyn kanban_domain::DataStore) -> KanbanResult<Vec<Command>>,
    ) -> KanbanResult<kanban_domain::Invalidation> {
        self.execute_with_extra(kanban_domain::EntityIds::default(), build)
    }

    pub fn execute_with_extra(
        &mut self,
        extra: kanban_domain::EntityIds,
        build: impl FnOnce(&dyn kanban_domain::DataStore) -> KanbanResult<Vec<Command>>,
    ) -> KanbanResult<kanban_domain::Invalidation> {
        let inv = self.inner.execute_with_extra(extra, build)?;
        if self.save_coordinator.has_save_channel() {
            self.save_coordinator.queue_flush();
        }
        Ok(inv)
    }

    // --- Delegation: state methods ---

    pub fn transfer_state_to(&self, target: &dyn KanbanBackend) -> KanbanResult<()> {
        self.inner.transfer_state_to(target)
    }

    pub fn export_all_boards(&self) -> KanbanResult<kanban_domain::export::AllBoardsExport> {
        self.inner.export_all_boards()
    }

    pub fn migrate_sprint_logs(&mut self) -> KanbanResult<usize> {
        let (result, _invalidation) = self.inner.migrate_sprint_logs()?;
        if result > 0 && self.save_coordinator.has_save_channel() {
            self.save_coordinator.queue_flush();
        }
        Ok(result)
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
        let _ = self.inner.replace_backend(backend);
    }

    pub async fn save(&self) -> KanbanResult<()> {
        self.inner.save().await
    }

    pub async fn reload(&mut self) -> KanbanResult<()> {
        self.inner.reload().await.map(|_| ())
    }

    pub fn data_store(&self) -> &dyn kanban_domain::DataStore {
        self.inner.data_store()
    }

    pub fn sync(
        &self,
        plan: &dyn kanban_service::FetchPlan,
        model: &mut kanban_domain::Model,
        proj: &mut impl kanban_domain::DerivedProjections,
    ) {
        self.inner.sync(plan, model, proj);
    }

    pub fn sync_invalidated(
        &self,
        inv: kanban_domain::Invalidation,
        plan: &dyn kanban_service::FetchPlan,
        model: &mut kanban_domain::Model,
        proj: &mut impl kanban_domain::DerivedProjections,
    ) {
        self.inner.sync_invalidated(inv, plan, model, proj);
    }

    pub fn resync_invalidated(
        &self,
        inv: kanban_domain::Invalidation,
        plan: &dyn kanban_service::FetchPlan,
        model: &mut kanban_domain::Model,
        proj: &mut impl kanban_domain::DerivedProjections,
    ) {
        self.inner.resync_invalidated(inv, plan, model, proj);
    }

    pub fn persistence_metadata(&self) -> Option<kanban_persistence::PersistenceMetadata> {
        self.inner.persistence_metadata()
    }

    pub fn app_config(&self) -> &kanban_service::AppConfig {
        self.inner.app_config()
    }

    pub fn set_app_config(&mut self, config: kanban_service::AppConfig) {
        self.inner.set_app_config(config);
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

impl MutationOperations for TuiContext {
    fn create_board_impl(
        &mut self,
        name: String,
        card_prefix: Option<String>,
    ) -> KanbanResult<(Board, Invalidation)> {
        let r = self.inner.create_board_impl(name, card_prefix);
        self.with_flush(r)
    }
    fn update_board_impl(
        &mut self,
        id: Uuid,
        updates: BoardUpdate,
    ) -> KanbanResult<(Board, Invalidation)> {
        let r = self.inner.update_board_impl(id, updates);
        self.with_flush(r)
    }
    fn delete_board_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation> {
        let r = self.inner.delete_board_impl(id);
        self.with_flush(r)
    }
    fn archive_board_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation> {
        let r = self.inner.archive_board_impl(id);
        self.with_flush(r)
    }
    fn restore_board_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation> {
        let r = self.inner.restore_board_impl(id);
        self.with_flush(r)
    }

    fn create_card_impl(
        &mut self,
        board_id: Uuid,
        column_id: Uuid,
        title: String,
        options: CreateCardOptions,
    ) -> KanbanResult<(Card, Invalidation)> {
        let r = self
            .inner
            .create_card_impl(board_id, column_id, title, options);
        self.with_flush(r)
    }
    fn update_card_impl(
        &mut self,
        id: Uuid,
        updates: CardUpdate,
    ) -> KanbanResult<(Card, Invalidation)> {
        let r = self.inner.update_card_impl(id, updates);
        self.with_flush(r)
    }
    fn move_card_impl(
        &mut self,
        id: Uuid,
        column_id: Uuid,
        position: Option<i32>,
    ) -> KanbanResult<(Card, Invalidation)> {
        let r = self.inner.move_card_impl(id, column_id, position);
        self.with_flush(r)
    }
    fn archive_card_impl(&mut self, id: Uuid) -> KanbanResult<((), Invalidation)> {
        let r = self.inner.archive_card_impl(id);
        self.with_flush(r)
    }
    fn restore_card_impl(
        &mut self,
        id: Uuid,
        column_id: Option<Uuid>,
    ) -> KanbanResult<(Card, Invalidation)> {
        let r = self.inner.restore_card_impl(id, column_id);
        self.with_flush(r)
    }
    fn delete_card_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation> {
        let r = self.inner.delete_card_impl(id);
        self.with_flush(r)
    }
    fn assign_card_to_sprint_impl(
        &mut self,
        card_id: Uuid,
        sprint_id: Uuid,
    ) -> KanbanResult<(Card, Invalidation)> {
        let r = self.inner.assign_card_to_sprint_impl(card_id, sprint_id);
        self.with_flush(r)
    }
    fn unassign_card_from_sprint_impl(
        &mut self,
        card_id: Uuid,
    ) -> KanbanResult<(Card, Invalidation)> {
        let r = self.inner.unassign_card_from_sprint_impl(card_id);
        self.with_flush(r)
    }

    fn archive_cards_impl(&mut self, ids: Vec<Uuid>) -> KanbanResult<(usize, Invalidation)> {
        let r = self.inner.archive_cards_impl(ids);
        self.with_flush(r)
    }
    fn move_cards_impl(
        &mut self,
        ids: Vec<Uuid>,
        column_id: Uuid,
    ) -> KanbanResult<(usize, Invalidation)> {
        let r = self.inner.move_cards_impl(ids, column_id);
        self.with_flush(r)
    }
    fn update_cards_impl(
        &mut self,
        updates: Vec<(Uuid, CardUpdate)>,
    ) -> KanbanResult<(usize, Invalidation)> {
        let r = self.inner.update_cards_impl(updates);
        self.with_flush(r)
    }
    fn assign_cards_to_sprint_impl(
        &mut self,
        ids: Vec<Uuid>,
        sprint_id: Uuid,
    ) -> KanbanResult<(usize, Invalidation)> {
        let r = self.inner.assign_cards_to_sprint_impl(ids, sprint_id);
        self.with_flush(r)
    }

    fn create_column_impl(
        &mut self,
        board_id: Uuid,
        name: String,
        position: Option<i32>,
    ) -> KanbanResult<(Column, Invalidation)> {
        let r = self.inner.create_column_impl(board_id, name, position);
        self.with_flush(r)
    }
    fn update_column_impl(
        &mut self,
        id: Uuid,
        updates: ColumnUpdate,
    ) -> KanbanResult<(Column, Invalidation)> {
        let r = self.inner.update_column_impl(id, updates);
        self.with_flush(r)
    }
    fn delete_column_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation> {
        let r = self.inner.delete_column_impl(id);
        self.with_flush(r)
    }
    fn reorder_column_impl(
        &mut self,
        id: Uuid,
        new_position: i32,
    ) -> KanbanResult<(Column, Invalidation)> {
        let r = self.inner.reorder_column_impl(id, new_position);
        self.with_flush(r)
    }

    fn carry_over_sprint_cards_impl(
        &mut self,
        from_sprint_id: Uuid,
        to_sprint_id: Uuid,
    ) -> KanbanResult<(usize, Invalidation)> {
        let r = self
            .inner
            .carry_over_sprint_cards_impl(from_sprint_id, to_sprint_id);
        self.with_flush(r)
    }
    fn create_sprint_impl(
        &mut self,
        board_id: Uuid,
        prefix: Option<String>,
        name: Option<String>,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        let r = self.inner.create_sprint_impl(board_id, prefix, name);
        self.with_flush(r)
    }
    fn update_sprint_impl(
        &mut self,
        id: Uuid,
        updates: SprintUpdate,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        let r = self.inner.update_sprint_impl(id, updates);
        self.with_flush(r)
    }
    fn activate_sprint_impl(
        &mut self,
        id: Uuid,
        duration_days: Option<i32>,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        let r = self.inner.activate_sprint_impl(id, duration_days);
        self.with_flush(r)
    }
    fn complete_sprint_impl(&mut self, id: Uuid) -> KanbanResult<(Sprint, Invalidation)> {
        let r = self.inner.complete_sprint_impl(id);
        self.with_flush(r)
    }
    fn cancel_sprint_impl(&mut self, id: Uuid) -> KanbanResult<(Sprint, Invalidation)> {
        let r = self.inner.cancel_sprint_impl(id);
        self.with_flush(r)
    }
    fn delete_sprint_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation> {
        let r = self.inner.delete_sprint_impl(id);
        self.with_flush(r)
    }
    fn import_board_impl(&mut self, data: &str) -> KanbanResult<(Board, Invalidation)> {
        let r = self.inner.import_board_impl(data);
        self.with_flush(r)
    }

    fn attach_children_impl(
        &mut self,
        parent: Uuid,
        children: Vec<Uuid>,
    ) -> KanbanResult<Invalidation> {
        let r = self.inner.attach_children_impl(parent, children);
        self.with_flush(r)
    }
    fn detach_children_impl(
        &mut self,
        parent: Uuid,
        children: Vec<Uuid>,
    ) -> KanbanResult<Invalidation> {
        let r = self.inner.detach_children_impl(parent, children);
        self.with_flush(r)
    }
    fn block_impl(
        &mut self,
        blocker: Uuid,
        blocked: Uuid,
        severity: Severity,
    ) -> KanbanResult<Invalidation> {
        let r = self.inner.block_impl(blocker, blocked, severity);
        self.with_flush(r)
    }
    fn unblock_impl(&mut self, blocker: Uuid, blocked: Uuid) -> KanbanResult<Invalidation> {
        let r = self.inner.unblock_impl(blocker, blocked);
        self.with_flush(r)
    }
    fn relate_impl(&mut self, a: Uuid, b: Uuid, kind: RelatesKind) -> KanbanResult<Invalidation> {
        let r = self.inner.relate_impl(a, b, kind);
        self.with_flush(r)
    }
    fn dissociate_impl(&mut self, a: Uuid, b: Uuid) -> KanbanResult<Invalidation> {
        let r = self.inner.dissociate_impl(a, b);
        self.with_flush(r)
    }

    fn create_board_from_spec(
        &mut self,
        id: Option<Uuid>,
        spec: NewBoard,
    ) -> KanbanResult<(Board, Invalidation)> {
        let r = self.inner.create_board_from_spec(id, spec);
        self.with_flush(r)
    }
    fn create_or_replace_board(
        &mut self,
        id: Uuid,
        spec: NewBoard,
    ) -> KanbanResult<(BoardCreateOutcome, Invalidation)> {
        let r = self.inner.create_or_replace_board(id, spec);
        self.with_flush(r)
    }
    fn create_card_from_spec(
        &mut self,
        client_id: Option<Uuid>,
        spec: NewCard,
    ) -> KanbanResult<(Card, Invalidation)> {
        let r = self.inner.create_card_from_spec(client_id, spec);
        self.with_flush(r)
    }
    fn create_or_replace_card(
        &mut self,
        id: Uuid,
        spec: NewCard,
    ) -> KanbanResult<(CardCreateOutcome, Invalidation)> {
        let r = self.inner.create_or_replace_card(id, spec);
        self.with_flush(r)
    }
    fn create_column_from_spec(
        &mut self,
        id: Option<Uuid>,
        spec: NewColumn,
    ) -> KanbanResult<(Column, Invalidation)> {
        let r = self.inner.create_column_from_spec(id, spec);
        self.with_flush(r)
    }
    fn create_or_replace_column(
        &mut self,
        id: Uuid,
        spec: NewColumn,
        position: Option<i32>,
    ) -> KanbanResult<(ColumnCreateOutcome, Invalidation)> {
        let r = self.inner.create_or_replace_column(id, spec, position);
        self.with_flush(r)
    }
    fn create_sprint_from_spec(
        &mut self,
        board_id: Uuid,
        id: Option<Uuid>,
        name: Option<String>,
        prefix: Option<String>,
        auto_consume_name: bool,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        let r = self
            .inner
            .create_sprint_from_spec(board_id, id, name, prefix, auto_consume_name);
        self.with_flush(r)
    }
    fn create_or_replace_sprint(
        &mut self,
        board_id: Uuid,
        id: Uuid,
        name: Option<String>,
        prefix: Option<String>,
        auto_consume_name: bool,
    ) -> KanbanResult<(SprintCreateOutcome, Invalidation)> {
        let r = self
            .inner
            .create_or_replace_sprint(board_id, id, name, prefix, auto_consume_name);
        self.with_flush(r)
    }

    fn execute(&mut self, commands: Vec<Command>) -> KanbanResult<Invalidation> {
        let r = self.inner.execute(commands);
        self.with_flush(r)
    }
}

impl UndoOperations for TuiContext {
    fn undo(&mut self) -> KanbanResult<Option<Invalidation>> {
        let inv = self.inner.undo()?;
        if inv.is_some() && self.save_coordinator.has_save_channel() {
            self.save_coordinator.queue_flush();
        }
        Ok(inv)
    }

    fn redo(&mut self) -> KanbanResult<Option<Invalidation>> {
        let inv = self.inner.redo()?;
        if inv.is_some() && self.save_coordinator.has_save_channel() {
            self.save_coordinator.queue_flush();
        }
        Ok(inv)
    }

    fn can_undo(&self) -> bool {
        self.inner.can_undo()
    }

    fn can_redo(&self) -> bool {
        self.inner.can_redo()
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

    fn list_boards_filtered(&self, filter: BoardListFilter) -> KanbanResult<Vec<Board>> {
        self.inner.list_boards_filtered(filter)
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
