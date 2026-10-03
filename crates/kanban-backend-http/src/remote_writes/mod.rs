//! `RemoteWrites` for `HttpBackend`: the nine board/column/card create,
//! update, delete mutations plus the board archive/restore pair and the
//! card archive, restore, move, assign-sprint and unassign-sprint mutations
//! (via `RemoteBoardWrites` and `RemoteCardWrites`) and the six graph edge
//! mutations (via `RemoteGraphWrites`), sent directly to the v1 routes
//! rather than executed locally. v1 sends no `If-Match`, so a
//! concurrent conflicting write is last-writer-wins, not rejected. Every
//! request carries the
//! backend's `instance_id` as the `X-Kanban-Client-Id` header, so the
//! server can stamp the change it broadcasts with the client that made it.

mod batch;
mod boards;
mod cards;
mod columns;
mod graph;
mod sprints;

use crate::HttpBackend;
use kanban_backend::{
    RemoteBatchOutcome, RemoteBatchWrites, RemoteBoardWrites, RemoteCardWrites, RemoteGraphWrites,
    RemoteSprintWrites, RemoteWrites,
};
use kanban_domain::{
    Board, BoardUpdate, Card, CardUpdate, Column, ColumnUpdate, Invalidation, KanbanResult,
    NewBoard, NewCard, NewColumn, RelatesKind, Severity, Sprint, SprintUpdate,
};
use uuid::Uuid;

impl RemoteBoardWrites for HttpBackend {
    fn archive_board(&self, id: Uuid) -> KanbanResult<Invalidation> {
        self.rw_archive_board(id)
    }

    fn restore_board(&self, id: Uuid) -> KanbanResult<Invalidation> {
        self.rw_restore_board(id)
    }
}
impl RemoteCardWrites for HttpBackend {
    fn archive_card(&self, id: Uuid) -> KanbanResult<Invalidation> {
        self.rw_archive_card(id)
    }

    fn restore_card(
        &self,
        id: Uuid,
        column_id: Option<Uuid>,
    ) -> KanbanResult<(Card, Invalidation)> {
        self.rw_restore_card(id, column_id)
    }

    fn move_card(
        &self,
        id: Uuid,
        column_id: Uuid,
        position: Option<i32>,
    ) -> KanbanResult<(Card, Invalidation)> {
        self.rw_move_card(id, column_id, position)
    }

    fn assign_card_to_sprint(
        &self,
        id: Uuid,
        sprint_id: Uuid,
    ) -> KanbanResult<(Card, Invalidation)> {
        self.rw_assign_card_to_sprint(id, sprint_id)
    }

    fn unassign_card_from_sprint(&self, id: Uuid) -> KanbanResult<(Card, Invalidation)> {
        self.rw_unassign_card_from_sprint(id)
    }
}

impl RemoteGraphWrites for HttpBackend {
    fn attach_children(&self, parent: Uuid, children: &[Uuid]) -> KanbanResult<Invalidation> {
        self.rw_attach_children(parent, children)
    }

    fn detach_children(&self, parent: Uuid, children: &[Uuid]) -> KanbanResult<Invalidation> {
        self.rw_detach_children(parent, children)
    }

    fn block(
        &self,
        blocker: Uuid,
        blocked: Uuid,
        severity: Severity,
    ) -> KanbanResult<Invalidation> {
        self.rw_block(blocker, blocked, severity)
    }

    fn unblock(&self, blocker: Uuid, blocked: Uuid) -> KanbanResult<Invalidation> {
        self.rw_unblock(blocker, blocked)
    }

    fn relate(&self, a: Uuid, b: Uuid, kind: RelatesKind) -> KanbanResult<Invalidation> {
        self.rw_relate(a, b, kind)
    }

    fn dissociate(&self, a: Uuid, b: Uuid) -> KanbanResult<Invalidation> {
        self.rw_dissociate(a, b)
    }
}

impl RemoteSprintWrites for HttpBackend {
    fn create_sprint(
        &self,
        board_id: Uuid,
        id: Option<Uuid>,
        name: Option<&str>,
        prefix: Option<&str>,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        self.rw_create_sprint(board_id, id, name, prefix)
    }

    fn update_sprint(
        &self,
        id: Uuid,
        updates: &SprintUpdate,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        self.rw_update_sprint(id, updates)
    }

    fn delete_sprint(&self, id: Uuid) -> KanbanResult<Invalidation> {
        self.rw_delete_sprint(id)
    }

    fn activate_sprint(
        &self,
        id: Uuid,
        duration_days: Option<i32>,
    ) -> KanbanResult<(Sprint, Invalidation)> {
        self.rw_activate_sprint(id, duration_days)
    }

    fn complete_sprint(&self, id: Uuid) -> KanbanResult<(Sprint, Invalidation)> {
        self.rw_complete_sprint(id)
    }

    fn cancel_sprint(&self, id: Uuid) -> KanbanResult<(Sprint, Invalidation)> {
        self.rw_cancel_sprint(id)
    }

    fn carry_over_sprint_cards(
        &self,
        from_sprint_id: Uuid,
        to_sprint_id: Uuid,
    ) -> KanbanResult<(usize, Invalidation)> {
        self.rw_carry_over_sprint_cards(from_sprint_id, to_sprint_id)
    }
}

impl RemoteBatchWrites for HttpBackend {
    fn archive_cards(&self, ids: &[Uuid]) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        self.rw_archive_cards(ids)
    }

    fn move_cards(
        &self,
        ids: &[Uuid],
        column_id: Uuid,
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        self.rw_move_cards(ids, column_id)
    }

    fn assign_cards_to_sprint(
        &self,
        ids: &[Uuid],
        sprint_id: Uuid,
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        self.rw_assign_cards_to_sprint(ids, sprint_id)
    }

    fn update_cards(
        &self,
        updates: &[(Uuid, CardUpdate)],
    ) -> KanbanResult<(RemoteBatchOutcome, Invalidation)> {
        self.rw_update_cards(updates)
    }
}

impl RemoteWrites for HttpBackend {
    fn create_board(
        &self,
        id: Option<Uuid>,
        spec: &NewBoard,
    ) -> KanbanResult<(Board, Invalidation)> {
        self.rw_create_board(id, spec)
    }

    fn update_board(&self, id: Uuid, updates: &BoardUpdate) -> KanbanResult<(Board, Invalidation)> {
        self.rw_update_board(id, updates)
    }

    fn delete_board(&self, id: Uuid) -> KanbanResult<Invalidation> {
        self.rw_delete_board(id)
    }

    fn create_column(
        &self,
        board_id: Uuid,
        spec: &NewColumn,
    ) -> KanbanResult<(Column, Invalidation)> {
        self.rw_create_column(board_id, spec)
    }

    fn update_column(
        &self,
        id: Uuid,
        updates: &ColumnUpdate,
    ) -> KanbanResult<(Column, Invalidation)> {
        self.rw_update_column(id, updates)
    }

    fn delete_column(&self, id: Uuid) -> KanbanResult<Invalidation> {
        self.rw_delete_column(id)
    }

    fn create_card(&self, id: Option<Uuid>, spec: &NewCard) -> KanbanResult<(Card, Invalidation)> {
        self.rw_create_card(id, spec)
    }

    fn update_card(&self, id: Uuid, updates: &CardUpdate) -> KanbanResult<(Card, Invalidation)> {
        self.rw_update_card(id, updates)
    }

    fn delete_card(&self, id: Uuid) -> KanbanResult<Invalidation> {
        self.rw_delete_card(id)
    }
}
