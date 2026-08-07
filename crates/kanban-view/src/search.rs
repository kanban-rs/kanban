use kanban_core::InputState;

/// UI state for search mode.
///
/// This struct manages the search input and active state.
/// The actual search logic is in the domain layer.
pub struct SearchState {
    pub input: InputState,
    pub is_active: bool,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            input: InputState::new(),
            is_active: false,
        }
    }

    pub fn activate(&mut self) {
        self.is_active = true;
        self.input.clear();
    }

    pub fn deactivate(&mut self) {
        self.is_active = false;
        self.input.clear();
    }

    pub fn query(&self) -> &str {
        self.input.as_str()
    }

    pub fn is_empty(&self) -> bool {
        self.input.as_str().is_empty()
    }

    pub fn active_query(&self) -> Option<&str> {
        if self.is_active {
            Some(self.query())
        } else {
            None
        }
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state_is_inactive_and_empty() {
        let state = SearchState::new();
        assert!(!state.is_active);
        assert!(state.is_empty());
        assert_eq!(state.active_query(), None);
    }

    #[test]
    fn test_activate_sets_active_and_clears_input() {
        let mut state = SearchState::new();
        state.input.insert_char('x');

        state.activate();

        assert!(state.is_active);
        assert!(state.is_empty());
    }

    #[test]
    fn test_deactivate_clears_active_flag_and_input() {
        let mut state = SearchState::new();
        state.activate();
        state.input.insert_char('a');

        state.deactivate();

        assert!(!state.is_active);
        assert!(state.is_empty());
    }

    #[test]
    fn test_active_query_returns_query_only_when_active() {
        let mut state = SearchState::new();
        state.input.insert_char('a');
        state.input.insert_char('b');
        assert_eq!(state.active_query(), None);

        state.is_active = true;
        assert_eq!(state.active_query(), Some("ab"));
        assert_eq!(state.query(), "ab");
    }
}
