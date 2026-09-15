use kanban_domain::{
    Board, BoardUpdate, Card, CardUpdate, Column, ColumnUpdate, Invalidation, KanbanResult,
    NewBoard, NewCard, NewColumn,
};
use uuid::Uuid;

/// Optional backend capability: delegate the nine core CRUD mutations
/// directly to a remote authority (a `kanban-server` instance) instead of
/// `KanbanContext`'s local command-execute-then-log path. When
/// `KanbanBackend::remote_writes()` returns `Some`, `KanbanContext`'s nine
/// v1 mutators divert to these methods before any local prework (id
/// minting, FK checks, cascade building) runs, and pass the returned
/// `Invalidation` straight through to the caller as the remote authority's
/// own answer. Local backends (JSON/SQLite/InMemory) never override
/// `KanbanBackend::remote_writes()`, so today this trait has no production
/// implementor.
pub trait RemoteWrites: Send + Sync {
    fn create_board(
        &self,
        id: Option<Uuid>,
        spec: &NewBoard,
    ) -> KanbanResult<(Board, Invalidation)>;
    fn update_board(&self, id: Uuid, updates: &BoardUpdate) -> KanbanResult<(Board, Invalidation)>;
    fn delete_board(&self, id: Uuid) -> KanbanResult<Invalidation>;

    fn create_column(
        &self,
        board_id: Uuid,
        spec: &NewColumn,
    ) -> KanbanResult<(Column, Invalidation)>;
    fn update_column(
        &self,
        id: Uuid,
        updates: &ColumnUpdate,
    ) -> KanbanResult<(Column, Invalidation)>;
    fn delete_column(&self, id: Uuid) -> KanbanResult<Invalidation>;

    fn create_card(&self, id: Option<Uuid>, spec: &NewCard) -> KanbanResult<(Card, Invalidation)>;
    fn update_card(&self, id: Uuid, updates: &CardUpdate) -> KanbanResult<(Card, Invalidation)>;
    fn delete_card(&self, id: Uuid) -> KanbanResult<Invalidation>;
}
