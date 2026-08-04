//! UI-framework-agnostic view layer for the kanban project management tool.
//!
//! Sits below `kanban-tui`/`kanban-web` and above `kanban-domain`/`kanban-core`
//! in the dependency graph. Deliberately does not depend on `kanban-service`
//! or any TUI rendering framework (see `scripts/check-kanban-view-no-ratatui.sh`).

pub mod card_list;
pub mod card_list_component;
pub mod filter_state;
pub mod filters;
pub mod layout_strategy;
pub mod list_component;
pub mod list_nav;
pub mod model;
pub mod panel_titles;
pub mod scroll_indicators;
pub mod search;
pub mod selection_dialog;
pub mod sprint_assign_list;
pub mod view_strategy;

pub use list_component::{ListComponent, ListRenderInfo};
