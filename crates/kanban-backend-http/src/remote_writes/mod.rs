//! `RemoteWrites` for `HttpBackend`: the nine board/column/card create,
//! update, delete mutations, sent directly to the v1 routes rather than
//! executed locally. v1 sends no `If-Match`, so a concurrent conflicting
//! write is last-writer-wins, not rejected. Every request carries the
//! backend's `instance_id` as the `X-Kanban-Client-Id` header, so the
//! server can stamp the change it broadcasts with the client that made it.

mod boards;
mod cards;
mod columns;

use crate::HttpBackend;
use kanban_backend::RemoteWrites;
use kanban_domain::{
    Board, BoardUpdate, Card, CardUpdate, Column, ColumnUpdate, Invalidation, KanbanResult,
    NewBoard, NewCard, NewColumn,
};
use uuid::Uuid;

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
