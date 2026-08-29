use super::Controller;
use kanban_core::AppConfig;
use kanban_domain::{BoardSortField, SortOrder};

impl Controller {
    pub fn board_sort(&self, _archived: bool) -> (BoardSortField, SortOrder) {
        todo!()
    }

    pub fn set_board_sort(&mut self, _archived: bool, _field: BoardSortField, _order: SortOrder) {
        todo!()
    }

    pub fn toggle_board_sort_order(&mut self, _archived: bool) {
        todo!()
    }

    pub fn set_board_sort_from_config(&mut self, _config: &AppConfig) {
        todo!()
    }
}
