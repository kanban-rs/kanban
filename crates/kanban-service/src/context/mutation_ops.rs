use super::KanbanContext;
use kanban_domain::commands::Command;
use kanban_domain::{
    Board, BoardCreateOutcome, BoardUpdate, Card, CardCreateOutcome, CardUpdate, Column,
    ColumnCreateOutcome, ColumnUpdate, CreateCardOptions, Invalidation, KanbanResult,
    MutationOperations, NewBoard, NewCard, NewColumn, RelatesKind, Severity, Sprint,
    SprintCreateOutcome, SprintUpdate,
};
use uuid::Uuid;

impl MutationOperations for KanbanContext {
    fn create_board_impl(
        &mut self,
        name: String,
        card_prefix: Option<String>,
    ) -> KanbanResult<(Board, Invalidation)> {
        KanbanContext::create_board_impl(self, name, card_prefix)
    }
    fn update_board_impl(
        &mut self,
        id: Uuid,
        updates: BoardUpdate,
    ) -> KanbanResult<(Board, Invalidation)> {
        KanbanContext::update_board_impl(self, id, updates)
    }
    fn delete_board_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation> {
        KanbanContext::delete_board_impl(self, id)
    }
    fn archive_board_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation> {
        KanbanContext::archive_board_impl(self, id)
    }
    fn restore_board_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation> {
        KanbanContext::restore_board_impl(self, id)
    }

    fn create_card_impl(
        &mut self,
        board_id: Uuid,
        column_id: Uuid,
        title: String,
        options: CreateCardOptions,
    ) -> KanbanResult<(Card, Invalidation)> {
        KanbanContext::create_card_impl(self, board_id, column_id, title, options)
    }
    fn update_card_impl(
        &mut self,
        id: Uuid,
        updates: CardUpdate,
    ) -> KanbanResult<(Card, Invalidation)> {
        KanbanContext::update_card_impl(self, id, updates)
    }
    fn move_card_impl(
        &mut self,
        id: Uuid,
        column_id: Uuid,
        position: Option<i32>,
    ) -> KanbanResult<(Card, Invalidation)> {
        KanbanContext::move_card_impl(self, id, column_id, position)
    }
    fn archive_card_impl(&mut self, id: Uuid) -> KanbanResult<((), Invalidation)> {
        KanbanContext::archive_card_impl(self, id)
    }
    fn restore_card_impl(
        &mut self,
        id: Uuid,
        column_id: Option<Uuid>,
    ) -> KanbanResult<(Card, Invalidation)> {
        KanbanContext::restore_card_impl(self, id, column_id)
    }
    fn delete_card_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation> {
        KanbanContext::delete_card_impl(self, id)
    }
    fn assign_card_to_sprint_impl(
        &mut self,
        card_id: Uuid,
        sprint_id: Uuid,
    ) -> KanbanResult<(Card, Invalidation)> {
        KanbanContext::assign_card_to_sprint_impl(self, card_id, sprint_id)
    }
    fn unassign_card_from_sprint_impl(
        &mut self,
        card_id: Uuid,
    ) -> KanbanResult<(Card, Invalidation)> {
        KanbanContext::unassign_card_from_sprint_impl(self, card_id)
    }

    fn archive_cards_impl(&mut self, ids: Vec<Uuid>) -> KanbanResult<(usize, Invalidation)> {
        KanbanContext::archive_cards_impl(self, ids)
    }
    fn move_cards_impl(
        &mut self,
        ids: Vec<Uuid>,
        column_id: Uuid,
    ) -> KanbanResult<(usize, Invalidation)> {
        KanbanContext::move_cards_impl(self, ids, column_id)
    }
    fn update_cards_impl(
        &mut self,
        updates: Vec<(Uuid, CardUpdate)>,
    ) -> KanbanResult<(usize, Invalidation)> {
        KanbanContext::update_cards_impl(self, updates)
    }
    fn assign_cards_to_sprint_impl(
        &mut self,
        ids: Vec<Uuid>,
        sprint_id: Uuid,
    ) -> KanbanResult<(usize, Invalidation)> {
        KanbanContext::assign_cards_to_sprint_impl(self, ids, sprint_id)
    }

    fn create_column_impl(
        &mut self,
        board_id: Uuid,
        name: String,
        position: Option<i32>,
    ) -> KanbanResult<(Column, Invalidation)> {
        KanbanContext::create_column_impl(self, board_id, name, position)
    }
    fn update_column_impl(
        &mut self,
        id: Uuid,
        updates: ColumnUpdate,
    ) -> KanbanResult<(Column, Invalidation)> {
        KanbanContext::update_column_impl(self, id, updates)
    }
    fn delete_column_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation> {
        KanbanContext::delete_column_impl(self, id)
    }
    fn reorder_column_impl(
        &mut self,
        id: Uuid,
        new_position: i32,
    ) -> KanbanResult<(Column, Invalidation)> {
        KanbanContext::reorder_column_impl(self, id, new_position)
    }

    fn carry_over_sprint_cards_impl(
        &mut self,
        from_sprint_id: Uuid,
        to_sprint_id: Uuid,
    ) -> KanbanResult<(usize, Invalidation)> {
        KanbanContext::carry_over_sprint_cards_impl(self, from_sprint_id, to_sprint_id)
    }
    fn create_sprint_impl(
        &mut self,
        board_id: Uuid,
        prefix: Option<String>,
        name: Option<String>,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        KanbanContext::create_sprint_impl(self, board_id, prefix, name)
    }
    fn update_sprint_impl(
        &mut self,
        id: Uuid,
        updates: SprintUpdate,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        KanbanContext::update_sprint_impl(self, id, updates)
    }
    fn activate_sprint_impl(
        &mut self,
        id: Uuid,
        duration_days: Option<i32>,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        KanbanContext::activate_sprint_impl(self, id, duration_days)
    }
    fn complete_sprint_impl(&mut self, id: Uuid) -> KanbanResult<(Sprint, Invalidation)> {
        KanbanContext::complete_sprint_impl(self, id)
    }
    fn cancel_sprint_impl(&mut self, id: Uuid) -> KanbanResult<(Sprint, Invalidation)> {
        KanbanContext::cancel_sprint_impl(self, id)
    }
    fn delete_sprint_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation> {
        KanbanContext::delete_sprint_impl(self, id)
    }
    fn import_board_impl(&mut self, data: &str) -> KanbanResult<(Board, Invalidation)> {
        KanbanContext::import_board_impl(self, data)
    }

    fn attach_children_impl(
        &mut self,
        parent: Uuid,
        children: Vec<Uuid>,
    ) -> KanbanResult<Invalidation> {
        KanbanContext::attach_children_impl(self, parent, children)
    }
    fn detach_children_impl(
        &mut self,
        parent: Uuid,
        children: Vec<Uuid>,
    ) -> KanbanResult<Invalidation> {
        KanbanContext::detach_children_impl(self, parent, children)
    }
    fn block_impl(
        &mut self,
        blocker: Uuid,
        blocked: Uuid,
        severity: Severity,
    ) -> KanbanResult<Invalidation> {
        KanbanContext::block_impl(self, blocker, blocked, severity)
    }
    fn unblock_impl(&mut self, blocker: Uuid, blocked: Uuid) -> KanbanResult<Invalidation> {
        KanbanContext::unblock_impl(self, blocker, blocked)
    }
    fn relate_impl(&mut self, a: Uuid, b: Uuid, kind: RelatesKind) -> KanbanResult<Invalidation> {
        KanbanContext::relate_impl(self, a, b, kind)
    }
    fn dissociate_impl(&mut self, a: Uuid, b: Uuid) -> KanbanResult<Invalidation> {
        KanbanContext::dissociate_impl(self, a, b)
    }

    fn create_board_from_spec(
        &mut self,
        id: Option<Uuid>,
        spec: NewBoard,
    ) -> KanbanResult<(Board, Invalidation)> {
        KanbanContext::create_board_from_spec(self, id, spec)
    }
    fn create_or_replace_board(
        &mut self,
        id: Uuid,
        spec: NewBoard,
    ) -> KanbanResult<(BoardCreateOutcome, Invalidation)> {
        KanbanContext::create_or_replace_board(self, id, spec)
    }
    fn create_card_from_spec(
        &mut self,
        client_id: Option<Uuid>,
        spec: NewCard,
    ) -> KanbanResult<(Card, Invalidation)> {
        KanbanContext::create_card_from_spec(self, client_id, spec)
    }
    fn create_or_replace_card(
        &mut self,
        id: Uuid,
        spec: NewCard,
    ) -> KanbanResult<(CardCreateOutcome, Invalidation)> {
        KanbanContext::create_or_replace_card(self, id, spec)
    }
    fn create_column_from_spec(
        &mut self,
        id: Option<Uuid>,
        spec: NewColumn,
    ) -> KanbanResult<(Column, Invalidation)> {
        KanbanContext::create_column_from_spec(self, id, spec)
    }
    fn create_or_replace_column(
        &mut self,
        id: Uuid,
        spec: NewColumn,
        position: Option<i32>,
    ) -> KanbanResult<(ColumnCreateOutcome, Invalidation)> {
        KanbanContext::create_or_replace_column(self, id, spec, position)
    }
    fn create_sprint_from_spec(
        &mut self,
        board_id: Uuid,
        id: Option<Uuid>,
        name: Option<String>,
        prefix: Option<String>,
        auto_consume_name: bool,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        KanbanContext::create_sprint_from_spec(self, board_id, id, name, prefix, auto_consume_name)
    }
    fn create_or_replace_sprint(
        &mut self,
        board_id: Uuid,
        id: Uuid,
        name: Option<String>,
        prefix: Option<String>,
        auto_consume_name: bool,
    ) -> KanbanResult<(SprintCreateOutcome, Invalidation)> {
        KanbanContext::create_or_replace_sprint(self, board_id, id, name, prefix, auto_consume_name)
    }

    fn execute(&mut self, commands: Vec<Command>) -> KanbanResult<Invalidation> {
        KanbanContext::execute(self, commands)
    }
}
