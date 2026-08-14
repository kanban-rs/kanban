use crate::components::sprint_picker::{SprintFilter, SprintPicker};
use crate::handlers::board_handlers::BoardDeleteCounts;
use kanban_core::SelectionState;
use kanban_view::ListComponent;
use uuid::Uuid;

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CreateCardFocus {
    #[default]
    Title,
    Sprint,
}

pub struct DialogInputState {
    pub import_files: Vec<String>,
    pub import_selection: SelectionState,
    pub priority_selection: SelectionState,
    pub default_status_selection: SelectionState,
    pub column_list: ListComponent,
    pub sprint_assign_selection: SelectionState,
    pub task_list_view_selection: SelectionState,
    pub carry_over_sprint_selection: SelectionState,
    pub carry_over_source_sprint_id: Option<Uuid>,
    pub create_card_sprint_picker: SprintPicker,
    pub create_card_focus: CreateCardFocus,
    /// Picker for the assign-to-existing-card dialogs (single and
    /// bulk). Configured with SprintFilter::All so the user can pick
    /// from completed/ended sprints as well, which the create-card
    /// picker intentionally hides.
    pub assign_sprint_picker: SprintPicker,
    /// Entity counts for the currently-open delete-board confirmation,
    /// snapshotted when the dialog opens (see `handle_delete_board_key`) so
    /// the modal never re-scans the model per frame.
    pub(crate) board_delete_counts: Option<BoardDeleteCounts>,
}

impl Default for DialogInputState {
    fn default() -> Self {
        Self {
            import_files: Vec::new(),
            import_selection: SelectionState::default(),
            priority_selection: SelectionState::default(),
            default_status_selection: SelectionState::default(),
            column_list: ListComponent::new(false),
            sprint_assign_selection: SelectionState::default(),
            task_list_view_selection: SelectionState::default(),
            carry_over_sprint_selection: SelectionState::default(),
            carry_over_source_sprint_id: None,
            create_card_sprint_picker: SprintPicker::with_filter(SprintFilter::ActiveOnly),
            create_card_focus: CreateCardFocus::default(),
            assign_sprint_picker: SprintPicker::with_filter(SprintFilter::All),
            board_delete_counts: None,
        }
    }
}

impl DialogInputState {
    pub fn create_card_focus_is_title(&self) -> bool {
        self.create_card_focus == CreateCardFocus::Title
    }

    pub fn toggle_create_card_focus(&mut self) {
        self.create_card_focus = match self.create_card_focus {
            CreateCardFocus::Title => CreateCardFocus::Sprint,
            CreateCardFocus::Sprint => CreateCardFocus::Title,
        };
    }

    pub fn reset_create_card_focus(&mut self) {
        self.create_card_focus = CreateCardFocus::Title;
    }
}
