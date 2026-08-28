use kanban_domain::CardFilters;
use std::cell::Cell;

#[derive(Debug, Clone, PartialEq)]
pub enum FilterDialogSection {
    Sprints,
    DateRange,
    Tags,
}

#[derive(Debug, Clone)]
pub struct FilterDialogState {
    pub current_section: FilterDialogSection,
    pub section_index: usize,
    pub item_selection: usize,
    pub item_scroll: Cell<usize>,
    pub filters: CardFilters,
}

impl FilterDialogState {
    pub fn new(filters: CardFilters) -> Self {
        Self {
            current_section: FilterDialogSection::Sprints,
            section_index: 0,
            item_selection: 0,
            item_scroll: Cell::new(0),
            filters,
        }
    }

    pub fn next_section(&mut self) {
        self.section_index = (self.section_index + 1) % 3;
        self.item_selection = 0;
        self.item_scroll.set(0);
        self.current_section = match self.section_index {
            0 => FilterDialogSection::Sprints,
            1 => FilterDialogSection::DateRange,
            2 => FilterDialogSection::Tags,
            _ => FilterDialogSection::Sprints,
        };
    }

    pub fn prev_section(&mut self) {
        self.section_index = if self.section_index == 0 {
            2
        } else {
            self.section_index - 1
        };
        self.item_selection = 0;
        self.item_scroll.set(0);
        self.current_section = match self.section_index {
            0 => FilterDialogSection::Sprints,
            1 => FilterDialogSection::DateRange,
            2 => FilterDialogSection::Tags,
            _ => FilterDialogSection::Sprints,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_starts_on_sprints_section() {
        let state = FilterDialogState::new(CardFilters::default());
        assert_eq!(state.current_section, FilterDialogSection::Sprints);
        assert_eq!(state.section_index, 0);
        assert_eq!(state.item_selection, 0);
        assert_eq!(state.item_scroll.get(), 0);
    }

    #[test]
    fn test_next_section_cycles_sprints_daterange_tags_sprints() {
        let mut state = FilterDialogState::new(CardFilters::default());

        state.next_section();
        assert_eq!(state.current_section, FilterDialogSection::DateRange);

        state.next_section();
        assert_eq!(state.current_section, FilterDialogSection::Tags);

        state.next_section();
        assert_eq!(state.current_section, FilterDialogSection::Sprints);
    }

    #[test]
    fn test_prev_section_wraps_from_sprints_to_tags() {
        let mut state = FilterDialogState::new(CardFilters::default());

        state.prev_section();
        assert_eq!(state.current_section, FilterDialogSection::Tags);
        assert_eq!(state.section_index, 2);
    }

    #[test]
    fn test_next_section_resets_item_selection_and_scroll() {
        let mut state = FilterDialogState::new(CardFilters::default());
        state.item_selection = 3;
        state.item_scroll.set(5);

        state.next_section();

        assert_eq!(state.item_selection, 0);
        assert_eq!(state.item_scroll.get(), 0);
    }
}
