use crate::card_list::CardList;
use kanban_domain::{Board, Card, Column, Sprint};
use uuid::Uuid;

pub struct ViewRefreshContext<'a> {
    pub board: &'a Board,
    pub all_cards: &'a [Card],
    pub all_columns: &'a [Column],
    pub all_sprints: &'a [Sprint],
    pub active_sprint_filters: std::collections::HashSet<Uuid>,
    pub hide_assigned_cards: bool,
    pub search_query: Option<&'a str>,
}

/// Render-free half of what was `UnifiedViewStrategy` in kanban-tui: which
/// cards go in which list, in what order, and how navigation moves between
/// lists. The ratatui-coupled pairing with `Box<dyn RenderStrategy>` stays
/// in kanban-tui's own `view_strategy.rs` as a thin delegating wrapper.
pub trait ViewStrategy {
    fn get_active_task_list(&self) -> Option<&CardList>;
    fn get_active_task_list_mut(&mut self) -> Option<&mut CardList>;
    fn get_all_task_lists(&self) -> Vec<&CardList>;
    fn navigate_left(&mut self, select_last: bool) -> bool;
    fn navigate_right(&mut self, select_last: bool) -> bool;
    fn refresh_task_lists(&mut self, ctx: &ViewRefreshContext);
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    fn as_any(&self) -> &dyn std::any::Any;
    fn try_navigate_to_column(&mut self, _index: usize) -> bool {
        false
    }
}
