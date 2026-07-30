//! Orchestration layer between persistence backends and interactive
//! frontends. [`KanbanContext`] owns per-session undo state
//! ([`undo_stack::UndoStack`]) and runs every command batch through
//! [`backend::KanbanBackend::with_transaction`].
//!
//! Undo and redo are inverse-command CRUD against current state.
//! [`kanban_domain::commands::Command::capture_inverse`] produces the
//! inverse batch at execute time; the `(forward, inverse)` pair lives
//! on the in-RAM `UndoStack`. The audit log (via
//! [`backend::KanbanBackend::append_batch`]) is a separate
//! append-only record of executed batches.

pub use kanban_api as api;
pub use kanban_backend as backend;
mod cascade;
pub mod config;
mod context;
#[cfg(feature = "json")]
pub mod json_backend;
mod path;
#[cfg(feature = "sqlite")]
pub mod sqlite_backend;
mod store_manager;
pub mod undo_stack;
pub use config::AppConfigDto;
pub use context::{
    BatchOperationFailure, BatchOperationResult, BoardCreateOutcome, BoardRelations,
    CardCreateOutcome, ColumnCreateOutcome, KanbanContext, SprintCreateOutcome,
};
pub use kanban_backend::KanbanBackend;
pub use kanban_backend::RemoteWrites;
pub use path::validate_path;
pub use store_manager::StoreManager;

#[cfg(feature = "test-helpers")]
pub mod test_helpers;

pub use kanban_core::{AppConfig, AppType};

pub use kanban_domain::{
    ArchivedCard, Board, BoardId, BoardUpdate, Card, CardId, CardListFilter, CardPriority,
    CardStatus, CardSummary, CardUpdate, Column, ColumnId, ColumnUpdate, CreateCardOptions,
    DependencyGraph, FieldUpdate, KanbanError, KanbanOperations, KanbanResult, NewBoard, NewCard,
    NewColumn, Snapshot, Sprint, SprintId, SprintStatus, SprintUpdate,
};

#[cfg(feature = "json")]
pub use kanban_persistence_json::JsonStoreFactory;
#[cfg(feature = "sqlite")]
pub use kanban_persistence_sqlite::SqliteStoreFactory;

/// Open a [`KanbanContext`] from a file locator with zero I/O.
/// The backend (JSON or SQLite) is detected automatically.
/// Data is loaded lazily on the first [`DataStore`] or [`CommandStore`] call.
#[cfg(any(feature = "json", feature = "sqlite"))]
pub async fn open_context(locator: &str, config: AppConfig) -> KanbanResult<KanbanContext> {
    let mut config = config;
    let sm = StoreManager::new(default_registry());
    sm.sync_backend_with_file(locator, &mut config);
    let backend = sm.make_backend(locator, &config).await?;
    KanbanContext::open(backend, config).await
}

/// Returns a `StoreRegistry` pre-populated with available backends.
/// SQLite is registered first so its magic-byte check takes priority.
#[cfg(any(feature = "json", feature = "sqlite"))]
pub fn default_registry() -> kanban_persistence::StoreRegistry {
    let mut registry = kanban_persistence::StoreRegistry::new();
    #[cfg(feature = "sqlite")]
    registry.register(Box::new(SqliteStoreFactory));
    #[cfg(feature = "json")]
    registry.register(Box::new(JsonStoreFactory));
    registry
}
