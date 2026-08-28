use crate::list_component::{ListComponent, ListRenderInfo};
use uuid::Uuid;

pub struct BoardList {
    boards: Vec<Uuid>,
    list: ListComponent,
}

impl Default for BoardList {
    fn default() -> Self {
        Self::new()
    }
}

impl BoardList {
    pub fn new() -> Self {
        Self {
            boards: Vec::new(),
            list: ListComponent::new(false),
        }
    }

    pub fn update_boards(&mut self, boards: Vec<Uuid>) {
        let current = self.get_selected_board_id();
        self.boards = boards;
        self.list.update_item_count(self.boards.len());

        if let Some(id) = current {
            if !self.select_board(id) {
                self.list
                    .set_selected_index((!self.boards.is_empty()).then_some(0));
            }
        }
    }

    pub fn get_selected_board_id(&self) -> Option<Uuid> {
        self.list
            .get_selected_index()
            .and_then(|i| self.boards.get(i).copied())
    }

    pub fn select_board(&mut self, id: Uuid) -> bool {
        if let Some(idx) = self.boards.iter().position(|&b| b == id) {
            self.list.set_selected_index(Some(idx));
            true
        } else {
            false
        }
    }

    pub fn navigate_up(&mut self) -> bool {
        self.list.navigate_up()
    }

    pub fn navigate_down(&mut self) -> bool {
        self.list.navigate_down()
    }

    pub fn jump_to_top(&mut self) {
        self.list.jump_to(0);
    }

    pub fn jump_to_bottom(&mut self) {
        if !self.boards.is_empty() {
            self.list.jump_to(self.boards.len() - 1);
        }
    }

    pub fn jump_to(&mut self, index: usize) {
        self.list.jump_to(index);
    }

    /// The board ids currently held, in display order — reflects whatever
    /// subset `update_boards` was last called with (e.g. narrowed by an
    /// active search filter).
    pub fn ids(&self) -> &[Uuid] {
        &self.boards
    }

    pub fn len(&self) -> usize {
        self.boards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.boards.is_empty()
    }

    pub fn get_selected_index(&self) -> Option<usize> {
        self.list.get_selected_index()
    }

    pub fn get_render_info(&self, viewport_height: usize) -> ListRenderInfo {
        self.list.get_render_info(viewport_height)
    }

    /// Escape hatch onto the underlying `ListComponent` for callers that need
    /// direct access (e.g. test setup seeding a raw selected index) without
    /// widening this wrapper's own API for every `ListComponent` method.
    pub fn inner(&self) -> &ListComponent {
        &self.list
    }

    pub fn inner_mut(&mut self) -> &mut ListComponent {
        &mut self.list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(count: usize) -> Vec<Uuid> {
        (0..count).map(|_| Uuid::new_v4()).collect()
    }

    #[test]
    fn test_board_list_ids_reflects_last_update_boards_call() {
        let boards = ids(3);
        let mut list = BoardList::new();
        list.update_boards(boards.clone());
        assert_eq!(list.ids(), boards.as_slice());

        let narrowed = vec![boards[1]];
        list.update_boards(narrowed.clone());
        assert_eq!(list.ids(), narrowed.as_slice());
    }

    #[test]
    fn test_board_list_update_boards_preserves_selection_by_id_when_still_present() {
        let boards = ids(3);
        let mut list = BoardList::new();
        list.update_boards(boards.clone());
        list.select_board(boards[2]);
        assert_eq!(list.get_selected_board_id(), Some(boards[2]));

        // Re-sync with the same board present at a different index (moved to
        // front).
        let reordered = vec![boards[2], boards[0], boards[1]];
        list.update_boards(reordered);

        assert_eq!(list.get_selected_board_id(), Some(boards[2]));
        assert_eq!(list.get_selected_index(), Some(0));
    }

    #[test]
    fn test_board_list_update_boards_clamps_to_first_when_selected_board_removed() {
        let boards = ids(3);
        let mut list = BoardList::new();
        list.update_boards(boards.clone());
        list.select_board(boards[1]);

        // The selected board (e.g. archived) is gone from the new set.
        let remaining = vec![boards[0], boards[2]];
        list.update_boards(remaining.clone());

        assert_eq!(list.get_selected_board_id(), Some(remaining[0]));
        assert_eq!(list.get_selected_index(), Some(0));
    }

    #[test]
    fn test_board_list_update_boards_clamps_to_none_when_all_boards_removed() {
        let boards = ids(2);
        let mut list = BoardList::new();
        list.update_boards(boards.clone());
        list.select_board(boards[0]);

        list.update_boards(Vec::new());

        assert_eq!(list.get_selected_board_id(), None);
        assert_eq!(list.get_selected_index(), None);
    }

    #[test]
    fn test_board_list_select_board_returns_false_for_unknown_id() {
        let boards = ids(2);
        let mut list = BoardList::new();
        list.update_boards(boards);

        assert!(!list.select_board(Uuid::new_v4()));
    }

    #[test]
    fn test_board_list_navigate_up_down_matches_list_component_bounds() {
        let boards = ids(3);
        let mut list = BoardList::new();
        list.update_boards(boards.clone());

        assert_eq!(list.get_selected_index(), Some(0));
        let was_at_top = list.navigate_up();
        assert!(was_at_top, "navigate_up at index 0 reports at-top");
        assert_eq!(list.get_selected_index(), Some(0));

        list.navigate_down();
        list.navigate_down();
        assert_eq!(list.get_selected_index(), Some(2));
        let was_at_bottom = list.navigate_down();
        assert!(
            was_at_bottom,
            "navigate_down at last index reports at-bottom"
        );
        assert_eq!(list.get_selected_index(), Some(2));
    }

    #[test]
    fn test_board_list_jump_to_top_and_bottom() {
        let boards = ids(4);
        let mut list = BoardList::new();
        list.update_boards(boards);
        list.jump_to(2);
        assert_eq!(list.get_selected_index(), Some(2));

        list.jump_to_top();
        assert_eq!(list.get_selected_index(), Some(0));

        list.jump_to_bottom();
        assert_eq!(list.get_selected_index(), Some(3));
    }

    #[test]
    fn test_board_list_get_render_info_reports_scroll_indicators() {
        let boards = ids(10);
        let mut list = BoardList::new();
        list.update_boards(boards);
        list.jump_to(9);
        list.inner_mut().ensure_selected_visible(3);

        let info = list.get_render_info(3);
        assert!(info.show_above_indicator);
        assert!(!info.show_below_indicator);
        assert!(info.visible_indices.contains(&9));
    }
}
