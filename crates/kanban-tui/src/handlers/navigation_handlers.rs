use crate::app::{App, AppMode, Focus};
use crate::view_strategy::UnifiedViewStrategy;
use kanban_domain::TaskListView;

/// Page boundary with start index and end index (exclusive)
#[derive(Debug, Clone, Copy)]
struct PageBoundary {
    start: usize,
    end: usize,
}

impl PageBoundary {
    fn contains(&self, index: usize) -> bool {
        index >= self.start && index < self.end
    }

    fn size(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

/// Count how many column headers will appear in the viewport
/// A boundary is relevant if any of its cards appear in the visible range
fn count_headers_in_viewport(
    column_boundaries: &[(usize, usize)], // (start_index, card_count) tuples
    scroll_offset: usize,
    viewport_height: usize,
) -> usize {
    if column_boundaries.is_empty() {
        return 0;
    }

    let viewport_end = scroll_offset + viewport_height;

    column_boundaries
        .iter()
        .filter(|(start_index, card_count)| {
            let boundary_end = start_index + card_count;
            // Boundary is relevant if it overlaps with viewport range
            *start_index < viewport_end && boundary_end > scroll_offset
        })
        .count()
}

/// Compute variable-sized page boundaries for jumping navigation.
///
/// Pages are sized based on position:
/// - First page: viewport_height - 1 (only "below" indicator)
/// - Middle pages: viewport_height - 2 (both indicators)
/// - Last page: remaining items (only "above" indicator, can use up to viewport_height - 1)
fn compute_page_boundaries(total_items: usize, viewport_height: usize) -> Vec<PageBoundary> {
    if total_items == 0 || viewport_height == 0 {
        return vec![];
    }

    let mut pages = Vec::new();

    // First page: viewport_height - 1 items
    let first_page_size = viewport_height.saturating_sub(1).max(1);
    let first_page_end = first_page_size.min(total_items);
    pages.push(PageBoundary {
        start: 0,
        end: first_page_end,
    });
    let mut current_idx = first_page_end;

    // Middle and last pages: viewport_height - 2 items each, except last page uses viewport_height - 1
    let middle_page_size = viewport_height.saturating_sub(2).max(1);
    let last_page_max_size = viewport_height.saturating_sub(1).max(1);

    while current_idx < total_items {
        let remaining = total_items - current_idx;

        // Check if remaining items fit in a last page (viewport_height - 1)
        let is_last_page = remaining <= last_page_max_size;

        let page_size = if is_last_page {
            last_page_max_size
        } else {
            middle_page_size
        };

        let page_end = (current_idx + page_size).min(total_items);
        pages.push(PageBoundary {
            start: current_idx,
            end: page_end,
        });
        current_idx = page_end;
    }

    pages
}

/// Compute page boundaries for grouped view, accounting for column headers causing overflow
///
/// Headers take up viewport space, so we need to dynamically reduce page sizes
/// to ensure cards don't spill into the next page when rendered.
fn compute_page_boundaries_with_headers(
    total_items: usize,
    viewport_height: usize,
    column_boundaries: &[(usize, usize)], // (start_index, card_count)
) -> Vec<PageBoundary> {
    if total_items == 0 || viewport_height == 0 {
        return vec![];
    }

    let mut pages = Vec::new();
    let mut current_idx = 0;
    let first_page_base = viewport_height.saturating_sub(1).max(1);
    let middle_page_base = viewport_height.saturating_sub(2).max(1);
    let last_page_base = viewport_height.saturating_sub(1).max(1);

    let mut is_first_page = true;

    while current_idx < total_items {
        let remaining = total_items - current_idx;

        // Determine base page size (already accounts for indicators)
        let is_last_page = remaining <= last_page_base;
        let base_size = if is_first_page {
            first_page_base
        } else if is_last_page {
            last_page_base
        } else {
            middle_page_base
        };

        // Count headers that will appear in this page's visible range
        let headers_count = count_headers_in_viewport(column_boundaries, current_idx, base_size);

        // Calculate actual page size after accounting for headers
        // (base_size already accounts for indicators)
        let adjusted_size = base_size.saturating_sub(headers_count);

        // Ensure we make progress (at least 1 card per page)
        let actual_size = adjusted_size.max(1);

        let page_end = (current_idx + actual_size).min(total_items);
        pages.push(PageBoundary {
            start: current_idx,
            end: page_end,
        });

        current_idx = page_end;
        is_first_page = false;
    }

    pages
}

/// Find which page contains the given index
fn find_current_page(pages: &[PageBoundary], index: usize) -> Option<usize> {
    pages.iter().position(|page| page.contains(index))
}

/// Calculate jump target for page-down navigation (Ctrl+D style)
/// Implements three-level navigation: top → middle → bottom → next page
fn calculate_jump_target_down(
    pages: &[PageBoundary],
    current_index: usize,
    current_page_idx: usize,
) -> usize {
    let current_page = pages[current_page_idx];

    // Determine position within current page
    let position_in_page = current_index - current_page.start;
    let page_size = current_page.size();

    if page_size == 0 {
        return current_index;
    }

    // Three-level jumping: top (0) → middle (size/2) → bottom (size-1) → next page
    (if position_in_page < page_size / 2 {
        // Currently at top, jump to middle
        current_page.start + page_size / 2
    } else if position_in_page < page_size - 1 {
        // Currently in middle/near-bottom, jump to bottom
        current_page.end - 1
    } else if current_page_idx < pages.len() - 1 {
        // Currently at bottom, jump to top of next page
        pages[current_page_idx + 1].start
    } else {
        // At bottom of last page, stay there
        current_index
    })
    .min(
        pages
            .iter()
            .map(|p| p.end)
            .max()
            .unwrap_or(0)
            .saturating_sub(1),
    )
}

/// Calculate jump target for page-up navigation (Ctrl+U style)
/// Implements three-level navigation: bottom → middle → top → previous page
fn calculate_jump_target_up(
    pages: &[PageBoundary],
    current_index: usize,
    current_page_idx: usize,
) -> usize {
    let current_page = pages[current_page_idx];

    // Determine position within current page
    let position_in_page = current_index - current_page.start;
    let page_size = current_page.size();

    if page_size == 0 {
        return current_index;
    }

    // Three-level jumping: bottom (size-1) → middle (size/2) → top (0) → previous page
    if position_in_page > page_size / 2 {
        // Currently at bottom, jump to middle
        current_page.start + page_size / 2
    } else if position_in_page > 0 {
        // Currently at top/near-top, jump to top
        current_page.start
    } else if current_page_idx > 0 {
        // Currently at top of page, jump to bottom of previous page
        pages[current_page_idx - 1].end - 1
    } else {
        // At top of first page, stay there
        current_index
    }
}

impl App {
    /// Calculate actual usable viewport height accounting for indicators and headers
    /// Must match the rendering logic to ensure page boundaries align with visible cards
    fn get_adjusted_viewport_height(&self) -> usize {
        let raw_viewport = self.view.viewport_height;

        if let Some(list) = self.view.strategy.get_active_task_list() {
            let scroll_offset = list.get_scroll_offset();

            // Calculate "above" indicator overhead (fixed based on scroll position)
            let above_indicator_height = if scroll_offset > 0 { 1 } else { 0 };

            // Start with space available after above indicator
            let available_space = raw_viewport.saturating_sub(above_indicator_height);

            // Count column header overhead (only in GroupedByColumn view)
            let mut header_overhead = 0;

            // Try to get column boundaries from UnifiedViewStrategy
            if let Some(unified) = self
                .view
                .strategy
                .as_any()
                .downcast_ref::<UnifiedViewStrategy>()
            {
                use kanban_view::layout_strategy::VirtualUnifiedLayout;

                if let Some(layout) = unified
                    .get_layout_strategy()
                    .as_any()
                    .downcast_ref::<VirtualUnifiedLayout>()
                {
                    let boundaries = layout.get_column_boundaries();
                    let column_boundaries: Vec<(usize, usize)> = boundaries
                        .iter()
                        .map(|b| (b.start_index, b.card_count))
                        .collect();

                    // Initial estimate based on available space
                    let initial_header_count = count_headers_in_viewport(
                        &column_boundaries,
                        scroll_offset,
                        available_space,
                    );

                    // Refine: count headers for only the cards that will actually be visible
                    let initial_card_slots = available_space.saturating_sub(initial_header_count);
                    header_overhead = count_headers_in_viewport(
                        &column_boundaries,
                        scroll_offset,
                        initial_card_slots,
                    );
                }
            }

            // Calculate card slots with refined header count
            let card_slots = available_space.saturating_sub(header_overhead);

            // Check if we need "below" indicator based on actual visible cards
            let below_indicator_height = if scroll_offset + card_slots < list.len() {
                1
            } else {
                0
            };

            return card_slots.saturating_sub(below_indicator_height);
        }

        raw_viewport
    }

    /// Compute page boundaries appropriate for the current view mode
    fn compute_pages_for_current_view(&self, total_items: usize) -> Vec<PageBoundary> {
        // For GroupedByColumn view, use header-aware pagination
        if let Some(board) = self.active_board() {
            if board.task_list_view == TaskListView::GroupedByColumn {
                // Try to get column boundaries for header-aware pagination
                if let Some(unified) = self
                    .view
                    .strategy
                    .as_any()
                    .downcast_ref::<UnifiedViewStrategy>()
                {
                    use kanban_view::layout_strategy::VirtualUnifiedLayout;

                    if let Some(layout) = unified
                        .get_layout_strategy()
                        .as_any()
                        .downcast_ref::<VirtualUnifiedLayout>()
                    {
                        let boundaries = layout.get_column_boundaries();
                        let column_boundaries: Vec<(usize, usize)> = boundaries
                            .iter()
                            .map(|b| (b.start_index, b.card_count))
                            .collect();

                        return compute_page_boundaries_with_headers(
                            total_items,
                            self.view.viewport_height,
                            &column_boundaries,
                        );
                    }
                }
            }
        }

        // For Flat view or any other view, use standard pagination
        compute_page_boundaries(total_items, self.view.viewport_height)
    }

    pub fn handle_focus_switch(&mut self, focus_target: Focus) {
        match focus_target {
            Focus::Boards => {
                self.focus.active = Focus::Boards;
            }
            Focus::Cards => {
                if self.selection.active_board_id.is_some() {
                    self.focus.active = Focus::Cards;
                }
            }
        }
    }

    pub fn handle_navigation_down(&mut self) {
        match self.focus.active {
            Focus::Boards => {
                self.board_list.navigate_down();
                if self.mode != AppMode::ArchivedBoardsView {
                    self.switch_view_strategy(TaskListView::GroupedByColumn);
                }
            }
            Focus::Cards => {
                // Get initial adjusted viewport before mutable borrow
                let initial_adjusted_viewport = self.get_adjusted_viewport_height();

                // Simple smooth navigation: move by 1 with viewport spanning pages
                let was_at_bottom =
                    if let Some(list) = self.view.strategy.get_active_task_list_mut() {
                        let was_at_bottom = list.navigate_down();

                        // Smooth scroll with initial viewport
                        list.ensure_selected_visible(initial_adjusted_viewport);

                        was_at_bottom
                    } else {
                        false
                    };

                // Recalculate viewport after scroll may have changed indicators/headers
                let final_adjusted_viewport = self.get_adjusted_viewport_height();
                if final_adjusted_viewport != initial_adjusted_viewport {
                    if let Some(list) = self.view.strategy.get_active_task_list_mut() {
                        list.ensure_selected_visible(final_adjusted_viewport);
                    }
                }

                // If selection mode active, add the new current card to selection
                if self.multi_select.selection_mode_active {
                    if let Some(card) = self.get_selected_card_in_context() {
                        self.multi_select.selected_cards.insert(card.id);
                    }
                }

                // Check for bottom navigation: only switch columns if we were ALREADY at bottom
                if was_at_bottom {
                    self.view.strategy.navigate_right(false);
                }
            }
        }
    }

    pub fn handle_navigation_up(&mut self) {
        match self.focus.active {
            Focus::Boards => {
                self.board_list.navigate_up();
                if self.mode != AppMode::ArchivedBoardsView {
                    self.switch_view_strategy(TaskListView::GroupedByColumn);
                }
            }
            Focus::Cards => {
                // Get initial adjusted viewport before mutable borrow
                let initial_adjusted_viewport = self.get_adjusted_viewport_height();

                // Simple smooth navigation: move by 1 with viewport spanning pages
                let was_at_top = if let Some(list) = self.view.strategy.get_active_task_list_mut() {
                    let was_at_top = list.navigate_up();

                    // Smooth scroll with initial viewport
                    list.ensure_selected_visible(initial_adjusted_viewport);

                    was_at_top
                } else {
                    false
                };

                // Recalculate viewport after scroll may have changed indicators/headers
                let final_adjusted_viewport = self.get_adjusted_viewport_height();
                if final_adjusted_viewport != initial_adjusted_viewport {
                    if let Some(list) = self.view.strategy.get_active_task_list_mut() {
                        list.ensure_selected_visible(final_adjusted_viewport);
                    }
                }

                // If selection mode active, add the new current card to selection
                if self.multi_select.selection_mode_active {
                    if let Some(card) = self.get_selected_card_in_context() {
                        self.multi_select.selected_cards.insert(card.id);
                    }
                }

                // Check for top navigation: only switch columns if we were ALREADY at top
                if was_at_top {
                    self.view.strategy.navigate_left(true);
                }
            }
        }
    }

    pub fn handle_selection_activate(&mut self) {
        match self.focus.active {
            Focus::Boards => {
                // Activate the highlighted board from whichever set the projects
                // panel currently displays (live OR archived) — a board is a
                // board. From here on it is THE active board, tracked by id, and
                // every view/operation resolves it archival-agnostically.
                let activated = self.board_list.get_selected_board_id().and_then(|id| {
                    self.model
                        .board_by_id(id)
                        .map(|b| (b.id, b.task_list_view, b.task_sort_field, b.task_sort_order))
                });

                if let Some((board_id, task_list_view, task_sort_field, task_sort_order)) =
                    activated
                {
                    self.selection.active_board_id = Some(board_id);
                    self.filter.current_sort_field = Some(task_sort_field);
                    self.filter.current_sort_order = Some(task_sort_order);
                    self.switch_view_strategy(task_list_view);
                    // Populate the tasks panel from the now-active board's subtree
                    // immediately, so the first item can be selected this tick.
                    self.reload_model();
                    self.prepare_frame();

                    if let Some(list) = self.view.strategy.get_active_task_list_mut() {
                        if !list.is_empty() {
                            list.set_selected_index(Some(0));
                            list.ensure_selected_visible(self.view.viewport_height);
                        }
                    }

                    self.focus.active = Focus::Cards;
                }
            }
            Focus::Cards => {
                if let Some(selected_card) = self.get_selected_card_in_context() {
                    let card_id = selected_card.id;
                    self.set_active_card_or_clear(card_id);
                    // Initialize list components with item counts
                    let parents = self.get_current_card_parents();
                    let children = self.get_current_card_children();
                    self.relationship
                        .parents_list
                        .update_item_count(parents.len());
                    self.relationship
                        .children_list
                        .update_item_count(children.len());
                    self.push_mode(AppMode::CardDetail);
                }
            }
        }
    }

    pub fn handle_escape_key(&mut self) {
        if self.filter.search.is_active {
            self.filter.search.deactivate();
            return;
        }

        // Clear selection mode first (only when actively in selection mode)
        if self.multi_select.selection_mode_active {
            self.multi_select.selection_mode_active = false;
            self.multi_select.selected_cards.clear();
            return;
        }

        // Leaving the active board returns focus to the projects panel. The
        // panel keeps showing whichever set it was on (live or archived), so
        // escaping an archived board lands back on the archived list — no
        // archival-specific branch needed.
        if self.selection.active_board_id.is_some() {
            self.selection.active_board_id = None;
            self.focus.active = Focus::Boards;
            self.switch_view_strategy(TaskListView::GroupedByColumn);
        }
    }

    pub fn is_kanban_view(&self) -> bool {
        // Kanban (column) layout only applies to the ACTIVE board. While browsing
        // the projects list (no active board) the split projects+tasks layout is
        // used so the list stays visible — this holds for the live and archived
        // sets alike, with no archival-specific branch.
        if let Some(board) = self.active_board() {
            return board.task_list_view == TaskListView::ColumnView;
        }
        false
    }

    pub fn handle_kanban_column_left(&mut self) {
        if !self.is_kanban_view() || self.focus.active != Focus::Cards {
            return;
        }

        if self.view.strategy.navigate_left(false) {
            tracing::info!("Moved to previous column");
        }
    }

    pub fn handle_kanban_column_right(&mut self) {
        if !self.is_kanban_view() || self.focus.active != Focus::Cards {
            return;
        }

        if self.view.strategy.navigate_right(false) {
            tracing::info!("Moved to next column");
        }
    }

    pub fn handle_column_or_focus_switch(&mut self, index: usize) {
        if self.is_kanban_view() && self.focus.active == Focus::Cards {
            let column_count = self.view.strategy.get_all_task_lists().len();

            if index < column_count {
                self.view.strategy.try_navigate_to_column(index);

                if let Some(list) = self.view.strategy.get_active_task_list_mut() {
                    if list.is_empty() {
                        list.clear();
                    } else if list.get_selected_index().is_none() {
                        list.set_selected_index(Some(0));
                    }
                    list.ensure_selected_visible(self.view.viewport_height);
                }
                tracing::info!("Switched to column {}", index);
            }
        } else {
            match index {
                0 => self.handle_focus_switch(Focus::Boards),
                1 => self.handle_focus_switch(Focus::Cards),
                _ => {}
            }
        }
    }

    pub fn handle_jump_to_top(&mut self) {
        match self.focus.active {
            Focus::Boards => {
                self.board_list.jump_to_top();
                self.switch_view_strategy(TaskListView::GroupedByColumn);
            }
            Focus::Cards => {
                let adjusted_viewport = self.get_adjusted_viewport_height();
                if let Some(list) = self.view.strategy.get_active_task_list_mut() {
                    list.jump_to_top();
                    list.ensure_selected_visible(adjusted_viewport);
                }
            }
        }
    }

    pub fn handle_jump_to_bottom(&mut self) {
        match self.focus.active {
            Focus::Boards => {
                self.board_list.jump_to_bottom();
                if self.mode != AppMode::ArchivedBoardsView {
                    self.switch_view_strategy(TaskListView::GroupedByColumn);
                }
            }
            Focus::Cards => {
                // Get adjusted viewport first (immutable borrow), then do all mutable work
                let adjusted_viewport = self.get_adjusted_viewport_height();
                if let Some(list) = self.view.strategy.get_active_task_list_mut() {
                    if !list.is_empty() {
                        let last_idx = list.len() - 1;
                        list.jump_to(last_idx);
                    }
                    list.ensure_selected_visible(adjusted_viewport);
                }
            }
        }
    }

    /// Shared half-viewport (Ctrl+U/Ctrl+D) jump for the boards panel: same
    /// three-level page navigation as the cards panel, driven by the same
    /// `calculate_jump_target_up`/`_down` free functions, over `board_list`
    /// instead of the active task list.
    fn jump_board_list_half_viewport(
        &mut self,
        calc_target: fn(&[PageBoundary], usize, usize) -> usize,
    ) {
        let total_items = self.board_list.len();
        if total_items == 0 {
            return;
        }

        let pages = compute_page_boundaries(total_items, self.view.viewport_height);
        if pages.is_empty() {
            return;
        }

        if let Some(current_idx) = self.board_list.get_selected_index() {
            if let Some(page_idx) = find_current_page(&pages, current_idx) {
                let target = calc_target(&pages, current_idx, page_idx);

                if let Some(target_page_idx) = find_current_page(&pages, target) {
                    if target_page_idx != page_idx {
                        let target_page = pages[target_page_idx];
                        self.board_list
                            .inner_mut()
                            .set_scroll_offset(target_page.start);
                    }
                }

                self.board_list.jump_to(target);
            }
        }
    }

    pub fn handle_jump_half_viewport_up(&mut self) {
        if self.focus.active == Focus::Boards {
            self.jump_board_list_half_viewport(calculate_jump_target_up);
            return;
        }
        if self.focus.active == Focus::Cards {
            // Get total items before borrowing mutably
            let total_items = self
                .view
                .strategy
                .get_active_task_list()
                .map(|list| list.len())
                .unwrap_or(0);

            if total_items == 0 {
                return;
            }

            // Compute pages and adjusted viewport before mutable borrow
            let pages = self.compute_pages_for_current_view(total_items);
            let adjusted_viewport = self.get_adjusted_viewport_height();

            if pages.is_empty() {
                return;
            }

            // Now borrow list mutably
            if let Some(list) = self.view.strategy.get_active_task_list_mut() {
                // Find current page and position
                if let Some(current_idx) = list.get_selected_index() {
                    if let Some(page_idx) = find_current_page(&pages, current_idx) {
                        let target = calculate_jump_target_up(&pages, current_idx, page_idx);

                        // If jumping to a different page, explicitly scroll to that page
                        if let Some(target_page_idx) = find_current_page(&pages, target) {
                            if target_page_idx != page_idx {
                                let target_page = pages[target_page_idx];
                                list.set_scroll_offset(target_page.start);
                            }
                        }

                        list.jump_to(target);
                        list.ensure_selected_visible(adjusted_viewport);
                    }
                }
            }
        }
    }

    pub fn handle_jump_half_viewport_down(&mut self) {
        if self.focus.active == Focus::Boards {
            self.jump_board_list_half_viewport(calculate_jump_target_down);
            return;
        }
        if self.focus.active == Focus::Cards {
            // Get total items before borrowing mutably
            let total_items = self
                .view
                .strategy
                .get_active_task_list()
                .map(|list| list.len())
                .unwrap_or(0);

            if total_items == 0 {
                return;
            }

            // Compute pages and adjusted viewport before mutable borrow
            let pages = self.compute_pages_for_current_view(total_items);
            let adjusted_viewport = self.get_adjusted_viewport_height();

            if pages.is_empty() {
                return;
            }

            // Now borrow list mutably
            if let Some(list) = self.view.strategy.get_active_task_list_mut() {
                // Find current page and position
                if let Some(current_idx) = list.get_selected_index() {
                    if let Some(page_idx) = find_current_page(&pages, current_idx) {
                        let target = calculate_jump_target_down(&pages, current_idx, page_idx);

                        // If jumping to a different page, explicitly scroll to that page
                        if let Some(target_page_idx) = find_current_page(&pages, target) {
                            if target_page_idx != page_idx {
                                let target_page = pages[target_page_idx];
                                list.set_scroll_offset(target_page.start);
                            }
                        }

                        list.jump_to(target);
                        list.ensure_selected_visible(adjusted_viewport);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create column boundaries
    fn column_boundary(start_index: usize, card_count: usize) -> (usize, usize) {
        (start_index, card_count)
    }

    // ============================================================================
    // compute_page_boundaries() Tests
    // ============================================================================

    #[test]
    fn test_compute_page_boundaries_empty() {
        let pages = compute_page_boundaries(0, 10);
        assert!(pages.is_empty());
    }

    #[test]
    fn test_compute_page_boundaries_zero_viewport() {
        let pages = compute_page_boundaries(100, 0);
        assert!(pages.is_empty());
    }

    #[test]
    fn test_compute_page_boundaries_single_page() {
        let pages = compute_page_boundaries(5, 10);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].start, 0);
        assert_eq!(pages[0].end, 5);
    }

    #[test]
    fn test_compute_page_boundaries_first_page_size() {
        let pages = compute_page_boundaries(20, 10);
        assert_eq!(pages[0].start, 0);
        assert_eq!(pages[0].size(), 9); // viewport - 1
    }

    #[test]
    fn test_compute_page_boundaries_middle_page_size() {
        let pages = compute_page_boundaries(22, 10);
        assert!(pages.len() >= 2);
        if pages.len() >= 2 {
            assert_eq!(pages[1].size(), 8); // viewport - 2
        }
    }

    #[test]
    fn test_compute_page_boundaries_last_page_special() {
        // 22 items, viewport 10
        // Page 1: 0-8 (9 items)
        // Page 2: 9-16 (8 items)
        // Page 3: 17-21 (5 items, remaining)
        let pages = compute_page_boundaries(22, 10);
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].size(), 9); // First: viewport - 1
        assert_eq!(pages[1].size(), 8); // Middle: viewport - 2
        assert_eq!(pages[2].size(), 5); // Last: remaining
    }

    #[test]
    fn test_compute_page_boundaries_single_item() {
        let pages = compute_page_boundaries(1, 10);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].start, 0);
        assert_eq!(pages[0].end, 1);
    }

    #[test]
    fn test_compute_page_boundaries_exact_fit() {
        // 9 items, viewport 10 = fits exactly on first page
        let pages = compute_page_boundaries(9, 10);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].size(), 9);
    }

    #[test]
    fn test_compute_page_boundaries_boundary_values() {
        // Test the boundary where last page detection switches
        // With viewport 10, last_page_max = 9
        // 9 remaining items should trigger last page sizing
        let pages = compute_page_boundaries(18, 10);
        // Page 1: 0-8 (9)
        // Remaining: 9, which == 9, so IS last page
        assert_eq!(pages[1].size(), 9); // Last page sizing
    }

    // ============================================================================
    // count_headers_in_viewport() Tests
    // ============================================================================

    #[test]
    fn test_count_headers_empty_boundaries() {
        let boundaries = vec![];
        let count = count_headers_in_viewport(&boundaries, 0, 10);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_count_headers_single_boundary_fully_visible() {
        let boundaries = vec![column_boundary(0, 5)];
        let count = count_headers_in_viewport(&boundaries, 0, 10);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_count_headers_multiple_boundaries() {
        let boundaries = vec![
            column_boundary(0, 5),
            column_boundary(5, 5),
            column_boundary(10, 5),
        ];
        let count = count_headers_in_viewport(&boundaries, 0, 10);
        assert_eq!(count, 2); // First two columns visible
    }

    #[test]
    fn test_count_headers_partial_overlap() {
        let boundaries = vec![
            column_boundary(0, 5),
            column_boundary(5, 10), // Cards 5-14
        ];
        let count = count_headers_in_viewport(&boundaries, 3, 8);
        assert_eq!(count, 2); // Both columns have cards in range [3, 11)
    }

    #[test]
    fn test_count_headers_no_overlap() {
        let boundaries = vec![column_boundary(0, 5)];
        let count = count_headers_in_viewport(&boundaries, 10, 5);
        assert_eq!(count, 0);
    }

    // ============================================================================
    // compute_page_boundaries_with_headers() Tests
    // ============================================================================

    #[test]
    fn test_page_boundaries_with_headers_empty() {
        let pages = compute_page_boundaries_with_headers(0, 10, &[]);
        assert!(pages.is_empty());
    }

    #[test]
    fn test_page_boundaries_with_headers_no_headers() {
        let boundaries = vec![];
        let pages = compute_page_boundaries_with_headers(20, 10, &boundaries);
        assert!(!pages.is_empty());
        assert_eq!(pages[0].size(), 9); // Same as standard pagination
    }

    #[test]
    fn test_page_boundaries_with_headers_single_header() {
        // Single column with all cards
        let boundaries = vec![column_boundary(0, 20)];
        let pages = compute_page_boundaries_with_headers(20, 10, &boundaries);

        // First page: viewport(9) - header(1) = 8 cards
        assert_eq!(pages[0].size(), 8);
    }

    #[test]
    fn test_page_boundaries_with_headers_multiple_headers() {
        // Two columns
        let boundaries = vec![column_boundary(0, 10), column_boundary(10, 10)];
        let pages = compute_page_boundaries_with_headers(20, 10, &boundaries);

        // First page: viewport(9) - header(1) = 8 cards (cards 0-7, all in column 1)
        assert_eq!(pages[0].size(), 8);

        // Second page starts at card 8, might include column 2 header
        if pages.len() > 1 {
            // Middle page: viewport(8) - headers(?)
            assert!(pages[1].size() > 0);
        }
    }

    #[test]
    fn test_page_boundaries_with_headers_progress() {
        // Ensure we make progress (no infinite loop)
        let boundaries = vec![column_boundary(0, 100)];
        let pages = compute_page_boundaries_with_headers(100, 10, &boundaries);

        let total_cards: usize = pages.iter().map(|p| p.size()).sum();
        assert_eq!(total_cards, 100); // All cards accounted for
    }

    #[test]
    fn test_page_boundaries_with_headers_minimum_progress() {
        // With heavy headers, page size could drop to 1
        let boundaries: Vec<(usize, usize)> = (0..20).map(|i| column_boundary(i, 1)).collect();
        let pages = compute_page_boundaries_with_headers(20, 3, &boundaries);

        // Even with 20 headers in viewport 3, should make progress
        assert!(!pages.is_empty());
        for page in &pages {
            assert!(page.size() >= 1);
        }
    }

    // ============================================================================
    // Jump Calculation Tests
    // ============================================================================

    #[test]
    fn test_calculate_jump_down_top_to_middle() {
        // At position 0 (top) of a 10-card page, should jump to middle
        let pages = vec![PageBoundary { start: 0, end: 10 }];
        let target = calculate_jump_target_down(&pages, 0, 0);
        assert_eq!(target, 5); // Middle of page
    }

    #[test]
    fn test_calculate_jump_down_middle_to_bottom() {
        // At position 5 (middle) of a 10-card page, should jump to bottom
        let pages = vec![PageBoundary { start: 0, end: 10 }];
        let target = calculate_jump_target_down(&pages, 5, 0);
        assert_eq!(target, 9); // Bottom of page
    }

    #[test]
    fn test_calculate_jump_down_bottom_to_next_page() {
        // At position 9 (bottom) of page 0, should jump to top of page 1
        let pages = vec![
            PageBoundary { start: 0, end: 10 },
            PageBoundary { start: 10, end: 20 },
        ];
        let target = calculate_jump_target_down(&pages, 9, 0);
        assert_eq!(target, 10);
    }

    #[test]
    fn test_calculate_jump_down_at_last_page_bottom() {
        // At bottom of last page, stay there
        let pages = vec![PageBoundary { start: 0, end: 10 }];
        let target = calculate_jump_target_down(&pages, 9, 0);
        assert_eq!(target, 9);
    }

    #[test]
    fn test_calculate_jump_down_single_item_page() {
        // Page with only 1 item
        let pages = vec![PageBoundary { start: 0, end: 1 }];
        let target = calculate_jump_target_down(&pages, 0, 0);
        assert_eq!(target, 0);
    }

    #[test]
    fn test_calculate_jump_up_bottom_to_middle() {
        // At position 9 (bottom) of a 10-card page, should jump to middle
        let pages = vec![PageBoundary { start: 0, end: 10 }];
        let target = calculate_jump_target_up(&pages, 9, 0);
        assert_eq!(target, 5); // Middle
    }

    #[test]
    fn test_calculate_jump_up_middle_to_top() {
        // At position 5 (middle) of a 10-card page, should jump to top
        let pages = vec![PageBoundary { start: 0, end: 10 }];
        let target = calculate_jump_target_up(&pages, 5, 0);
        assert_eq!(target, 0); // Top
    }

    #[test]
    fn test_calculate_jump_up_top_to_previous_page() {
        // At position 0 (top) of page 1, should jump to bottom of page 0
        let pages = vec![
            PageBoundary { start: 0, end: 10 },
            PageBoundary { start: 10, end: 20 },
        ];
        let target = calculate_jump_target_up(&pages, 10, 1);
        assert_eq!(target, 9); // Bottom of page 0
    }

    #[test]
    fn test_calculate_jump_up_at_first_page_top() {
        // At top of first page, stay there
        let pages = vec![PageBoundary { start: 0, end: 10 }];
        let target = calculate_jump_target_up(&pages, 0, 0);
        assert_eq!(target, 0);
    }

    #[test]
    fn test_find_current_page() {
        let pages = vec![
            PageBoundary { start: 0, end: 9 },
            PageBoundary { start: 9, end: 17 },
            PageBoundary { start: 17, end: 22 },
        ];

        assert_eq!(find_current_page(&pages, 0), Some(0));
        assert_eq!(find_current_page(&pages, 8), Some(0));
        assert_eq!(find_current_page(&pages, 9), Some(1));
        assert_eq!(find_current_page(&pages, 16), Some(1));
        assert_eq!(find_current_page(&pages, 17), Some(2));
        assert_eq!(find_current_page(&pages, 21), Some(2));
        assert_eq!(find_current_page(&pages, 22), None);
    }

    // KAN-892: ArchivedBoardsView navigation bounded by archived list length

    fn seed_two_archived_boards(app: &mut crate::App) {
        use kanban_domain::KanbanOperations;
        let b1 = app
            .ctx
            .inner_mut()
            .create_board("Arch1".to_string(), None)
            .unwrap();
        let b2 = app
            .ctx
            .inner_mut()
            .create_board("Arch2".to_string(), None)
            .unwrap();
        app.ctx.inner_mut().archive_board(b1.id).unwrap();
        app.ctx.inner_mut().archive_board(b2.id).unwrap();
        let snap = app.ctx.snapshot().unwrap();
        app.model.load_from_snapshot(snap);
    }

    #[test]
    fn test_archived_view_j_moves_when_no_live_boards() {
        let mut app = crate::App::test_default();
        seed_two_archived_boards(&mut app);
        app.mode = AppMode::ArchivedBoardsView;
        app.focus.active = Focus::Boards;
        app.reload_model();
        app.prepare_frame();
        app.board_list.inner_mut().set_selected_index(Some(0));

        app.handle_navigation_down();

        assert_eq!(
            app.board_list.get_selected_index(),
            Some(1),
            "j must move to index 1 in archived view even with no live boards"
        );
    }

    #[test]
    fn test_archived_view_j_clamps_to_archived_len_not_live_len() {
        use kanban_domain::KanbanOperations;
        let mut app = crate::App::test_default();
        app.ctx
            .inner_mut()
            .create_board("Live".to_string(), None)
            .unwrap();
        let b1 = app
            .ctx
            .inner_mut()
            .create_board("Arch1".to_string(), None)
            .unwrap();
        let b2 = app
            .ctx
            .inner_mut()
            .create_board("Arch2".to_string(), None)
            .unwrap();
        let b3 = app
            .ctx
            .inner_mut()
            .create_board("Arch3".to_string(), None)
            .unwrap();
        app.ctx.inner_mut().archive_board(b1.id).unwrap();
        app.ctx.inner_mut().archive_board(b2.id).unwrap();
        app.ctx.inner_mut().archive_board(b3.id).unwrap();
        let snap = app.ctx.snapshot().unwrap();
        app.model.load_from_snapshot(snap);
        app.mode = AppMode::ArchivedBoardsView;
        app.focus.active = Focus::Boards;
        app.reload_model();
        app.prepare_frame();
        app.board_list.inner_mut().set_selected_index(Some(0));

        app.handle_navigation_down();
        app.handle_navigation_down();
        app.handle_navigation_down();

        assert_eq!(
            app.board_list.get_selected_index(),
            Some(2),
            "after 3 j presses, should clamp at archived_len-1=2, not live_len-1=0"
        );
    }

    #[test]
    fn test_archived_view_g_jumps_to_last_archived() {
        use kanban_domain::KanbanOperations;
        let mut app = crate::App::test_default();
        app.ctx
            .inner_mut()
            .create_board("Live".to_string(), None)
            .unwrap();
        let b1 = app
            .ctx
            .inner_mut()
            .create_board("Arch1".to_string(), None)
            .unwrap();
        let b2 = app
            .ctx
            .inner_mut()
            .create_board("Arch2".to_string(), None)
            .unwrap();
        let b3 = app
            .ctx
            .inner_mut()
            .create_board("Arch3".to_string(), None)
            .unwrap();
        app.ctx.inner_mut().archive_board(b1.id).unwrap();
        app.ctx.inner_mut().archive_board(b2.id).unwrap();
        app.ctx.inner_mut().archive_board(b3.id).unwrap();
        let snap = app.ctx.snapshot().unwrap();
        app.model.load_from_snapshot(snap);
        app.mode = AppMode::ArchivedBoardsView;
        app.focus.active = Focus::Boards;
        app.reload_model();
        app.prepare_frame();
        app.board_list.inner_mut().set_selected_index(Some(0));

        app.handle_jump_to_bottom();

        assert_eq!(
            app.board_list.get_selected_index(),
            Some(2),
            "G should jump to archived_len-1=2"
        );
    }

    #[test]
    fn test_archived_view_navigation_does_not_switch_view_strategy() {
        use crate::view_strategy::UnifiedViewStrategy;
        use kanban_view::layout_strategy::SingleListLayout;
        let mut app = crate::App::test_default();
        seed_two_archived_boards(&mut app);
        app.mode = AppMode::ArchivedBoardsView;
        app.focus.active = Focus::Boards;
        app.board_list.inner_mut().set_selected_index(Some(0));

        app.switch_view_strategy(kanban_domain::TaskListView::Flat);

        app.handle_navigation_down();

        let is_flat = app
            .view
            .strategy
            .as_any()
            .downcast_ref::<UnifiedViewStrategy>()
            .map(|s| {
                s.get_layout_strategy()
                    .as_any()
                    .downcast_ref::<SingleListLayout>()
                    .is_some()
            })
            .unwrap_or(false);
        assert!(
            is_flat,
            "navigation in ArchivedBoardsView must not switch view strategy"
        );
    }

    #[test]
    fn test_live_view_navigation_still_bounded_by_live_count() {
        use kanban_domain::KanbanOperations;
        let mut app = crate::App::test_default();
        app.ctx
            .inner_mut()
            .create_board("Live1".to_string(), None)
            .unwrap();
        app.ctx
            .inner_mut()
            .create_board("Live2".to_string(), None)
            .unwrap();
        for i in 0..5 {
            let b = app
                .ctx
                .inner_mut()
                .create_board(format!("Arch{i}"), None)
                .unwrap();
            app.ctx.inner_mut().archive_board(b.id).unwrap();
        }
        let snap = app.ctx.snapshot().unwrap();
        app.model.load_from_snapshot(snap);
        app.mode = AppMode::Normal;
        app.focus.active = Focus::Boards;
        app.reload_model();
        app.prepare_frame();
        app.board_list.inner_mut().set_selected_index(Some(0));

        for _ in 0..10 {
            app.handle_navigation_down();
        }

        assert_eq!(
            app.board_list.get_selected_index(),
            Some(1),
            "live view navigation must clamp at live_count-1=1"
        );
    }

    // KAN-1090: Ctrl+D/Ctrl+U half-viewport jumps must work on the boards
    // panel, matching the card list's existing behavior.

    fn seed_boards_for_half_viewport_jump(app: &mut crate::App, count: usize) {
        use kanban_domain::KanbanOperations;
        for i in 0..count {
            app.ctx.create_board(format!("Board{i}"), None).unwrap();
        }
        app.reload_model();
        app.prepare_frame();
        app.mode = AppMode::Normal;
        app.focus.active = Focus::Boards;
        // Single-page pagination (6 items comfortably fit under a viewport of
        // 10), so calculate_jump_target_down/_up's three-level jump (top ->
        // middle -> bottom) resolves deterministically to page_size / 2 = 3.
        app.view.viewport_height = 10;
    }

    #[test]
    fn test_handle_jump_half_viewport_down_on_boards_focus_jumps_a_half_page() {
        let mut app = crate::App::test_default();
        seed_boards_for_half_viewport_jump(&mut app, 6);
        app.board_list.inner_mut().set_selected_index(Some(0));

        app.handle_jump_half_viewport_down();

        assert_eq!(
            app.board_list.get_selected_index(),
            Some(3),
            "Ctrl+D from the top of a 6-board single page should jump to the \
             middle (page_size / 2 = 3), not move by one like j"
        );
    }

    #[test]
    fn test_handle_jump_half_viewport_up_on_boards_focus_jumps_a_half_page() {
        let mut app = crate::App::test_default();
        seed_boards_for_half_viewport_jump(&mut app, 6);
        app.board_list.inner_mut().set_selected_index(Some(5));

        app.handle_jump_half_viewport_up();

        assert_eq!(
            app.board_list.get_selected_index(),
            Some(3),
            "Ctrl+U from the bottom of a 6-board single page should jump to \
             the middle (page_size / 2 = 3), not move by one like k"
        );
    }

    // KAN-893: is_kanban_view must return false in ArchivedBoardsView

    fn make_app_with_columnview_active_board() -> crate::App {
        use kanban_domain::{BoardUpdate, KanbanOperations};
        let mut app = crate::App::test_default();
        let board = app
            .ctx
            .inner_mut()
            .create_board("ColView".to_string(), None)
            .unwrap();
        app.ctx
            .inner_mut()
            .update_board(
                board.id,
                BoardUpdate {
                    task_list_view: Some(kanban_domain::TaskListView::ColumnView),
                    ..Default::default()
                },
            )
            .unwrap();
        let snap = app.ctx.snapshot().unwrap();
        app.model.load_from_snapshot(snap);
        app.selection.active_board_id = app.model.boards().first().map(|b| b.id);
        app
    }

    #[test]
    fn test_is_kanban_view_false_while_browsing_boards_list() {
        // KAN-893: browsing a boards list (no active board) always uses the split
        // projects+tasks layout so the list stays visible — even when a live
        // ColumnView board exists in the model. Holds for the archived set too,
        // with no archival-specific branch: toggling to it clears the active
        // board, so `is_kanban_view` is false while browsing.
        let mut app = make_app_with_columnview_active_board();
        app.focus.active = Focus::Boards;
        app.handle_toggle_archived_boards_view();
        assert_eq!(app.mode, AppMode::ArchivedBoardsView);
        assert_eq!(
            app.selection.active_board_id, None,
            "no board active in list"
        );
        assert!(
            !app.is_kanban_view(),
            "browsing the boards list must never be treated as kanban view"
        );
    }

    #[test]
    fn test_is_kanban_view_true_for_columnview_active_board_in_normal_mode() {
        let app = make_app_with_columnview_active_board();
        assert_eq!(app.mode, AppMode::Normal);
        assert!(
            app.is_kanban_view(),
            "Normal mode with ColumnView active board must be kanban view"
        );
    }
}
