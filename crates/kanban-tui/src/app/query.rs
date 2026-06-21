use super::App;

impl App {
    pub fn get_current_priority_selection_index(&self) -> usize {
        if let Some(active_id) = self.selection.active_card_id {
            if let Some(card) = self.model.card(active_id) {
                use kanban_domain::CardPriority;
                return match card.priority {
                    CardPriority::Low => 0,
                    CardPriority::Medium => 1,
                    CardPriority::High => 2,
                    CardPriority::Critical => 3,
                };
            }
        }
        0
    }

    pub fn get_current_sprint_selection_index(&self) -> usize {
        use crate::components::sprint_assign_list::{build_entries, sprint_id_of};

        if let Some(active_id) = self.selection.active_card_id {
            if let Some(card) = self.model.card(active_id) {
                if let Some(card_sprint_id) = card.sprint_id {
                    if let Some(board_idx) = self.selection.active_board_index {
                        let boards = self.model.boards();
                        if let Some(board) = boards.get(board_idx) {
                            let sprints = self.model.sprints();
                            let entries = build_entries(sprints, board.id, chrono::Utc::now());
                            for (idx, entry) in entries.iter().enumerate() {
                                if sprint_id_of(entry) == Some(card_sprint_id) {
                                    return idx;
                                }
                            }
                        }
                    }
                }
            }
        }
        0
    }

    pub fn get_current_sort_field_selection_index(&self) -> usize {
        self.filter
            .current_sort_field
            .map(crate::components::selection_dialog::popup_index_of_sort_field)
            .unwrap_or(0)
    }
}
