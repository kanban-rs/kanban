//! UI-framework-agnostic view layer for the kanban project management tool.
//!
//! Sits below `kanban-tui`/`kanban-web` and above `kanban-domain`/`kanban-core`
//! in the dependency graph. Deliberately does not depend on `kanban-service`
//! or any TUI rendering framework (see `scripts/check-kanban-view-no-ratatui.sh`).

pub mod list_component;
pub mod list_nav;

pub use list_component::{ListComponent, ListRenderInfo};
