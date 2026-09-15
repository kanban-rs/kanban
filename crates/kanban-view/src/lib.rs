//! UI-framework-agnostic view layer for the kanban project management tool.
//!
//! Sits below `kanban-tui`/`kanban-web` and above `kanban-domain`/`kanban-core`
//! in the dependency graph. Deliberately does not depend on `kanban-service`
//! or any rendering framework — this `Cargo.toml` simply never declares one.
//! Each consumer (`kanban-tui`, `kanban-web`) brings its own rendering stack.

pub mod board_list;
pub mod card_list;
pub mod card_list_component;
pub mod controller;
pub mod filter_state;
pub mod filters;
pub mod layout_strategy;
pub mod list_component;
pub mod list_nav;
pub mod list_query;
pub mod panel_titles;
pub mod scroll_indicators;
pub mod search;
pub mod selection_dialog;
pub mod sprint_assign_list;
pub mod view_strategy;

pub use controller::Controller;
pub use list_component::{ListComponent, ListRenderInfo};

#[cfg(test)]
mod dependency_lock_tests {
    #[test]
    fn test_manifest_dependencies_stay_locked_to_the_renderer_free_set() {
        let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
            .expect("kanban-view Cargo.toml must be readable");

        let deps_section = manifest
            .split("[dependencies]")
            .nth(1)
            .expect("[dependencies] section present")
            .split("\n[")
            .next()
            .unwrap();

        let declared: Vec<&str> = deps_section
            .lines()
            .filter_map(|l| {
                let l = l.trim();
                if l.is_empty() || l.starts_with('#') {
                    return None;
                }
                l.split(['=', ' ', '.']).next()
            })
            .collect();

        assert_eq!(
            declared,
            ["kanban-core", "kanban-domain", "serde", "uuid", "chrono"],
            "kanban-view must stay renderer-agnostic: its dependency set is \
             locked so kanban-tui and kanban-web can both build on it. Do not \
             add a rendering framework (or anything else) here."
        );
    }
}
