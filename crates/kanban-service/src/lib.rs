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
#[cfg(test)]
mod backend_test_support;
mod cascade;
pub mod config;
mod context;
pub mod git;
mod path;
mod store_adapter;
mod store_manager;
pub mod undo_stack;
pub use config::AppConfigDto;
pub use context::{
    BatchOperationFailure, BatchOperationResult, BoardCreateOutcome, BoardRelations,
    CardCreateOutcome, ColumnCreateOutcome, KanbanContext, SprintCreateOutcome,
};
pub use git::{CommitRef, GitProvider, ShellGitProvider};
pub use kanban_backend::KanbanBackend;
pub use kanban_backend::RemoteWrites;
pub use kanban_backend::TransactionFn;
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
