use crate::filters::FilterDialogState;
use crate::search::SearchState;
use kanban_core::SelectionState;
use kanban_domain::{SortField, SortOrder};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Default)]
pub struct FilterState {
    pub active_sprint_filters: HashSet<Uuid>,
    pub hide_assigned_cards: bool,
    pub current_sort_field: Option<SortField>,
    pub current_sort_order: Option<SortOrder>,
    pub sort_field_selection: SelectionState,
    /// Highlighted row in the PROJECTS-panel sort field picker (KAN-948), the
    /// board-side analogue of `sort_field_selection`.
    pub board_sort_field_selection: SelectionState,
    pub search: SearchState,
    /// Independent from `search`: the projects panel's own search state, so a
    /// board-name query never bleeds into the tasks panel's card search (the
    /// two panels can carry different active queries at once).
    pub board_search: SearchState,
    /// Independent from both `search` and `board_search`: the board detail
    /// view's column-list search, so a column-name query never bleeds into
    /// the other two panels' searches.
    pub column_search: SearchState,
    pub dialog_state: Option<FilterDialogState>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_has_no_filters_or_dialog_state() {
        let state = FilterState::default();

        assert!(state.active_sprint_filters.is_empty());
        assert!(!state.hide_assigned_cards);
        assert_eq!(state.current_sort_field, None);
        assert_eq!(state.current_sort_order, None);
        assert!(state.dialog_state.is_none());
        assert!(!state.search.is_active);
        assert!(!state.board_search.is_active);
        assert!(!state.column_search.is_active);
    }
}
