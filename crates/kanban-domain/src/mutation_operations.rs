use crate::commands::Command;
use crate::dependencies::{RelatesKind, Severity};
use crate::{
    Board, BoardCreateOutcome, BoardUpdate, Card, CardCreateOutcome, CardUpdate, Column,
    ColumnCreateOutcome, ColumnUpdate, CreateCardOptions, Invalidation, KanbanResult, NewBoard,
    NewCard, NewColumn, Sprint, SprintCreateOutcome, SprintUpdate,
};
use uuid::Uuid;

/// The invalidation-carrying mutation surface `KanbanContext` exposes today:
/// every `*_impl` mutator that returns a domain [`Invalidation`], the eight
/// spec-shaped create/create-or-replace entry points, and `execute`. Every
/// method returns its `Invalidation` (or the value it wraps); implementers
/// must not discard it.
///
/// Object-safe by construction: no default bodies, no generic methods.
pub trait MutationOperations {
    fn create_board_impl(
        &mut self,
        name: String,
        card_prefix: Option<String>,
    ) -> KanbanResult<(Board, Invalidation)>;
    fn update_board_impl(
        &mut self,
        id: Uuid,
        updates: BoardUpdate,
    ) -> KanbanResult<(Board, Invalidation)>;
    fn delete_board_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation>;
    fn archive_board_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation>;
    fn restore_board_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation>;

    fn create_card_impl(
        &mut self,
        board_id: Uuid,
        column_id: Uuid,
        title: String,
        options: CreateCardOptions,
    ) -> KanbanResult<(Card, Invalidation)>;
    fn update_card_impl(
        &mut self,
        id: Uuid,
        updates: CardUpdate,
    ) -> KanbanResult<(Card, Invalidation)>;
    fn move_card_impl(
        &mut self,
        id: Uuid,
        column_id: Uuid,
        position: Option<i32>,
    ) -> KanbanResult<(Card, Invalidation)>;
    fn archive_card_impl(&mut self, id: Uuid) -> KanbanResult<((), Invalidation)>;
    fn restore_card_impl(
        &mut self,
        id: Uuid,
        column_id: Option<Uuid>,
    ) -> KanbanResult<(Card, Invalidation)>;
    fn delete_card_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation>;
    fn assign_card_to_sprint_impl(
        &mut self,
        card_id: Uuid,
        sprint_id: Uuid,
    ) -> KanbanResult<(Card, Invalidation)>;
    fn unassign_card_from_sprint_impl(
        &mut self,
        card_id: Uuid,
    ) -> KanbanResult<(Card, Invalidation)>;

    fn archive_cards_impl(&mut self, ids: Vec<Uuid>) -> KanbanResult<(usize, Invalidation)>;
    fn move_cards_impl(
        &mut self,
        ids: Vec<Uuid>,
        column_id: Uuid,
    ) -> KanbanResult<(usize, Invalidation)>;
    fn update_cards_impl(
        &mut self,
        updates: Vec<(Uuid, CardUpdate)>,
    ) -> KanbanResult<(usize, Invalidation)>;
    fn assign_cards_to_sprint_impl(
        &mut self,
        ids: Vec<Uuid>,
        sprint_id: Uuid,
    ) -> KanbanResult<(usize, Invalidation)>;

    fn create_column_impl(
        &mut self,
        board_id: Uuid,
        name: String,
        position: Option<i32>,
    ) -> KanbanResult<(Column, Invalidation)>;
    fn update_column_impl(
        &mut self,
        id: Uuid,
        updates: ColumnUpdate,
    ) -> KanbanResult<(Column, Invalidation)>;
    fn delete_column_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation>;
    fn reorder_column_impl(
        &mut self,
        id: Uuid,
        new_position: i32,
    ) -> KanbanResult<(Column, Invalidation)>;

    fn carry_over_sprint_cards_impl(
        &mut self,
        from_sprint_id: Uuid,
        to_sprint_id: Uuid,
    ) -> KanbanResult<(usize, Invalidation)>;
    fn create_sprint_impl(
        &mut self,
        board_id: Uuid,
        prefix: Option<String>,
        name: Option<String>,
    ) -> KanbanResult<(Sprint, Invalidation)>;
    fn update_sprint_impl(
        &mut self,
        id: Uuid,
        updates: SprintUpdate,
    ) -> KanbanResult<(Sprint, Invalidation)>;
    fn activate_sprint_impl(
        &mut self,
        id: Uuid,
        duration_days: Option<i32>,
    ) -> KanbanResult<(Sprint, Invalidation)>;
    fn complete_sprint_impl(&mut self, id: Uuid) -> KanbanResult<(Sprint, Invalidation)>;
    fn cancel_sprint_impl(&mut self, id: Uuid) -> KanbanResult<(Sprint, Invalidation)>;
    fn delete_sprint_impl(&mut self, id: Uuid) -> KanbanResult<Invalidation>;
    fn import_board_impl(&mut self, data: &str) -> KanbanResult<(Board, Invalidation)>;

    fn attach_children_impl(
        &mut self,
        parent: Uuid,
        children: Vec<Uuid>,
    ) -> KanbanResult<Invalidation>;
    fn detach_children_impl(
        &mut self,
        parent: Uuid,
        children: Vec<Uuid>,
    ) -> KanbanResult<Invalidation>;
    fn block_impl(
        &mut self,
        blocker: Uuid,
        blocked: Uuid,
        severity: Severity,
    ) -> KanbanResult<Invalidation>;
    fn unblock_impl(&mut self, blocker: Uuid, blocked: Uuid) -> KanbanResult<Invalidation>;
    fn relate_impl(&mut self, a: Uuid, b: Uuid, kind: RelatesKind) -> KanbanResult<Invalidation>;
    fn dissociate_impl(&mut self, a: Uuid, b: Uuid) -> KanbanResult<Invalidation>;

    fn create_board_from_spec(
        &mut self,
        id: Option<Uuid>,
        spec: NewBoard,
    ) -> KanbanResult<(Board, Invalidation)>;
    fn create_or_replace_board(
        &mut self,
        id: Uuid,
        spec: NewBoard,
    ) -> KanbanResult<(BoardCreateOutcome, Invalidation)>;
    fn create_card_from_spec(
        &mut self,
        client_id: Option<Uuid>,
        spec: NewCard,
    ) -> KanbanResult<(Card, Invalidation)>;
    fn create_or_replace_card(
        &mut self,
        id: Uuid,
        spec: NewCard,
    ) -> KanbanResult<(CardCreateOutcome, Invalidation)>;
    fn create_column_from_spec(
        &mut self,
        id: Option<Uuid>,
        spec: NewColumn,
    ) -> KanbanResult<(Column, Invalidation)>;
    fn create_or_replace_column(
        &mut self,
        id: Uuid,
        spec: NewColumn,
        position: Option<i32>,
    ) -> KanbanResult<(ColumnCreateOutcome, Invalidation)>;
    fn create_sprint_from_spec(
        &mut self,
        board_id: Uuid,
        id: Option<Uuid>,
        name: Option<String>,
        prefix: Option<String>,
        auto_consume_name: bool,
    ) -> KanbanResult<(Sprint, Invalidation)>;
    fn create_or_replace_sprint(
        &mut self,
        board_id: Uuid,
        id: Uuid,
        name: Option<String>,
        prefix: Option<String>,
        auto_consume_name: bool,
    ) -> KanbanResult<(SprintCreateOutcome, Invalidation)>;

    fn execute(&mut self, commands: Vec<Command>) -> KanbanResult<Invalidation>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_is_object_safe() {
        fn _accepts_dyn(_: &mut dyn MutationOperations) {}
    }

    #[test]
    fn test_mutation_operations_declares_every_invalidation_carrying_mutation() {
        let src = include_str!("mutation_operations.rs");
        let decl = &src[..src.find("#[cfg(test)]").unwrap()];
        assert_eq!(
            decl.matches("\n    fn ").count(),
            44,
            "MutationOperations must declare exactly 44 methods"
        );
        assert!(
            !decl.contains("\n    }"),
            "MutationOperations methods must have no default bodies"
        );
    }
}
