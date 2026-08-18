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

    #[test]
    fn test_active_search_returns_none_when_no_search_is_active() {
        let mut state = FilterState::default();

        assert!(state.active_search().is_none());
        assert!(state.active_search_mut().is_none());
    }

    #[test]
    fn test_active_search_prefers_board_then_column_then_card() {
        let mut state = FilterState::default();
        state.search.activate();
        state.search.input.insert_char('c');
        state.board_search.activate();
        state.board_search.input.insert_char('b');
        state.column_search.activate();
        state.column_search.input.insert_char('l');

        assert_eq!(state.active_search().unwrap().query(), "b");

        state.board_search.deactivate();
        assert_eq!(state.active_search().unwrap().query(), "l");

        state.column_search.deactivate();
        assert_eq!(state.active_search().unwrap().query(), "c");
    }

    #[test]
    fn test_active_search_mut_deactivates_the_board_search_through_the_accessor() {
        let mut state = FilterState::default();
        state.board_search.activate();
        state.board_search.input.insert_char('b');

        state.active_search_mut().unwrap().deactivate();

        assert!(!state.board_search.is_active);
        assert!(state.board_search.query().is_empty());
        assert!(!state.search.is_active);
        assert!(!state.column_search.is_active);
    }

    #[test]
    fn test_search_input_target_mut_falls_back_to_the_card_search_when_none_is_active() {
        let mut state = FilterState::default();

        state.search_input_target_mut().input.insert_char('x');
        assert_eq!(state.search.query(), "x");

        let mut state = FilterState::default();
        state.column_search.activate();

        state.search_input_target_mut().input.insert_char('y');
        assert_eq!(state.column_search.query(), "y");
        assert!(state.search.is_empty());
    }
}
