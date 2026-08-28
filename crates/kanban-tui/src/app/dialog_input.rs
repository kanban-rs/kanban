use crate::components::sprint_picker::{SprintFilter, SprintPicker};
use crate::handlers::board_handlers::BoardDeleteCounts;
use kanban_core::{InputState, SelectionState};
use kanban_view::ListComponent;
use uuid::Uuid;

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CreateCardFocus {
    #[default]
    Title,
    Column,
    Sprint,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CreateColumnFocus {
    #[default]
    Name,
    Status,
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
    pub create_column_focus: CreateColumnFocus,
    pub create_card_column_input: InputState,
    create_card_column_editable: bool,
    create_card_sprint_visible: bool,
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
            create_column_focus: CreateColumnFocus::default(),
            create_card_column_input: InputState::default(),
            create_card_column_editable: false,
            create_card_sprint_visible: true,
            assign_sprint_picker: SprintPicker::with_filter(SprintFilter::All),
            board_delete_counts: None,
        }
    }
}

impl DialogInputState {
    pub fn create_card_focus_is_title(&self) -> bool {
        self.create_card_focus == CreateCardFocus::Title
    }

    pub fn create_card_focus_is_column(&self) -> bool {
        self.create_card_focus == CreateCardFocus::Column
    }

    pub fn create_card_focus_is_sprint(&self) -> bool {
        self.create_card_focus == CreateCardFocus::Sprint
    }

    pub fn create_card_column_is_editable(&self) -> bool {
        self.create_card_column_editable
    }

    pub fn create_card_sprint_is_visible(&self) -> bool {
        self.create_card_sprint_visible
    }

    pub fn advance_create_card_focus(&mut self) {
        self.create_card_focus = match self.create_card_focus {
            CreateCardFocus::Title if self.create_card_column_editable => CreateCardFocus::Column,
            CreateCardFocus::Title | CreateCardFocus::Column if self.create_card_sprint_visible => {
                CreateCardFocus::Sprint
            }
            _ => CreateCardFocus::Title,
        };
    }

    pub fn create_card_focus_is_last_visible(&self) -> bool {
        match self.create_card_focus {
            CreateCardFocus::Sprint => true,
            CreateCardFocus::Column => !self.create_card_sprint_visible,
            CreateCardFocus::Title => {
                !self.create_card_column_editable && !self.create_card_sprint_visible
            }
        }
    }

    pub fn reset_create_card_focus(&mut self) {
        self.create_card_focus = CreateCardFocus::Title;
    }

    pub fn create_column_focus_is_name(&self) -> bool {
        self.create_column_focus == CreateColumnFocus::Name
    }

    pub fn toggle_create_column_focus(&mut self) {
        self.create_column_focus = match self.create_column_focus {
            CreateColumnFocus::Name => CreateColumnFocus::Status,
            CreateColumnFocus::Status => CreateColumnFocus::Name,
        };
    }

    pub fn reset_create_column_focus(&mut self) {
        self.create_column_focus = CreateColumnFocus::Name;
    }

    pub fn prime_create_card_column_field(&mut self, name: String, editable: bool) {
        self.create_card_column_editable = editable;
        self.create_card_column_input.set(name);
    }

    pub fn prime_create_card_sprint_field(&mut self, visible: bool) {
        self.create_card_sprint_visible = visible;
    }
}
