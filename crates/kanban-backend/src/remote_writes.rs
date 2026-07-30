use kanban_domain::{
    Board, BoardUpdate, Card, CardUpdate, Column, ColumnUpdate, KanbanResult, NewBoard, NewCard,
    NewColumn,
};
use uuid::Uuid;

/// Optional backend capability: delegate the nine core CRUD mutations
/// directly to a remote authority (a `kanban-server` instance) instead of
/// `KanbanContext`'s local command-execute-then-log path. `HttpBackend`
/// (KAN-697) is the only real implementor; local backends (JSON/SQLite/
/// InMemory) never override `KanbanBackend::remote_writes()`, so this trait
/// has exactly one live implementation.
pub trait RemoteWrites: Send + Sync {
    fn create_board(&self, id: Option<Uuid>, spec: &NewBoard) -> KanbanResult<Board>;
    fn update_board(&self, id: Uuid, updates: &BoardUpdate) -> KanbanResult<Board>;
    fn delete_board(&self, id: Uuid) -> KanbanResult<()>;

    fn create_column(&self, board_id: Uuid, spec: &NewColumn) -> KanbanResult<Column>;
    fn update_column(&self, id: Uuid, updates: &ColumnUpdate) -> KanbanResult<Column>;
    fn delete_column(&self, id: Uuid) -> KanbanResult<()>;

    fn create_card(&self, id: Option<Uuid>, spec: &NewCard) -> KanbanResult<Card>;
    fn update_card(&self, id: Uuid, updates: &CardUpdate) -> KanbanResult<Card>;
    fn delete_card(&self, id: Uuid) -> KanbanResult<()>;
}
