//! Library surface for `kanban-cli`.
//!
//! The `kanban` binary and third-party backend crates both depend on this
//! library. Third-party backends register themselves via
//! [`CliApp::register_backend`] and call [`CliApp::run`] from their own
//! `main.rs`, owning the entrypoint while reusing all CLI plumbing.

pub(crate) mod app;
pub(crate) mod cli;
pub(crate) mod context;
pub(crate) mod error;
pub(crate) mod handlers;
pub(crate) mod model_read;
pub(crate) mod output;
pub(crate) mod scope;

pub use app::CliApp;
pub use error::{KanbanCliError, KanbanCliResult};
pub use kanban_persistence::{StoreFactory, StoreRegistry};
pub use kanban_service::StoreManager;
