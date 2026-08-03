use kanban_core::SelectionState;
use kanban_core::{Page, PageInfo};
use std::collections::HashSet;

pub type ListRenderInfo = PageInfo;

#[derive(Clone)]
pub struct ListComponent {
    pub selection: SelectionState,
    page: Page,
    pub multi_selected: HashSet<usize>,
    pub allow_multi_select: bool,
}

impl ListComponent {
    pub fn new(allow_multi_select: bool) -> Self {
        Self {
            selection: SelectionState::new(),
            page: Page::new(0),
            multi_selected: HashSet::new(),
            allow_multi_select,
        }
    }

    pub fn update_item_count(&mut self, count: usize) {
        let current_selection = self.selection.get();
        self.page.set_total_items(count);

        if let Some(idx) = current_selection {
            if idx >= count && count > 0 {
                self.selection.set(Some(count - 1));
            } else if count == 0 {
                self.selection.clear();
            }
        } else if count > 0 {
            self.selection.auto_select_first_if_empty(true);
        }
    }

    pub fn navigate_up(&mut self) -> bool {
        if self.page.total_items == 0 {
            return true;
        }

        let was_at_top = self.selection.get() == Some(0) || self.selection.get().is_none();

        if !was_at_top {
            let current_idx = self.selection.get().unwrap_or(0);
            // Invariant: total > 0 (early return above) and predicate is
            // always-true, so list_nav always returns Some.
            let new_idx = crate::list_nav::prev_selectable_index(
                Some(current_idx),
                self.page.total_items,
                |_| true,
            )
            .expect("list_nav returns Some when total > 0 with always-true predicate");
            self.selection.set(Some(new_idx));

            // Scroll up if selection moved before scroll window
            if new_idx < self.page.scroll_offset {
                self.page.set_scroll_offset(new_idx);
            }
        }

        was_at_top
    }

    pub fn navigate_down(&mut self) -> bool {
        if self.page.total_items == 0 {
            return true;
        }

        let was_at_bottom = self.selection.get() == Some(self.page.total_items - 1);

        if !was_at_bottom {
            let current_idx = self.selection.get().unwrap_or(0);
            // Invariant: total > 0 (early return above) and predicate is
            // always-true, so list_nav always returns Some.
            let new_idx = crate::list_nav::next_selectable_index(
                Some(current_idx),
                self.page.total_items,
                |_| true,
            )
            .expect("list_nav returns Some when total > 0 with always-true predicate");
            self.selection.set(Some(new_idx));
        }

        was_at_bottom
    }

    pub fn get_selected_index(&self) -> Option<usize> {
        self.selection.get()
    }

    pub fn set_selected_index(&mut self, index: Option<usize>) {
        if let Some(idx) = index {
            if idx < self.page.total_items {
                self.selection.set(Some(idx));
            } else {
                self.selection.clear();
            }
        } else {
            self.selection.clear();
        }
    }

    pub fn toggle_multi_select(&mut self, index: usize) {
        if self.allow_multi_select && index < self.page.total_items {
            if self.multi_selected.contains(&index) {
                self.multi_selected.remove(&index);
            } else {
                self.multi_selected.insert(index);
            }
        }
    }

    pub fn clear_multi_select(&mut self) {
        self.multi_selected.clear();
    }

    pub fn select_all(&mut self) {
        if self.allow_multi_select {
            self.multi_selected = (0..self.page.total_items).collect();
        }
    }

    pub fn get_multi_selected_indices(&self) -> Vec<usize> {
        let mut indices: Vec<_> = self.multi_selected.iter().copied().collect();
        indices.sort_unstable();
        indices
    }

    pub fn get_scroll_offset(&self) -> usize {
        self.page.scroll_offset
    }

    pub fn set_scroll_offset(&mut self, offset: usize) {
        self.page.set_scroll_offset(offset);
    }

    /// Ensure selected item is visible by scrolling if needed
    pub fn ensure_selected_visible(&mut self, viewport_height: usize) {
        if let Some(selected_idx) = self.selection.get() {
            self.page.scroll_to_visible(selected_idx, viewport_height);
        }
    }

    /// Calculate viewport height adjusted for scroll indicators
    ///
    /// This accounts for the visual overhead of "X items above" and "X items below"
    /// indicators that appear when scrolling. The adjusted viewport represents the
    /// actual number of content lines available for rendering items.
    ///
    /// # Arguments
    /// * `raw_viewport_height` - The total height available (excluding borders)
    ///
    /// # Returns
    /// The number of lines available for actual item content after accounting for indicators
    pub fn get_adjusted_viewport_height(&self, raw_viewport_height: usize) -> usize {
        self.page.get_adjusted_viewport_height(raw_viewport_height)
    }

    /// Get rendering information given a viewport height (raw lines, all overhead pre-accounted)
    pub fn get_render_info(&self, viewport_height: usize) -> ListRenderInfo {
        self.page.get_page_info(viewport_height)
    }

    /// Direct navigation - jump to a specific index
    pub fn jump_to(&mut self, index: usize) {
        if index < self.page.total_items {
            self.selection.set(Some(index));
        }
    }

    /// Get total item count
    pub fn len(&self) -> usize {
        self.page.total_items
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.page.total_items == 0
    }

    /// Reset list state: clear item count, selection, and scroll position.
    ///
    /// `update_item_count(0)` implicitly clears the selection; the explicit
    /// `set_scroll_offset(0)` ensures no stale scroll position remains.
    pub fn reset(&mut self) {
        self.update_item_count(0);
        self.page.set_scroll_offset(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_list(item_count: usize) -> ListComponent {
        let mut list = ListComponent::new(false);
        list.update_item_count(item_count);
        list
    }

    #[test]
    fn test_navigate_down() {
        let mut list = create_test_list(5);
        assert_eq!(list.get_selected_index(), Some(0));

        list.navigate_down();
        assert_eq!(list.get_selected_index(), Some(1));

        list.navigate_down();
        assert_eq!(list.get_selected_index(), Some(2));
    }

    #[test]
    fn test_navigate_up() {
        let mut list = create_test_list(5);
        list.set_selected_index(Some(2));

        list.navigate_up();
        assert_eq!(list.get_selected_index(), Some(1));

        list.navigate_up();
        assert_eq!(list.get_selected_index(), Some(0));
    }

    #[test]
    fn test_navigate_down_at_boundary() {
        let mut list = create_test_list(5);
        list.set_selected_index(Some(4));

        let was_at_bottom = list.navigate_down();
        assert!(was_at_bottom);
        assert_eq!(list.get_selected_index(), Some(4));
    }

    #[test]
    fn test_navigate_up_at_boundary() {
        let mut list = create_test_list(5);
        list.set_selected_index(Some(0));

        let was_at_top = list.navigate_up();
        assert!(was_at_top);
        assert_eq!(list.get_selected_index(), Some(0));
    }

    #[test]
    fn test_multi_select() {
        let mut list = ListComponent::new(true);
        list.update_item_count(5);

        list.toggle_multi_select(1);
        list.toggle_multi_select(3);

        assert!(list.multi_selected.contains(&1));
        assert!(list.multi_selected.contains(&3));
        assert_eq!(list.get_multi_selected_indices(), vec![1, 3]);
    }

    #[test]
    fn test_clear_multi_select() {
        let mut list = ListComponent::new(true);
        list.update_item_count(5);
        list.toggle_multi_select(1);
        list.toggle_multi_select(3);

        list.clear_multi_select();
        assert_eq!(list.get_multi_selected_indices().len(), 0);
    }

    #[test]
    fn test_select_all() {
        let mut list = ListComponent::new(true);
        list.update_item_count(5);

        list.select_all();
        assert_eq!(list.get_multi_selected_indices().len(), 5);
    }

    #[test]
    fn test_empty_list() {
        let mut list = create_test_list(0);
        assert_eq!(list.get_selected_index(), None);

        list.navigate_down();
        assert_eq!(list.get_selected_index(), None);

        list.navigate_up();
        assert_eq!(list.get_selected_index(), None);
    }

    #[test]
    fn test_update_item_count_keeps_selection() {
        let mut list = create_test_list(5);
        list.set_selected_index(Some(2));

        list.update_item_count(10);
        assert_eq!(list.get_selected_index(), Some(2));
    }

    #[test]
    fn test_update_item_count_clamps_selection() {
        let mut list = create_test_list(10);
        list.set_selected_index(Some(8));

        list.update_item_count(5);
        assert_eq!(list.get_selected_index(), Some(4));
    }

    #[test]
    fn test_render_info_basic() {
        let list = create_test_list(5);
        let info = list.get_render_info(3);

        assert!(!info.show_above_indicator);
        assert_eq!(info.visible_indices, vec![0, 1, 2]);
        assert!(info.show_below_indicator);
    }

    #[test]
    fn test_render_info_with_scroll() {
        let mut list = create_test_list(10);
        list.set_scroll_offset(3);

        let info = list.get_render_info(3);
        assert!(info.show_above_indicator);
        assert_eq!(info.items_above, 3);
    }

    #[test]
    fn test_multi_select_disabled() {
        let mut list = ListComponent::new(false);
        list.update_item_count(5);

        list.toggle_multi_select(1);
        assert!(list.multi_selected.is_empty());
    }

    #[test]
    fn test_jump_to() {
        let mut list = create_test_list(10);
        list.jump_to(5);
        assert_eq!(list.get_selected_index(), Some(5));
    }

    #[test]
    fn test_ensure_selected_visible() {
        let mut list = create_test_list(20);
        list.jump_to(15);
        list.ensure_selected_visible(5);

        let info = list.get_render_info(5);
        assert!(info.visible_indices.contains(&15));
    }

    #[test]
    fn test_navigation_with_adjusted_viewport_prevents_scrolling_past_indicator() {
        // Scenario: 20 cards, raw viewport 10 lines
        // When indicators appear, adjusted viewport is 8-9 lines (accounting for indicators)
        let mut list = create_test_list(20);

        // Start at card 0
        assert_eq!(list.get_selected_index(), Some(0));

        // Simulate navigation down to card 10 with adjusted viewport of 8
        // (raw viewport 10 minus 2 for both indicators)
        for _ in 0..10 {
            list.navigate_down();
        }

        assert_eq!(list.get_selected_index(), Some(10));

        // Ensure selected visible with adjusted viewport of 8
        // This should scroll the list so card 10 is visible
        let adjusted_viewport = 8;
        list.ensure_selected_visible(adjusted_viewport);

        // Verify card 10 is visible with the adjusted viewport
        let info = list.get_render_info(adjusted_viewport);
        assert!(
            info.visible_indices.contains(&10),
            "Card 10 should be visible. Visible indices: {:?}, scroll_offset: {}",
            info.visible_indices,
            list.get_scroll_offset()
        );

        // Verify the selection is placed correctly
        // The scroll should place it roughly in the middle or bottom of visible area
        let scroll_offset = list.get_scroll_offset();
        let scroll_end = scroll_offset + adjusted_viewport;
        assert!(
            scroll_offset <= 10 && 10 < scroll_end,
            "Card 10 should be within the viewport range [{}, {})",
            scroll_offset,
            scroll_end
        );
    }

    #[test]
    fn test_navigation_down_then_up_with_adjusted_viewport() {
        // Test the full cycle: navigate down, then navigate up
        let mut list = create_test_list(20);

        // Navigate down to card 5
        for _ in 0..5 {
            list.navigate_down();
        }
        assert_eq!(list.get_selected_index(), Some(5));

        // Ensure visible with adjusted viewport (accounts for indicators)
        let adjusted_viewport = 8;
        list.ensure_selected_visible(adjusted_viewport);

        // Verify it's visible
        let info = list.get_render_info(adjusted_viewport);
        assert!(info.visible_indices.contains(&5));

        // Navigate up back to card 0
        for _ in 0..5 {
            list.navigate_up();
        }
        assert_eq!(list.get_selected_index(), Some(0));

        // Ensure visible again with adjusted viewport
        list.ensure_selected_visible(adjusted_viewport);

        // Verify card 0 is visible and not scrolled past indicator
        let info = list.get_render_info(adjusted_viewport);
        assert!(info.visible_indices.contains(&0));
        assert_eq!(list.get_scroll_offset(), 0, "Should be at top of list");
    }

    #[test]
    fn test_get_adjusted_viewport_height_no_scroll() {
        // When not scrolled, no "above" indicator, but check for "below"
        let list = create_test_list(10);

        // Raw viewport 5, items 0-4 visible (5 total)
        // Remaining: 5 items below → show "below" indicator
        // Adjusted: 5 - 0 (above) - 1 (below) = 4
        let adjusted = list.get_adjusted_viewport_height(5);
        assert_eq!(adjusted, 4);
    }

    #[test]
    fn test_get_adjusted_viewport_height_scrolled_middle() {
        // When scrolled in middle, both indicators shown
        let mut list = create_test_list(10);
        list.set_scroll_offset(3);

        // Raw viewport 5
        // Above indicator: 1 (scrolled)
        // Available: 5 - 1 = 4
        // Items 3-6 would be visible (4 total)
        // Remaining: 3 items below (7-9) → show "below" indicator
        // Adjusted: 4 - 1 (below) = 3
        let adjusted = list.get_adjusted_viewport_height(5);
        assert_eq!(adjusted, 3);
    }

    #[test]
    fn test_get_adjusted_viewport_height_scrolled_end() {
        // When scrolled to end, only "above" indicator
        let mut list = create_test_list(10);
        list.set_scroll_offset(6);

        // Raw viewport 5
        // Above indicator: 1 (scrolled)
        // Available: 5 - 1 = 4
        // Items 6-9 would be visible (4 total) - exactly fits!
        // Remaining: 0 items below → no "below" indicator
        // Adjusted: 4 - 0 (below) = 4
        let adjusted = list.get_adjusted_viewport_height(5);
        assert_eq!(adjusted, 4);
    }

    #[test]
    fn test_get_adjusted_viewport_height_all_fit() {
        // When all items fit, no indicators
        let list = create_test_list(5);

        // Raw viewport 5, all items fit
        // No indicators needed
        // Adjusted: 5
        let adjusted = list.get_adjusted_viewport_height(5);
        assert_eq!(adjusted, 5);
    }

    #[test]
    fn test_get_adjusted_viewport_height_empty_list() {
        let list = create_test_list(0);
        let adjusted = list.get_adjusted_viewport_height(5);
        assert_eq!(adjusted, 5); // No adjustment for empty list
    }

    #[test]
    fn test_reset_clears_items_and_selection() {
        let mut list = ListComponent::new(false);
        list.update_item_count(10);
        list.set_selected_index(Some(5));
        list.set_scroll_offset(3);

        list.reset();

        assert_eq!(list.len(), 0);
        assert_eq!(list.get_selected_index(), None);
        assert_eq!(list.get_scroll_offset(), 0);
    }

    #[test]
    fn test_navigation_with_adjusted_viewport_bug_scenario() {
        // Exact bug scenario: 6 cards in viewport 5
        // User reports card 6 not visible when scrolling
        let mut list = create_test_list(6);

        // Start at card 0
        assert_eq!(list.get_selected_index(), Some(0));

        // Navigate down to card 5 (6th card)
        for _ in 0..5 {
            list.navigate_down();
        }
        assert_eq!(list.get_selected_index(), Some(5));

        // Use adjusted viewport (should account for indicators)
        let adjusted_viewport = list.get_adjusted_viewport_height(5);
        list.ensure_selected_visible(adjusted_viewport);

        // Verify card 5 is visible
        let info = list.get_render_info(adjusted_viewport);
        assert!(
            info.visible_indices.contains(&5),
            "Card 5 should be visible. Visible: {:?}, scroll: {}, viewport: {}",
            info.visible_indices,
            list.get_scroll_offset(),
            adjusted_viewport
        );
    }
}
