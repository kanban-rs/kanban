use crate::app::{
    App, AppMode, BoardField, BoardFocus, CardField, CardFocus, DialogMode, SprintTaskPanel,
};
use crate::editor::edit_in_external_editor;
use crate::events::EventHandler;
use crossterm::event::KeyCode;
use kanban_core::Editable;
use kanban_domain::{BoardSettingsDto, CardMetadataDto};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

// Viewport constants (must match ui.rs values)
const RELATIONSHIP_VIEWPORT_RAW: usize = 5;

#[derive(Clone, Copy)]
pub(crate) enum RelationSide {
    Parents,
    Children,
}

impl App {
    pub fn handle_card_detail_key(
        &mut self,
        key_code: KeyCode,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
    ) -> bool {
        let mut should_restart = false;
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
                self.selection.active_card_id = None;
                self.focus.card_focus = CardFocus::Title;
                self.relationship.parents_list.selection.clear();
                self.relationship.children_list.selection.clear();
                self.selection.card_navigation_history.clear();
            }
            KeyCode::Char('1') => {
                self.focus.card_focus = CardFocus::Title;
            }
            KeyCode::Char('2') => {
                self.focus.card_focus = CardFocus::Metadata;
            }
            KeyCode::Char('3') => {
                self.focus.card_focus = CardFocus::Description;
            }
            KeyCode::Char('4') => {
                self.focus.card_focus = CardFocus::Parents;
            }
            KeyCode::Char('5') => {
                self.focus.card_focus = CardFocus::Children;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                match self.focus.card_focus {
                    CardFocus::Parents => {
                        // Navigate within parents list or wrap to next section
                        let parents = self.get_current_card_parents();
                        if !parents.is_empty() {
                            let was_at_boundary = self.relationship.parents_list.navigate_down();
                            let viewport = self
                                .relationship
                                .parents_list
                                .get_adjusted_viewport_height(RELATIONSHIP_VIEWPORT_RAW);
                            self.relationship
                                .parents_list
                                .ensure_selected_visible(viewport);

                            if was_at_boundary {
                                // At last parent, wrap to Children section
                                self.focus.card_focus = CardFocus::Children;
                                self.relationship.parents_list.selection.clear();

                                let children = self.get_current_card_children();
                                self.relationship
                                    .children_list
                                    .update_item_count(children.len());
                                if !children.is_empty() {
                                    self.relationship.children_list.selection.jump_to_first();
                                }
                            }
                        } else {
                            // No parents, move to Children section
                            self.focus.card_focus = CardFocus::Children;
                        }
                    }
                    CardFocus::Children => {
                        // Navigate within children list or wrap to next section
                        let children = self.get_current_card_children();
                        if !children.is_empty() {
                            let was_at_boundary = self.relationship.children_list.navigate_down();
                            let viewport = self
                                .relationship
                                .children_list
                                .get_adjusted_viewport_height(RELATIONSHIP_VIEWPORT_RAW);
                            self.relationship
                                .children_list
                                .ensure_selected_visible(viewport);

                            if was_at_boundary {
                                // At last child, wrap to Title section
                                self.focus.card_focus = CardFocus::Title;
                                self.relationship.children_list.selection.clear();
                            }
                        } else {
                            // No children, move to Title section
                            self.focus.card_focus = CardFocus::Title;
                        }
                    }
                    _ => {
                        // Navigate between sections
                        self.focus.card_focus = match self.focus.card_focus {
                            CardFocus::Title => CardFocus::Metadata,
                            CardFocus::Metadata => CardFocus::Description,
                            CardFocus::Description => CardFocus::Parents,
                            CardFocus::Parents => CardFocus::Children,
                            CardFocus::Children => CardFocus::Title,
                        };
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                match self.focus.card_focus {
                    CardFocus::Parents => {
                        // Navigate within parents list or wrap to previous section
                        let parents = self.get_current_card_parents();
                        if !parents.is_empty() {
                            let was_at_boundary = self.relationship.parents_list.navigate_up();
                            let viewport = self
                                .relationship
                                .parents_list
                                .get_adjusted_viewport_height(RELATIONSHIP_VIEWPORT_RAW);
                            self.relationship
                                .parents_list
                                .ensure_selected_visible(viewport);

                            if was_at_boundary {
                                // At first parent or no selection, wrap to Description section
                                self.focus.card_focus = CardFocus::Description;
                                self.relationship.parents_list.selection.clear();
                            }
                        } else {
                            // No parents, move to Description section
                            self.focus.card_focus = CardFocus::Description;
                        }
                    }
                    CardFocus::Children => {
                        // Navigate within children list or wrap to previous section
                        let children = self.get_current_card_children();
                        if !children.is_empty() {
                            let was_at_boundary = self.relationship.children_list.navigate_up();
                            let viewport = self
                                .relationship
                                .children_list
                                .get_adjusted_viewport_height(RELATIONSHIP_VIEWPORT_RAW);
                            self.relationship
                                .children_list
                                .ensure_selected_visible(viewport);

                            if was_at_boundary {
                                // At first child or no selection, wrap to Parents section
                                let parents = self.get_current_card_parents();
                                self.focus.card_focus = CardFocus::Parents;
                                self.relationship.children_list.selection.clear();
                                self.relationship
                                    .parents_list
                                    .update_item_count(parents.len());
                                if !parents.is_empty() {
                                    self.relationship
                                        .parents_list
                                        .selection
                                        .jump_to_last(parents.len());
                                }
                            }
                        } else {
                            // No children, move to Parents section
                            self.focus.card_focus = CardFocus::Parents;
                        }
                    }
                    CardFocus::Title => {
                        // When at Title, wrap backward to Children and select last child
                        let children = self.get_current_card_children();
                        self.focus.card_focus = CardFocus::Children;
                        self.relationship
                            .children_list
                            .update_item_count(children.len());
                        if !children.is_empty() {
                            self.relationship
                                .children_list
                                .selection
                                .jump_to_last(children.len());
                            let viewport = self
                                .relationship
                                .children_list
                                .get_adjusted_viewport_height(RELATIONSHIP_VIEWPORT_RAW);
                            self.relationship
                                .children_list
                                .ensure_selected_visible(viewport);
                        }
                    }
                    _ => {
                        // Navigate between remaining sections (Metadata, Description)
                        self.focus.card_focus = match self.focus.card_focus {
                            CardFocus::Description => CardFocus::Metadata,
                            CardFocus::Metadata => CardFocus::Title,
                            // Other cases won't reach here due to explicit handling above
                            _ => CardFocus::Title,
                        };
                    }
                }
            }
            KeyCode::Char('y') => {
                self.copy_branch_name();
            }
            KeyCode::Char('Y') => {
                self.copy_git_checkout_command();
            }
            KeyCode::Char('e') => match self.focus.card_focus {
                CardFocus::Title => {
                    if let Err(e) = self.edit_card_field(terminal, event_handler, CardField::Title)
                    {
                        tracing::error!("Failed to edit title: {}", e);
                        self.set_error(format!("Failed to edit title: {}", e));
                    }
                    should_restart = true;
                }
                CardFocus::Description => {
                    if let Err(e) =
                        self.edit_card_field(terminal, event_handler, CardField::Description)
                    {
                        tracing::error!("Failed to edit description: {}", e);
                        self.set_error(format!("Failed to edit description: {}", e));
                    }
                    should_restart = true;
                }
                CardFocus::Metadata => {
                    if let Some(card) = self.get_card_for_detail_view() {
                        let card_id = card.id;
                        let dto = CardMetadataDto::from_entity(&card);
                        let json =
                            serde_json::to_string_pretty(&dto).unwrap_or_else(|_| "{}".to_string());
                        let temp_file = std::env::temp_dir()
                            .join(format!("kanban-card-{}-metadata.json", card_id));
                        match edit_in_external_editor(terminal, event_handler, temp_file, &json) {
                            Ok(Some(new_content)) => {
                                match serde_json::from_str::<CardMetadataDto>(&new_content) {
                                    Ok(new_dto) => {
                                        let cmd = kanban_domain::commands::Command::Card(
                                            kanban_domain::commands::CardCommand::ApplyMetadata(
                                                kanban_domain::commands::ApplyCardMetadata {
                                                    card_id,
                                                    dto: new_dto,
                                                },
                                            ),
                                        );
                                        if let Err(e) = self.ctx.execute_command(cmd) {
                                            tracing::error!("Failed to apply metadata: {}", e);
                                            self.set_error(format!(
                                                "Failed to apply metadata: {}",
                                                e
                                            ));
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to parse metadata JSON: {}", e);
                                        self.set_error(format!(
                                            "Failed to parse metadata JSON: {}",
                                            e
                                        ));
                                    }
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                tracing::error!("Failed to edit metadata: {}", e);
                                self.set_error(format!("Failed to edit metadata: {}", e));
                            }
                        }
                        should_restart = true;
                    }
                }
                CardFocus::Parents => {
                    // Parents section - use 'r' to manage parents
                }
                CardFocus::Children => {
                    // Children section - use 'R' to manage children
                }
            },
            KeyCode::Char('d') => {
                self.handle_archive_card();
                self.pop_mode();
                self.selection.active_card_id = None;
                self.focus.card_focus = CardFocus::Title;
            }
            KeyCode::Char('a') => {
                if let Some(board) = self.active_board().cloned() {
                    let sprint_count = self
                        .model
                        .sprints()
                        .iter()
                        .filter(|s| s.board_id == board.id)
                        .count();
                    if sprint_count > 0 {
                        let current_sprint_id = self
                            .selection
                            .active_card_id
                            .and_then(|id| self.model.card_by_id(id))
                            .and_then(|c| c.sprint_id);
                        self.dialog_input
                            .assign_sprint_picker
                            .reset_for_card_assignment(
                                current_sprint_id,
                                self.model.sprints(),
                                &board,
                                chrono::Utc::now(),
                            );
                        self.open_dialog(DialogMode::AssignCardToSprint);
                    }
                }
            }
            KeyCode::Char('p') => {
                self.open_dialog(DialogMode::SetCardPoints);
            }
            KeyCode::Char('P') => {
                let priority_idx = self.get_current_priority_selection_index();
                self.dialog_input.priority_selection.set(Some(priority_idx));
                self.open_dialog(DialogMode::SetCardPriority);
            }
            KeyCode::Char('r') => {
                self.handle_manage_parents();
            }
            KeyCode::Char('R') => {
                self.handle_manage_children();
            }
            KeyCode::Enter => match self.focus.card_focus {
                CardFocus::Parents => self.navigate_to_selected_parent(),
                CardFocus::Children => self.navigate_to_selected_child(),
                _ => {}
            },
            KeyCode::Backspace | KeyCode::Char('h')
                if self.focus.card_focus != CardFocus::Title
                    && self.focus.card_focus != CardFocus::Metadata
                    && self.focus.card_focus != CardFocus::Description =>
            {
                self.return_to_previous_card_from_detail_history();
            }
            _ => {}
        }
        should_restart
    }

    pub fn handle_board_detail_key(
        &mut self,
        key_code: KeyCode,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
    ) -> bool {
        let mut should_restart = false;
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
                self.focus.board_focus = BoardFocus::Name;
            }
            KeyCode::Char('1') => {
                self.focus.board_focus = BoardFocus::Name;
            }
            KeyCode::Char('2') => {
                self.focus.board_focus = BoardFocus::Description;
            }
            KeyCode::Char('3') => {
                self.focus.board_focus = BoardFocus::Settings;
            }
            KeyCode::Char('4') => {
                self.focus.board_focus = BoardFocus::Sprints;
            }
            KeyCode::Char('5') => {
                self.focus.board_focus = BoardFocus::Columns;
            }
            KeyCode::Char('e') => match self.focus.board_focus {
                BoardFocus::Name => {
                    if let Err(e) = self.edit_board_field(terminal, event_handler, BoardField::Name)
                    {
                        tracing::error!("Failed to edit board name: {}", e);
                        self.set_error(format!("Failed to edit board name: {}", e));
                    }
                    should_restart = true;
                }
                BoardFocus::Description => {
                    if let Err(e) =
                        self.edit_board_field(terminal, event_handler, BoardField::Description)
                    {
                        tracing::error!("Failed to edit board description: {}", e);
                        self.set_error(format!("Failed to edit board description: {}", e));
                    }
                    should_restart = true;
                }
                BoardFocus::Settings => {
                    if let Some(board) = self.active_board() {
                        {
                            let board_id = board.id;
                            let dto = BoardSettingsDto::from_entity(board);
                            let json = serde_json::to_string_pretty(&dto)
                                .unwrap_or_else(|_| "{}".to_string());
                            let temp_file = std::env::temp_dir()
                                .join(format!("kanban-board-{}-settings.json", board_id));
                            match edit_in_external_editor(terminal, event_handler, temp_file, &json)
                            {
                                Ok(Some(new_content)) => {
                                    match serde_json::from_str::<BoardSettingsDto>(&new_content) {
                                        Ok(new_dto) => {
                                            let cmd = kanban_domain::commands::Command::Board(
                                                kanban_domain::commands::BoardCommand::ApplySettings(kanban_domain::commands::ApplyBoardSettings {
                                                    board_id,
                                                    dto: new_dto,
                                                }),
                                            );
                                            if let Err(e) = self.ctx.execute_command(cmd) {
                                                tracing::error!(
                                                    "Failed to apply board settings: {}",
                                                    e
                                                );
                                                self.set_error(format!(
                                                    "Failed to apply board settings: {}",
                                                    e
                                                ));
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                "Failed to parse board settings JSON: {}",
                                                e
                                            );
                                            self.set_error(format!(
                                                "Failed to parse board settings JSON: {}",
                                                e
                                            ));
                                        }
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    tracing::error!("Failed to edit board settings: {}", e);
                                    self.set_error(format!("Failed to edit board settings: {}", e));
                                }
                            }
                            should_restart = true;
                        }
                    }
                }
                BoardFocus::Sprints => {}
                BoardFocus::Columns => {}
            },
            KeyCode::Char('n') => {
                if self.focus.board_focus == BoardFocus::Sprints {
                    self.handle_create_sprint_key();
                } else if self.focus.board_focus == BoardFocus::Columns {
                    self.handle_create_column_key();
                }
            }
            KeyCode::Char('r') => {
                if self.focus.board_focus == BoardFocus::Columns {
                    self.handle_rename_column_key();
                }
            }
            KeyCode::Char('d') => {
                if self.focus.board_focus == BoardFocus::Columns {
                    self.handle_delete_column_key();
                }
            }
            KeyCode::Char('J') => {
                if self.focus.board_focus == BoardFocus::Columns {
                    self.handle_move_column_down();
                }
            }
            KeyCode::Char('K') => {
                if self.focus.board_focus == BoardFocus::Columns {
                    self.handle_move_column_up();
                }
            }
            KeyCode::Char('j') | KeyCode::Down => match self.focus.board_focus {
                BoardFocus::Sprints => {
                    if let Some(board) = self.active_board() {
                        {
                            let sprint_count = self
                                .model
                                .sprints()
                                .iter()
                                .filter(|s| s.board_id == board.id)
                                .count();
                            let current_idx = self.selection.sprint.get().unwrap_or(0);
                            if sprint_count == 0 || current_idx >= sprint_count - 1 {
                                self.focus.board_focus = BoardFocus::Columns;
                                self.dialog_input.column_selection.set(Some(0));
                            } else {
                                self.selection.sprint.next(sprint_count);
                            }
                        }
                    }
                }
                BoardFocus::Columns => {
                    if let Some(board) = self.active_board() {
                        {
                            let column_count = self
                                .model
                                .columns()
                                .iter()
                                .filter(|col| col.board_id == board.id)
                                .count();
                            let current_idx = self.dialog_input.column_selection.get().unwrap_or(0);
                            if column_count > 0 && current_idx >= column_count - 1 {
                                self.focus.board_focus = BoardFocus::Name;
                                self.selection.sprint.set(Some(0));
                            } else {
                                self.dialog_input.column_selection.next(column_count);
                            }
                        }
                    }
                }
                _ => {
                    self.focus.board_focus = match self.focus.board_focus {
                        BoardFocus::Name => BoardFocus::Description,
                        BoardFocus::Description => BoardFocus::Settings,
                        BoardFocus::Settings => BoardFocus::Sprints,
                        BoardFocus::Sprints => BoardFocus::Columns,
                        BoardFocus::Columns => BoardFocus::Name,
                    };
                    if self.focus.board_focus == BoardFocus::Sprints {
                        self.selection.sprint.set(Some(0));
                    } else if self.focus.board_focus == BoardFocus::Columns {
                        self.dialog_input.column_selection.set(Some(0));
                    }
                }
            },
            KeyCode::Char('k') | KeyCode::Up => match self.focus.board_focus {
                BoardFocus::Sprints => {
                    let current_idx = self.selection.sprint.get().unwrap_or(0);
                    if current_idx == 0 {
                        self.focus.board_focus = BoardFocus::Settings;
                    } else {
                        self.selection.sprint.prev();
                    }
                }
                BoardFocus::Columns => {
                    let current_idx = self.dialog_input.column_selection.get().unwrap_or(0);
                    if current_idx == 0 {
                        let sprint_count = self
                            .selection
                            .board
                            .get()
                            .and_then(|idx| {
                                self.displayed_boards().get(idx).map(|board| {
                                    self.model
                                        .sprints()
                                        .iter()
                                        .filter(|s| s.board_id == board.id)
                                        .count()
                                })
                            })
                            .unwrap_or(0);
                        if sprint_count == 0 {
                            self.focus.board_focus = BoardFocus::Settings;
                        } else {
                            self.focus.board_focus = BoardFocus::Sprints;
                            self.selection.sprint.set(Some(sprint_count - 1));
                        }
                    } else {
                        self.dialog_input.column_selection.prev();
                    }
                }
                _ => {
                    self.focus.board_focus = match self.focus.board_focus {
                        BoardFocus::Name => BoardFocus::Columns,
                        BoardFocus::Description => BoardFocus::Name,
                        BoardFocus::Settings => BoardFocus::Description,
                        BoardFocus::Sprints => BoardFocus::Settings,
                        BoardFocus::Columns => BoardFocus::Sprints,
                    };
                    if self.focus.board_focus == BoardFocus::Columns {
                        self.dialog_input.column_selection.set(Some(0));
                    }
                }
            },
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.focus.board_focus == BoardFocus::Sprints {
                    if let Some(sprint_idx) = self.selection.sprint.get() {
                        let board_ctx = self.board_in_context().map(|b| b.id);
                        if let Some(board_id) = board_ctx {
                            let sprints = self.model.sprints();
                            let board_sprints: Vec<_> = sprints
                                .iter()
                                .enumerate()
                                .filter(|(_, s)| s.board_id == board_id)
                                .collect();
                            if let Some((actual_idx, _)) = board_sprints.get(sprint_idx) {
                                let actual_idx = *actual_idx;
                                let sprint_id = sprints.get(actual_idx).map(|s| s.id);
                                self.selection.active_sprint_index = Some(actual_idx);
                                self.selection.active_board_id = Some(board_id);
                                if let Some(sprint_id) = sprint_id {
                                    self.populate_sprint_task_lists(sprint_id);
                                }
                                self.push_mode(AppMode::SprintDetail);
                            }
                        }
                    }
                }
            }
            KeyCode::Char('p') => {
                if self.focus.board_focus == BoardFocus::Settings {
                    if let Some(current_prefix) = self
                        .active_board()
                        .map(|b| b.sprint_prefix.clone().unwrap_or_default())
                    {
                        self.input.set(current_prefix);
                        self.open_dialog(DialogMode::SetBranchPrefix);
                    }
                }
            }
            _ => {}
        }
        should_restart
    }

    pub fn handle_sprint_detail_key(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
                self.focus.board_focus = BoardFocus::Sprints;
                self.selection.active_sprint_index = None;
            }
            KeyCode::Char('a') => {
                self.handle_activate_sprint_key();
            }
            KeyCode::Char('c') => {
                let selected = if self.sprint_view.panel == SprintTaskPanel::Uncompleted {
                    self.sprint_view.uncompleted_component.get_multi_selected()
                } else {
                    vec![]
                };
                if !selected.is_empty() {
                    self.toggle_completion_for_card_ids(selected);
                    self.sprint_view.uncompleted_component.clear_multi_select();
                } else {
                    self.handle_complete_sprint_key();
                }
            }
            KeyCode::Char('d') => {
                let selected = if self.sprint_view.panel == SprintTaskPanel::Uncompleted {
                    self.sprint_view.uncompleted_component.get_multi_selected()
                } else {
                    vec![]
                };
                if !selected.is_empty() {
                    self.start_delete_animations_for_card_ids(selected);
                    self.sprint_view.uncompleted_component.clear_multi_select();
                }
            }
            KeyCode::Char('p') => {
                if let Some(sprint_idx) = self.selection.active_sprint_index {
                    if let Some(sprint) = self.model.sprints().get(sprint_idx) {
                        let current_prefix = sprint.prefix.clone().unwrap_or_else(String::new);
                        self.input.set(current_prefix);
                        self.open_dialog(DialogMode::SetSprintPrefix);
                    }
                }
            }
            KeyCode::Char('C') => {
                if let Some(sprint_idx) = self.selection.active_sprint_index {
                    if let Some(sprint) = self.model.sprints().get(sprint_idx) {
                        let current_prefix = sprint.card_prefix.clone().unwrap_or_else(String::new);
                        self.input.set(current_prefix);
                        self.open_dialog(DialogMode::SetSprintCardPrefix);
                    }
                }
            }
            KeyCode::Char('o') => {
                let sort_idx = self.get_current_sort_field_selection_index();
                self.filter.sort_field_selection.set(Some(sort_idx));
                self.open_dialog(DialogMode::OrderCards);
            }
            KeyCode::Char('O') => {
                if let Some(current_order) = self.filter.current_sort_order {
                    let new_order = match current_order {
                        kanban_domain::SortOrder::Ascending => kanban_domain::SortOrder::Descending,
                        kanban_domain::SortOrder::Descending => kanban_domain::SortOrder::Ascending,
                    };
                    self.filter.current_sort_order = Some(new_order);

                    if let Some(field) = self.filter.current_sort_field {
                        self.apply_sort_to_sprint_lists(field, new_order);
                        tracing::info!("Toggled sort order to: {:?}", new_order);
                    }
                }
            }
            KeyCode::Char('M') => {
                if let Some(sprint_idx) = self.selection.active_sprint_index {
                    if let Some(sprint) = self.model.sprints().get(sprint_idx) {
                        use kanban_domain::SprintStatus;
                        if sprint.status == SprintStatus::Completed
                            || sprint.status == SprintStatus::Cancelled
                        {
                            let sprint_id = sprint.id;
                            self.handle_carry_over_for_sprint(sprint_id);
                        }
                    }
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if let Some(sprint_idx) = self.selection.active_sprint_index {
                    if let Some(sprint) = self.model.sprints().get(sprint_idx) {
                        if sprint.status == kanban_domain::SprintStatus::Completed {
                            self.sprint_view.panel = SprintTaskPanel::Uncompleted;
                        }
                    }
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if let Some(sprint_idx) = self.selection.active_sprint_index {
                    if let Some(sprint) = self.model.sprints().get(sprint_idx) {
                        if sprint.status == kanban_domain::SprintStatus::Completed {
                            self.sprint_view.panel = SprintTaskPanel::Completed;
                        }
                    }
                }
            }
            _ => {
                let action = {
                    let active_component = match self.sprint_view.panel {
                        SprintTaskPanel::Uncompleted => &mut self.sprint_view.uncompleted_component,
                        SprintTaskPanel::Completed => &mut self.sprint_view.completed_component,
                    };
                    active_component.handle_key(key_code)
                };

                if let Some(action) = action {
                    use crate::card_list_component::CardListAction;

                    match action {
                        CardListAction::Select(card_id) => {
                            if self.activate_card(card_id) {
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
                                self.focus.card_focus = CardFocus::Title;
                            }
                        }
                        CardListAction::Edit(card_id) => {
                            if self.activate_card(card_id) {
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
                                self.focus.card_focus = CardFocus::Title;
                            }
                        }
                        CardListAction::Complete(card_id) => {
                            if let Some(card) = self.model.cards().iter().find(|c| c.id == card_id)
                            {
                                use kanban_domain::{CardStatus, CardUpdate, KanbanOperations};
                                let new_status = if card.status == CardStatus::Done {
                                    CardStatus::Todo
                                } else {
                                    CardStatus::Done
                                };

                                // Service layer chains the column move automatically.
                                if let Err(e) = self.ctx.update_card(
                                    card_id,
                                    CardUpdate {
                                        status: Some(new_status),
                                        ..Default::default()
                                    },
                                ) {
                                    tracing::error!("Failed to toggle card completion: {}", e);
                                    self.set_error(format!(
                                        "Failed to toggle card completion: {}",
                                        e
                                    ));
                                }
                            }
                        }
                        CardListAction::TogglePriority(card_id) => {
                            if self.activate_card(card_id) {
                                let priority_idx = self.get_current_priority_selection_index();
                                self.dialog_input.priority_selection.set(Some(priority_idx));
                                self.open_dialog(DialogMode::SetCardPriority);
                            }
                        }
                        CardListAction::AssignSprint(card_id)
                        | CardListAction::ReassignSprint(card_id) => {
                            if self.activate_card(card_id) {
                                if let Some(board) = self.active_board().cloned() {
                                    let sprint_count = self
                                        .model
                                        .sprints()
                                        .iter()
                                        .filter(|s| s.board_id == board.id)
                                        .count();
                                    if sprint_count > 0 {
                                        let current_sprint_id = self
                                            .model
                                            .card_by_id(card_id)
                                            .and_then(|c| c.sprint_id);
                                        self.dialog_input
                                            .assign_sprint_picker
                                            .reset_for_card_assignment(
                                                current_sprint_id,
                                                self.model.sprints(),
                                                &board,
                                                chrono::Utc::now(),
                                            );
                                        self.open_dialog(DialogMode::AssignCardToSprint);
                                    }
                                }
                            }
                        }
                        CardListAction::Sort => {
                            let sort_idx = self.get_current_sort_field_selection_index();
                            self.filter.sort_field_selection.set(Some(sort_idx));
                            self.open_dialog(DialogMode::OrderCards);
                        }
                        CardListAction::OrderCards => {
                            if let Some(current_order) = self.filter.current_sort_order {
                                let new_order = match current_order {
                                    kanban_domain::SortOrder::Ascending => {
                                        kanban_domain::SortOrder::Descending
                                    }
                                    kanban_domain::SortOrder::Descending => {
                                        kanban_domain::SortOrder::Ascending
                                    }
                                };
                                self.filter.current_sort_order = Some(new_order);

                                if let Some(field) = self.filter.current_sort_field {
                                    self.apply_sort_to_sprint_lists(field, new_order);
                                    tracing::info!("Toggled sort order to: {:?}", new_order);
                                }
                            }
                        }
                        CardListAction::MoveColumn(card_id, is_right) => {
                            if let Some(card) =
                                self.model.cards().iter().find(|c| c.id == card_id).cloned()
                            {
                                let direction = if is_right {
                                    kanban_domain::card_lifecycle::MoveDirection::Right
                                } else {
                                    kanban_domain::card_lifecycle::MoveDirection::Left
                                };

                                let columns = self.model.columns();
                                let cards = self.model.cards();
                                let move_result = self.active_board().and_then(|board| {
                                    kanban_domain::card_lifecycle::compute_card_column_move(
                                        &card, board, columns, cards, direction,
                                    )
                                });

                                if let Some(result) = move_result {
                                    use kanban_domain::KanbanOperations;
                                    // Service layer chains the status flip when the
                                    // move crosses the completion-column boundary.
                                    if let Err(e) =
                                        self.ctx.move_card(card_id, result.target_column_id, None)
                                    {
                                        tracing::error!("Failed to move card: {}", e);
                                        self.set_error(format!("Failed to move card: {}", e));
                                    }
                                }
                            }
                        }
                        CardListAction::Create => {
                            self.open_dialog(DialogMode::CreateCard);
                            self.input.clear();
                        }
                        CardListAction::ToggleMultiSelect(card_id) => {
                            let component = match self.sprint_view.panel {
                                SprintTaskPanel::Uncompleted => {
                                    &mut self.sprint_view.uncompleted_component
                                }
                                SprintTaskPanel::Completed => {
                                    &mut self.sprint_view.completed_component
                                }
                            };
                            component.toggle_multi_select(card_id);
                        }
                        CardListAction::ClearMultiSelect => {
                            let component = match self.sprint_view.panel {
                                SprintTaskPanel::Uncompleted => {
                                    &mut self.sprint_view.uncompleted_component
                                }
                                SprintTaskPanel::Completed => {
                                    &mut self.sprint_view.completed_component
                                }
                            };
                            component.clear_multi_select();
                        }
                        CardListAction::SelectAll => {
                            let component = match self.sprint_view.panel {
                                SprintTaskPanel::Uncompleted => {
                                    &mut self.sprint_view.uncompleted_component
                                }
                                SprintTaskPanel::Completed => {
                                    &mut self.sprint_view.completed_component
                                }
                            };
                            component.select_all();
                        }
                    }
                }

                // Sync component selection back to CardList for rendering
                let (active_component, active_card_list) = match self.sprint_view.panel {
                    SprintTaskPanel::Uncompleted => (
                        &self.sprint_view.uncompleted_component,
                        &mut self.sprint_view.uncompleted_cards,
                    ),
                    SprintTaskPanel::Completed => (
                        &self.sprint_view.completed_component,
                        &mut self.sprint_view.completed_cards,
                    ),
                };
                active_card_list.set_selected_index(active_component.get_selected_index());
            }
        }
    }

    pub(crate) fn handle_manage_parents(&mut self) {
        if let Some(active_id) = self.selection.active_card_id {
            if let Some(card) = self.model.card_by_id(active_id) {
                let card_id = card.id;
                let card_column_id = card.column_id;

                // Get the board for this card's column
                let board_id = self
                    .model
                    .columns()
                    .iter()
                    .find(|c| c.id == card_column_id)
                    .map(|c| c.board_id);

                if let Some(board_id) = board_id {
                    // Get all descendants to exclude (to prevent cycles)
                    let descendants = self.model.graph().descendants(card_id);

                    // Get cards from current board, excluding self and descendants
                    let column_ids: std::collections::HashSet<_> = self
                        .model
                        .columns()
                        .iter()
                        .filter(|c| c.board_id == board_id)
                        .map(|c| c.id)
                        .collect();

                    let eligible_cards: Vec<_> = self
                        .model
                        .cards()
                        .iter()
                        .filter(|c| column_ids.contains(&c.column_id))
                        .filter(|c| c.id != card_id)
                        .filter(|c| !descendants.contains(&c.id))
                        .map(|c| c.id)
                        .collect();

                    // Get current parents (for checkbox display)
                    let current_parents: std::collections::HashSet<_> =
                        self.model.graph().parents(card_id).into_iter().collect();

                    // Set up dialog state
                    self.relationship.card_ids = eligible_cards;
                    self.relationship.selected = current_parents;
                    self.relationship.selection.set(Some(0));
                    self.relationship.search.clear();

                    self.open_dialog(DialogMode::ManageParents);
                }
            }
        }
    }

    pub(crate) fn handle_manage_children(&mut self) {
        if let Some(active_id) = self.selection.active_card_id {
            if let Some(card) = self.model.card_by_id(active_id) {
                let card_id = card.id;
                let card_column_id = card.column_id;

                // Get the board for this card's column
                let board_id = self
                    .model
                    .columns()
                    .iter()
                    .find(|c| c.id == card_column_id)
                    .map(|c| c.board_id);

                if let Some(board_id) = board_id {
                    // Get all ancestors to exclude (to prevent cycles)
                    let ancestors = self.model.graph().ancestors(card_id);

                    // Get cards from current board, excluding self and ancestors
                    let column_ids: std::collections::HashSet<_> = self
                        .model
                        .columns()
                        .iter()
                        .filter(|c| c.board_id == board_id)
                        .map(|c| c.id)
                        .collect();

                    let eligible_cards: Vec<_> = self
                        .model
                        .cards()
                        .iter()
                        .filter(|c| column_ids.contains(&c.column_id))
                        .filter(|c| c.id != card_id)
                        .filter(|c| !ancestors.contains(&c.id))
                        .map(|c| c.id)
                        .collect();

                    // Get current children (for checkbox display)
                    let current_children: std::collections::HashSet<_> =
                        self.model.graph().children(card_id).into_iter().collect();

                    // Set up dialog state
                    self.relationship.card_ids = eligible_cards;
                    self.relationship.selected = current_children;
                    self.relationship.selection.set(Some(0));
                    self.relationship.search.clear();

                    self.open_dialog(DialogMode::ManageChildren);
                }
            }
        }
    }

    pub fn get_current_card_parents(&self) -> Vec<uuid::Uuid> {
        if let Some(active_id) = self.selection.active_card_id {
            if let Some(card) = self.model.card_by_id(active_id) {
                return self.model.graph().parents(card.id);
            }
        }
        Vec::new()
    }

    pub fn get_current_card_children(&self) -> Vec<uuid::Uuid> {
        if let Some(active_id) = self.selection.active_card_id {
            if let Some(card) = self.model.card_by_id(active_id) {
                return self.model.graph().children(card.id);
            }
        }
        Vec::new()
    }

    fn refresh_relationship_counts(&mut self) {
        let parents = self.get_current_card_parents();
        let children = self.get_current_card_children();
        self.relationship
            .parents_list
            .update_item_count(parents.len());
        self.relationship
            .children_list
            .update_item_count(children.len());
    }

    fn related_card_ids(&self, side: RelationSide) -> Vec<uuid::Uuid> {
        match side {
            RelationSide::Parents => self.get_current_card_parents(),
            RelationSide::Children => self.get_current_card_children(),
        }
    }

    fn list_selection(&self, side: RelationSide) -> Option<usize> {
        match side {
            RelationSide::Parents => self.relationship.parents_list.selection.get(),
            RelationSide::Children => self.relationship.children_list.selection.get(),
        }
    }

    pub(crate) fn return_to_previous_card_from_detail_history(&mut self) {
        if let Some(previous_id) = self.selection.card_navigation_history.pop() {
            // Clear-on-miss is required here: an externally archived previous
            // card must clear the active selection so Backspace doesn't strand
            // the user on a stale card. Pinned by
            // test_backspace_return_with_unknown_previous_id_clears_active_card_entirely.
            self.set_active_card_or_clear(previous_id);
            self.focus.card_focus = CardFocus::Title;
            self.refresh_relationship_counts();
        }
    }

    pub(crate) fn navigate_to_selected_parent(&mut self) {
        self.navigate_to_related_card(RelationSide::Parents);
    }

    pub(crate) fn navigate_to_selected_child(&mut self) {
        self.navigate_to_related_card(RelationSide::Children);
    }

    fn navigate_to_related_card(&mut self, side: RelationSide) {
        let Some(current_card_id) = self.selection.active_card_id else {
            return;
        };
        let related = self.related_card_ids(side);
        let selected_id = self
            .list_selection(side)
            .and_then(|i| related.get(i).copied());
        // Prefer the list selection; fall back to the first related card.
        let candidates = selected_id.into_iter().chain(related.first().copied());

        for target_id in candidates {
            if self.activate_card(target_id) {
                self.selection.card_navigation_history.push(current_card_id);
                self.focus.card_focus = CardFocus::Title;
                self.refresh_relationship_counts();
                return;
            }
        }
    }

    pub fn start_delete_animations_for_card_ids(&mut self, ids: Vec<uuid::Uuid>) {
        for card_id in ids {
            self.start_delete_animation(card_id);
        }
    }

    pub fn toggle_completion_for_card_ids(&mut self, ids: Vec<uuid::Uuid>) {
        use kanban_domain::{CardStatus, CardUpdate, KanbanOperations};

        let updates: Vec<(uuid::Uuid, CardUpdate)> = ids
            .iter()
            .filter_map(|card_id| {
                let card = self
                    .model
                    .cards()
                    .iter()
                    .find(|c| c.id == *card_id)?
                    .clone();
                let new_status = if card.status == CardStatus::Done {
                    CardStatus::Todo
                } else {
                    CardStatus::Done
                };
                Some((
                    *card_id,
                    CardUpdate {
                        status: Some(new_status),
                        ..Default::default()
                    },
                ))
            })
            .collect();

        if !updates.is_empty() {
            if let Err(e) = self.ctx.update_cards(updates) {
                tracing::error!("Failed to toggle card completion: {}", e);
                self.set_error(format!("Failed to toggle card completion: {}", e));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::sprint_view::SprintTaskPanel;
    use crate::app::CardFocus;
    use crate::App;
    use crossterm::event::KeyCode;
    use kanban_domain::{CreateCardOptions, GraphOperations, KanbanOperations, Snapshot};

    fn reload_snapshot(app: &mut App) {
        let snap = Snapshot {
            archived_boards: Vec::new(),
            boards: app.ctx.data_store().list_boards().unwrap(),
            columns: app.ctx.data_store().list_all_columns().unwrap(),
            cards: app.ctx.data_store().list_all_cards().unwrap(),
            archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
            sprints: app.ctx.data_store().list_all_sprints().unwrap(),
            graph: app.ctx.data_store().get_graph().unwrap(),
        };
        app.model.load_from_snapshot(snap);
    }

    fn seed_chain(app: &mut App, titles: &[&str]) -> Vec<uuid::Uuid> {
        let board = app.ctx.create_board("Board".into(), None).unwrap();
        let column = app
            .ctx
            .create_column(board.id, "TODO".into(), None)
            .unwrap();
        let mut ids = Vec::new();
        for t in titles {
            let card = app
                .ctx
                .create_card(
                    board.id,
                    column.id,
                    (*t).into(),
                    CreateCardOptions::default(),
                )
                .unwrap();
            ids.push(card.id);
        }
        for w in ids.windows(2) {
            app.ctx.attach_child(w[0], w[1]).unwrap();
        }
        reload_snapshot(app);
        ids
    }

    fn seed_sprint_with_card(app: &mut App, title: &str) -> uuid::Uuid {
        let board = app.ctx.create_board("Board".into(), None).unwrap();
        let column = app
            .ctx
            .create_column(board.id, "TODO".into(), None)
            .unwrap();
        let sprint = app.ctx.create_sprint(board.id, None, None).unwrap();
        let card = app
            .ctx
            .create_card(
                board.id,
                column.id,
                title.into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        app.ctx.assign_card_to_sprint(card.id, sprint.id).unwrap();
        reload_snapshot(app);
        app.populate_sprint_task_lists(sprint.id);
        app.sprint_view.panel = SprintTaskPanel::Uncompleted;
        app.sprint_view
            .uncompleted_component
            .update_cards(vec![card.id]);
        app.sprint_view
            .uncompleted_component
            .set_selected_index(Some(0));
        card.id
    }

    #[test]
    fn test_navigate_to_selected_parent_updates_active_card_id_so_detail_view_reloads() {
        let mut app = App::test_default();
        let ids = seed_chain(&mut app, &["Parent", "Child"]);
        let parent_id = ids[0];
        let child_id = ids[1];

        app.selection.active_card_id = Some(child_id);
        app.focus.card_focus = CardFocus::Parents;
        app.relationship.parents_list.update_item_count(1);
        app.relationship.parents_list.selection.set(Some(0));

        app.navigate_to_selected_parent();

        assert_eq!(
            app.selection.active_card_id,
            Some(parent_id),
            "active_card.id() must be updated to the parent so the detail view rerenders against the parent card"
        );
        assert_eq!(
            app.get_card_for_detail_view()
                .expect("detail must resolve")
                .id,
            parent_id,
            "get_card_for_detail_view() must return the parent after Enter on a parent entry"
        );
    }

    #[test]
    fn test_navigate_to_selected_child_updates_active_card_id_so_detail_view_reloads() {
        let mut app = App::test_default();
        let ids = seed_chain(&mut app, &["Parent", "Child"]);
        let parent_id = ids[0];
        let child_id = ids[1];

        app.selection.active_card_id = Some(parent_id);
        app.focus.card_focus = CardFocus::Children;
        app.relationship.children_list.update_item_count(1);
        app.relationship.children_list.selection.set(Some(0));

        app.navigate_to_selected_child();

        assert_eq!(app.selection.active_card_id, Some(child_id));
        assert_eq!(
            app.get_card_for_detail_view()
                .expect("detail must resolve")
                .id,
            child_id
        );
    }

    #[test]
    fn test_backspace_return_from_detail_history_updates_active_card_id() {
        let mut app = App::test_default();
        let ids = seed_chain(&mut app, &["A", "B", "C"]);
        let b_id = ids[1];
        let c_id = ids[2];

        app.selection.active_card_id = Some(c_id);
        app.selection.card_navigation_history.push(b_id);
        app.focus.card_focus = CardFocus::Parents;

        app.return_to_previous_card_from_detail_history();

        assert_eq!(
            app.selection.active_card_id,
            Some(b_id),
            "Backspace return must update active_card to the previous card"
        );
        assert_eq!(
            app.get_card_for_detail_view()
                .expect("detail must resolve")
                .id,
            b_id
        );
    }

    #[test]
    fn test_sprint_detail_enter_on_card_sets_active_card_id_so_detail_view_resolves() {
        let mut app = App::test_default();
        let card_id = seed_sprint_with_card(&mut app, "task");

        app.handle_sprint_detail_key(KeyCode::Enter);

        assert_eq!(
            app.selection.active_card_id,
            Some(card_id),
            "Enter on a sprint-detail card row must set active_card so the detail view can resolve the card"
        );
        assert_eq!(
            app.get_card_for_detail_view()
                .expect("detail must resolve")
                .id,
            card_id
        );
    }

    #[test]
    fn test_sprint_detail_e_on_card_sets_active_card_id_so_detail_view_resolves() {
        let mut app = App::test_default();
        let card_id = seed_sprint_with_card(&mut app, "task");

        app.handle_sprint_detail_key(KeyCode::Char('e'));

        assert_eq!(
            app.selection.active_card_id,
            Some(card_id),
            "'e' on a sprint-detail card row must set active_card so the detail view can resolve the card"
        );
        assert_eq!(
            app.get_card_for_detail_view()
                .expect("detail must resolve")
                .id,
            card_id
        );
    }

    #[test]
    fn test_navigate_to_selected_parent_falls_back_to_first_parent_when_no_list_selection() {
        let mut app = App::test_default();
        let ids = seed_chain(&mut app, &["Parent", "Child"]);
        let parent_id = ids[0];
        let child_id = ids[1];

        app.selection.active_card_id = Some(child_id);
        app.focus.card_focus = CardFocus::Parents;
        app.relationship.parents_list.update_item_count(1);
        // Deliberately no parents_list.selection.set(...) — exercise the fallback path.

        app.navigate_to_selected_parent();

        assert_eq!(
            app.selection.active_card_id,
            Some(parent_id),
            "with no list selection, Enter on Parents must fall back to navigating to the first parent"
        );
    }

    #[test]
    fn test_navigate_to_selected_child_falls_back_to_first_child_when_no_list_selection() {
        let mut app = App::test_default();
        let ids = seed_chain(&mut app, &["Parent", "Child"]);
        let parent_id = ids[0];
        let child_id = ids[1];

        app.selection.active_card_id = Some(parent_id);
        app.focus.card_focus = CardFocus::Children;
        app.relationship.children_list.update_item_count(1);
        // Deliberately no children_list.selection.set(...).

        app.navigate_to_selected_child();

        assert_eq!(
            app.selection.active_card_id,
            Some(child_id),
            "with no list selection, Enter on Children must fall back to navigating to the first child"
        );
    }

    #[test]
    fn test_backspace_return_with_unknown_previous_id_clears_active_card_entirely() {
        let mut app = App::test_default();
        let ids = seed_chain(&mut app, &["A", "B"]);
        let b_id = ids[1];
        let unknown_id = uuid::Uuid::new_v4();

        app.selection.active_card_id = Some(b_id);
        app.selection.card_navigation_history.push(unknown_id);

        app.return_to_previous_card_from_detail_history();

        assert!(
            app.selection.active_card_id.is_none(),
            "when previous_id no longer resolves to a card in the model, active_card must be cleared"
        );
        assert!(
            app.get_card_for_detail_view().is_none(),
            "detail view must resolve to None when active_card was cleared by an unknown-id recovery"
        );
    }

    use crate::test_helpers::{load_with_card_order, setup_reload_resort_fixture};

    #[test]
    fn test_return_to_previous_card_after_reload_resort_returns_to_originally_visited_card() {
        let mut app = App::test_default();
        let board = app.ctx.create_board("Board".into(), None).unwrap();
        let column = app
            .ctx
            .create_column(board.id, "Todo".into(), None)
            .unwrap();
        let a = app
            .ctx
            .create_card(
                board.id,
                column.id,
                "A".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        let p = app
            .ctx
            .create_card(
                board.id,
                column.id,
                "P".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        let b = app
            .ctx
            .create_card(
                board.id,
                column.id,
                "B".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        let c = app
            .ctx
            .create_card(
                board.id,
                column.id,
                "C".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        let d = app
            .ctx
            .create_card(
                board.id,
                column.id,
                "D".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        app.ctx.attach_child(a.id, d.id).unwrap();

        load_with_card_order(&mut app, &[a.id, p.id, b.id, c.id, d.id]);
        app.selection.active_card_id = Some(a.id);
        app.selection.active_board_id = app.model.boards().first().map(|b| b.id);

        app.focus.card_focus = CardFocus::Children;
        app.relationship.children_list.update_item_count(1);
        app.relationship.children_list.selection.set(Some(0));
        app.navigate_to_selected_child();
        assert_eq!(
            app.selection.active_card_id,
            Some(d.id),
            "precondition: navigate_to_selected_child must have set active_card to D"
        );

        load_with_card_order(&mut app, &[p.id, b.id, a.id, c.id, d.id]);

        app.return_to_previous_card_from_detail_history();

        assert_eq!(
            app.selection.active_card_id,
            Some(a.id),
            "backspace must return to A (originally visited, by id) — not whatever card now sits at A's old slot after the external reload re-ordered cards()"
        );
    }

    #[test]
    fn test_get_current_card_parents_after_reload_resort_returns_originally_selected_card_parents()
    {
        let mut app = App::test_default();
        let fx = setup_reload_resort_fixture(&mut app);

        let parents = app.get_current_card_parents();

        assert_eq!(
            parents,
            vec![fx.p_id],
            "after reload-resort, parents of the active card (A) must be returned by id; resolving by stale index would return parents of the wrong card"
        );
    }

    #[test]
    fn test_get_current_card_children_after_reload_resort_returns_originally_selected_card_children(
    ) {
        let mut app = App::test_default();
        let fx = setup_reload_resort_fixture(&mut app);

        let children = app.get_current_card_children();

        assert_eq!(
            children,
            vec![fx.d_id],
            "after reload-resort, children of the active card (A) must be returned by id; resolving by stale index would return children of the wrong card"
        );
    }

    #[test]
    fn test_handle_manage_parents_after_reload_resort_uses_originally_selected_card() {
        let mut app = App::test_default();
        let fx = setup_reload_resort_fixture(&mut app);

        app.handle_manage_parents();

        assert!(
            app.relationship.selected.contains(&fx.p_id),
            "manage_parents must mark P as a current parent of A — selection must be built from the active card's id, not from a stale index that resolves to a different card"
        );
        assert!(
            app.relationship.card_ids.contains(&fx.p_id),
            "manage_parents eligibility must include P as a candidate parent of A — built for A by id, not for the wrong card at A's stale index"
        );
    }

    #[test]
    fn test_handle_manage_children_after_reload_resort_uses_originally_selected_card() {
        let mut app = App::test_default();
        let fx = setup_reload_resort_fixture(&mut app);

        app.handle_manage_children();

        assert!(
            app.relationship.selected.contains(&fx.d_id),
            "manage_children must mark D as a current child of A — selection must be built from the active card's id, not from a stale index that resolves to a different card"
        );
        assert!(
            !app.relationship.card_ids.contains(&fx.p_id),
            "manage_children eligibility must exclude P as A's ancestor — built for A by id, not for the wrong card at A's stale index"
        );
    }

    #[test]
    fn test_get_card_for_detail_view_after_reload_resort_returns_originally_selected_card() {
        let mut app = App::test_default();
        let fx = setup_reload_resort_fixture(&mut app);

        let card = app
            .get_card_for_detail_view()
            .expect("detail view must resolve when active card is set");

        assert_eq!(
            card.id, fx.a_id,
            "get_card_for_detail_view must return the originally selected card (A) by id, not the card now at A's stale index"
        );
    }
}
