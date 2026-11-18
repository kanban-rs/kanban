//! Card filter configuration.
//!
//! Provides the CardFilters struct which holds filter settings for querying cards.

use std::collections::HashSet;
use uuid::Uuid;

/// Configuration for filtering cards by various criteria.
///
/// This struct holds the filter settings (what to filter by) as opposed to
/// CardFilter trait implementations which perform the actual filtering.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CardFilters {
    /// Include cards not assigned to any sprint.
    pub show_unassigned_sprints: bool,
    /// Filter to cards in these specific sprints.
    pub selected_sprint_ids: HashSet<Uuid>,
    /// Filter by creation/due date range start.
    pub date_from: Option<String>,
    /// Filter by creation/due date range end.
    pub date_to: Option<String>,
    /// Filter to cards with these labels.
    pub selected_labels: HashSet<String>,
}

impl CardFilters {
    /// Create a new empty filter configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any filters are active.
    pub fn has_active_filters(&self) -> bool {
        self.show_unassigned_sprints
            || !self.selected_sprint_ids.is_empty()
            || self.date_from.is_some()
            || self.date_to.is_some()
            || !self.selected_labels.is_empty()
    }

    /// Clear all filters.
    pub fn clear(&mut self) {
        self.show_unassigned_sprints = false;
        self.selected_sprint_ids.clear();
        self.date_from = None;
        self.date_to = None;
        self.selected_labels.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_has_no_active_filters() {
        let filters = CardFilters::default();
        assert!(!filters.has_active_filters());
    }

    #[test]
    fn test_has_active_filters_with_sprint() {
        let mut filters = CardFilters::new();
        filters.selected_sprint_ids.insert(Uuid::new_v4());
        assert!(filters.has_active_filters());
    }

    #[test]
    fn test_has_active_filters_with_unassigned() {
        let mut filters = CardFilters::new();
        filters.show_unassigned_sprints = true;
        assert!(filters.has_active_filters());
    }

    #[test]
    fn test_clear_filters() {
        let mut filters = CardFilters::new();
        filters.show_unassigned_sprints = true;
        filters.selected_sprint_ids.insert(Uuid::new_v4());
        filters.date_from = Some("2024-01-01".to_string());
        filters.selected_labels.insert("urgent".to_string());

        assert!(filters.has_active_filters());

        filters.clear();
        assert!(!filters.has_active_filters());
    }
}
