use kanban_core::SelectionState;
use std::cell::Cell;

#[derive(Default)]
pub struct SelectionHub {
    /// The board the user is currently viewing/acting on, tracked by IDENTITY so
    /// it is archival-agnostic: it resolves through `Model::board_by_id`, which
    /// finds the board whether its head is live or archived. `None` while
    /// browsing the projects list without a board opened. An archived board is
    /// substitutable for a live one everywhere — there is no separate archived
    /// active-board state (Liskov).
    pub active_board_id: Option<uuid::Uuid>,
    pub active_card_id: Option<uuid::Uuid>,
    pub sprint: SelectionState,
    pub sprint_scroll: Cell<usize>,
    /// The sprint whose detail view is open, tracked by IDENTITY: it resolves
    /// through `Model::sprints().iter().find(...)` rather than indexing, since
    /// a position into that collection is only meaningful against the exact
    /// collection instance it came from — the list is ordered across every
    /// board, so deleting any sprint ahead of this one shifts its slot.
    pub active_sprint_id: Option<uuid::Uuid>,
    pub card_navigation_history: Vec<uuid::Uuid>,
    pub settings_config: SelectionState,
    pub settings_config_file: SelectionState,
    pub settings_storage: SelectionState,
}
