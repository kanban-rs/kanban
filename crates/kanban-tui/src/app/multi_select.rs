use std::collections::HashSet;
use uuid::Uuid;

#[derive(Default)]
pub struct MultiSelectState {
    pub selected_cards: HashSet<Uuid>,
    pub selection_mode_active: bool,
    pub selected_boards: HashSet<Uuid>,
    pub board_selection_mode_active: bool,
}
