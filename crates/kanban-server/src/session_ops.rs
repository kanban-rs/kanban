//! `Session`'s own capability surface: adding a method to `KanbanOperations`,
//! `GraphOperations`, or `UndoOperations` must be answered here, not
//! silently inherited through `Deref` to `KanbanContext`.

use crate::state::{mutate, mutate_unit, Session};
use kanban_domain::{
    ArchivedBoard, ArchivedCard, Board, BoardListFilter, BoardUpdate, Card, CardListFilter,
    CardSummary, CardUpdate, Column, ColumnUpdate, CreateCardOptions, GraphOperations,
    Invalidation, KanbanError, KanbanOperations, KanbanResult, RelatesKind, Severity, Sprint,
    SprintUpdate, UndoOperations,
};
use uuid::Uuid;

impl KanbanOperations for Session {
    fn create_board(&mut self, name: String, card_prefix: Option<String>) -> KanbanResult<Board> {
        mutate(self, |c| c.create_board_impl(name, card_prefix)).map(|(value, _)| value)
    }
    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.ctx.list_boards()
    }
    fn list_boards_filtered(&self, filter: BoardListFilter) -> KanbanResult<Vec<Board>> {
        self.ctx.list_boards_filtered(filter)
    }
    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.ctx.get_board(id)
    }
    fn update_board(&mut self, id: Uuid, updates: BoardUpdate) -> KanbanResult<Board> {
        mutate(self, |c| c.update_board_impl(id, updates)).map(|(value, _)| value)
    }
    fn delete_board(&mut self, id: Uuid) -> KanbanResult<()> {
        mutate_unit(self, |c| c.delete_board_impl(id)).map(|_| ())
    }
    fn archive_board(&mut self, id: Uuid) -> KanbanResult<()> {
        mutate_unit(self, |c| c.archive_board_impl(id)).map(|_| ())
    }
    fn restore_board(&mut self, id: Uuid) -> KanbanResult<()> {
        mutate_unit(self, |c| c.restore_board_impl(id)).map(|_| ())
    }
    fn list_archived_boards(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        self.ctx.list_archived_boards()
    }

    fn create_column(
        &mut self,
        board_id: Uuid,
        name: String,
        position: Option<i32>,
    ) -> KanbanResult<Column> {
        mutate(self, |c| c.create_column_impl(board_id, name, position)).map(|(value, _)| value)
    }
    fn list_columns(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.ctx.list_columns(board_id)
    }
    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.ctx.get_column(id)
    }
    fn update_column(&mut self, id: Uuid, updates: ColumnUpdate) -> KanbanResult<Column> {
        mutate(self, |c| c.update_column_impl(id, updates)).map(|(value, _)| value)
    }
    fn delete_column(&mut self, id: Uuid) -> KanbanResult<()> {
        mutate_unit(self, |c| c.delete_column_impl(id)).map(|_| ())
    }
    fn reorder_column(&mut self, id: Uuid, new_position: i32) -> KanbanResult<Column> {
        mutate(self, |c| c.reorder_column_impl(id, new_position)).map(|(value, _)| value)
    }

    fn create_card(
        &mut self,
        board_id: Uuid,
        column_id: Uuid,
        title: String,
        options: CreateCardOptions,
    ) -> KanbanResult<Card> {
        mutate(self, |c| {
            c.create_card_impl(board_id, column_id, title, options)
        })
        .map(|(value, _)| value)
    }
    fn list_cards(&self, filter: CardListFilter) -> KanbanResult<Vec<CardSummary>> {
        self.ctx.list_cards(filter)
    }
    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        self.ctx.get_card(id)
    }
    fn find_cards_by_identifier(&self, identifier: &str) -> KanbanResult<Vec<Card>> {
        self.ctx.find_cards_by_identifier(identifier)
    }
    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        self.ctx.list_all_cards()
    }
    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        self.ctx.list_all_columns()
    }
    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        self.ctx.list_all_sprints()
    }
    fn update_card(&mut self, id: Uuid, updates: CardUpdate) -> KanbanResult<Card> {
        mutate(self, |c| c.update_card_impl(id, updates)).map(|(value, _)| value)
    }
    fn move_card(
        &mut self,
        id: Uuid,
        column_id: Uuid,
        position: Option<i32>,
    ) -> KanbanResult<Card> {
        mutate(self, |c| c.move_card_impl(id, column_id, position)).map(|(value, _)| value)
    }
    fn archive_card(&mut self, id: Uuid) -> KanbanResult<()> {
        mutate(self, |c| c.archive_card_impl(id)).map(|(value, _)| value)
    }
    fn restore_card(&mut self, id: Uuid, column_id: Option<Uuid>) -> KanbanResult<Card> {
        mutate(self, |c| c.restore_card_impl(id, column_id)).map(|(value, _)| value)
    }
    fn delete_card(&mut self, id: Uuid) -> KanbanResult<()> {
        mutate_unit(self, |c| c.delete_card_impl(id)).map(|_| ())
    }
    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.ctx.list_archived_cards()
    }
    fn list_archived_cards_by_board(&self, board_id: Uuid) -> KanbanResult<Vec<ArchivedCard>> {
        self.ctx.list_archived_cards_by_board(board_id)
    }

    fn assign_card_to_sprint(&mut self, card_id: Uuid, sprint_id: Uuid) -> KanbanResult<Card> {
        mutate(self, |c| c.assign_card_to_sprint_impl(card_id, sprint_id)).map(|(value, _)| value)
    }
    fn unassign_card_from_sprint(&mut self, card_id: Uuid) -> KanbanResult<Card> {
        mutate(self, |c| c.unassign_card_from_sprint_impl(card_id)).map(|(value, _)| value)
    }

    fn get_card_branch_name(&self, id: Uuid) -> KanbanResult<String> {
        self.ctx.get_card_branch_name(id)
    }
    fn get_card_git_checkout(&self, id: Uuid) -> KanbanResult<String> {
        self.ctx.get_card_git_checkout(id)
    }

    fn archive_cards(&mut self, ids: Vec<Uuid>) -> KanbanResult<usize> {
        mutate(self, |c| c.archive_cards_impl(ids)).map(|(value, _)| value)
    }
    fn move_cards(&mut self, ids: Vec<Uuid>, column_id: Uuid) -> KanbanResult<usize> {
        mutate(self, |c| c.move_cards_impl(ids, column_id)).map(|(value, _)| value)
    }
    fn update_cards(&mut self, updates: Vec<(Uuid, CardUpdate)>) -> KanbanResult<usize> {
        mutate(self, |c| c.update_cards_impl(updates)).map(|(value, _)| value)
    }
    fn assign_cards_to_sprint(&mut self, ids: Vec<Uuid>, sprint_id: Uuid) -> KanbanResult<usize> {
        mutate(self, |c| c.assign_cards_to_sprint_impl(ids, sprint_id)).map(|(value, _)| value)
    }
    fn carry_over_sprint_cards(
        &mut self,
        from_sprint_id: Uuid,
        to_sprint_id: Uuid,
    ) -> KanbanResult<usize> {
        mutate(self, |c| {
            c.carry_over_sprint_cards_impl(from_sprint_id, to_sprint_id)
        })
        .map(|(value, _)| value)
    }

    fn create_sprint(
        &mut self,
        board_id: Uuid,
        prefix: Option<String>,
        name: Option<String>,
    ) -> KanbanResult<Sprint> {
        mutate(self, |c| c.create_sprint_impl(board_id, prefix, name)).map(|(value, _)| value)
    }
    fn list_sprints(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.ctx.list_sprints(board_id)
    }
    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.ctx.get_sprint(id)
    }
    fn update_sprint(&mut self, id: Uuid, updates: SprintUpdate) -> KanbanResult<Sprint> {
        mutate(self, |c| c.update_sprint_impl(id, updates)).map(|(value, _)| value)
    }
    fn activate_sprint(&mut self, id: Uuid, duration_days: Option<i32>) -> KanbanResult<Sprint> {
        mutate(self, |c| c.activate_sprint_impl(id, duration_days)).map(|(value, _)| value)
    }
    fn complete_sprint(&mut self, id: Uuid) -> KanbanResult<Sprint> {
        mutate(self, |c| c.complete_sprint_impl(id)).map(|(value, _)| value)
    }
    fn cancel_sprint(&mut self, id: Uuid) -> KanbanResult<Sprint> {
        mutate(self, |c| c.cancel_sprint_impl(id)).map(|(value, _)| value)
    }
    fn delete_sprint(&mut self, id: Uuid) -> KanbanResult<()> {
        mutate_unit(self, |c| c.delete_sprint_impl(id)).map(|_| ())
    }

    fn export_board(&self, board_id: Option<Uuid>) -> KanbanResult<String> {
        self.ctx.export_board(board_id)
    }
    fn import_board(&mut self, data: &str) -> KanbanResult<Board> {
        mutate(self, |c| c.import_board_impl(data)).map(|(value, _)| value)
    }
}

impl GraphOperations for Session {
    fn attach_children(&mut self, parent: Uuid, children: Vec<Uuid>) -> KanbanResult<()> {
        mutate_unit(self, |c| c.attach_children_impl(parent, children)).map(|_| ())
    }
    fn detach_children(&mut self, parent: Uuid, children: Vec<Uuid>) -> KanbanResult<()> {
        mutate_unit(self, |c| c.detach_children_impl(parent, children)).map(|_| ())
    }
    fn list_children_of(&self, parent: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.ctx.list_children_of(parent)
    }
    fn list_parents_of(&self, child: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.ctx.list_parents_of(child)
    }

    fn block(&mut self, blocker: Uuid, blocked: Uuid, severity: Severity) -> KanbanResult<()> {
        mutate_unit(self, |c| c.block_impl(blocker, blocked, severity)).map(|_| ())
    }
    fn unblock(&mut self, blocker: Uuid, blocked: Uuid) -> KanbanResult<()> {
        mutate_unit(self, |c| c.unblock_impl(blocker, blocked)).map(|_| ())
    }
    fn list_blocked_by(&self, blocker: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.ctx.list_blocked_by(blocker)
    }
    fn list_blockers_of(&self, blocked: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.ctx.list_blockers_of(blocked)
    }

    fn relate(&mut self, a: Uuid, b: Uuid, kind: RelatesKind) -> KanbanResult<()> {
        mutate_unit(self, |c| c.relate_impl(a, b, kind)).map(|_| ())
    }
    fn dissociate(&mut self, a: Uuid, b: Uuid) -> KanbanResult<()> {
        mutate_unit(self, |c| c.dissociate_impl(a, b)).map(|_| ())
    }
    fn list_related_to(&self, card: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.ctx.list_related_to(card)
    }
}

impl UndoOperations for Session {
    fn undo(&mut self) -> KanbanResult<Option<Invalidation>> {
        Err(KanbanError::unsupported(
            "sessions are shared across clients",
        ))
    }

    fn redo(&mut self) -> KanbanResult<Option<Invalidation>> {
        Err(KanbanError::unsupported(
            "sessions are shared across clients",
        ))
    }

    fn can_undo(&self) -> bool {
        false
    }

    fn can_redo(&self) -> bool {
        false
    }
}

#[cfg(all(test, feature = "test-helpers"))]
mod tests {
    use super::*;
    use kanban_backend_memory::InMemoryStore;
    use kanban_domain::UndoOperations;
    use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
    use std::sync::Arc;

    async fn seeded_ctx() -> Session {
        let backend: Arc<dyn KanbanBackend> = Arc::new(InMemoryStore::new());
        let mut ctx = KanbanContext::open(backend, AppConfig::default())
            .await
            .unwrap();
        ctx.create_board("Board1".into(), None).unwrap();
        Session { ctx }
    }

    #[tokio::test]
    async fn test_session_undo_declines_with_unsupported() {
        let mut session = seeded_ctx().await;

        assert!(UndoOperations::can_undo(&session.ctx));

        let undo_err = UndoOperations::undo(&mut session).unwrap_err();
        assert!(undo_err.is_unsupported());
        let redo_err = UndoOperations::redo(&mut session).unwrap_err();
        assert!(redo_err.is_unsupported());
        assert!(!UndoOperations::can_undo(&session));
        assert!(!UndoOperations::can_redo(&session));
    }
}
