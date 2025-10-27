use crate::app::{App, AppMode, BoardField, BoardFocus, CardField, CardFocus, SprintTaskPanel};
use crate::events::EventHandler;
use crossterm::event::KeyCode;
use kanban_domain::{BoardSettingsDto, CardMetadataDto, ColumnMetadataDto, SprintMetadataDto};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

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
                self.mode = AppMode::Normal;
                self.active_card_index = None;
                self.card_focus = CardFocus::Title;
            }
            KeyCode::Char('1') => {
                self.card_focus = CardFocus::Title;
            }
            KeyCode::Char('2') => {
                self.card_focus = CardFocus::Metadata;
            }
            KeyCode::Char('3') => {
                self.card_focus = CardFocus::Description;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.card_focus = match self.card_focus {
                    CardFocus::Title => CardFocus::Metadata,
                    CardFocus::Metadata => CardFocus::Description,
                    CardFocus::Description => CardFocus::Title,
                };
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.card_focus = match self.card_focus {
                    CardFocus::Title => CardFocus::Description,
                    CardFocus::Description => CardFocus::Metadata,
                    CardFocus::Metadata => CardFocus::Title,
                };
            }
            KeyCode::Char('y') => {
                self.copy_branch_name();
            }
            KeyCode::Char('Y') => {
                self.copy_git_checkout_command();
            }
            KeyCode::Char('e') => match self.card_focus {
                CardFocus::Title => {
                    if let Err(e) = self.edit_card_field(terminal, event_handler, CardField::Title)
                    {
                        tracing::error!("Failed to edit title: {}", e);
                    }
                    should_restart = true;
                }
                CardFocus::Description => {
                    if let Err(e) =
                        self.edit_card_field(terminal, event_handler, CardField::Description)
                    {
                        tracing::error!("Failed to edit description: {}", e);
                    }
                    should_restart = true;
                }
                CardFocus::Metadata => {
                    if let Some(card_idx) = self.active_card_index {
                        if let Some(card) = self.cards.get_mut(card_idx) {
                            let card_id = card.id;
                            let temp_file = std::env::temp_dir()
                                .join(format!("kanban-card-{}-metadata.json", card_id));
                            if let Err(e) = App::edit_entity_json_impl::<CardMetadataDto, _>(
                                card,
                                terminal,
                                event_handler,
                                temp_file,
                            ) {
                                tracing::error!("Failed to edit metadata: {}", e);
                            }
                            should_restart = true;
                        }
                    }
                }
            },
            KeyCode::Char('s') => {
                if let Some(board_idx) = self.active_board_index {
                    if let Some(board) = self.boards.get(board_idx) {
                        let sprint_count = self
                            .sprints
                            .iter()
                            .filter(|s| s.board_id == board.id)
                            .count();
                        if sprint_count > 0 {
                            let selection_idx = self.get_current_sprint_selection_index();
                            self.sprint_assign_selection.set(Some(selection_idx));
                            self.mode = AppMode::AssignCardToSprint;
                        }
                    }
                }
            }
            KeyCode::Char('p') => {
                let priority_idx = self.get_current_priority_selection_index();
                self.priority_selection.set(Some(priority_idx));
                self.mode = AppMode::SetCardPriority;
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
                self.mode = AppMode::Normal;
                self.board_focus = BoardFocus::Name;
            }
            KeyCode::Char('1') => {
                self.board_focus = BoardFocus::Name;
            }
            KeyCode::Char('2') => {
                self.board_focus = BoardFocus::Description;
            }
            KeyCode::Char('3') => {
                self.board_focus = BoardFocus::Settings;
            }
            KeyCode::Char('4') => {
                self.board_focus = BoardFocus::Sprints;
            }
            KeyCode::Char('5') => {
                self.board_focus = BoardFocus::Columns;
            }
            KeyCode::Char('e') => match self.board_focus {
                BoardFocus::Name => {
                    if let Err(e) = self.edit_board_field(terminal, event_handler, BoardField::Name)
                    {
                        tracing::error!("Failed to edit board name: {}", e);
                    }
                    should_restart = true;
                }
                BoardFocus::Description => {
                    if let Err(e) =
                        self.edit_board_field(terminal, event_handler, BoardField::Description)
                    {
                        tracing::error!("Failed to edit board description: {}", e);
                    }
                    should_restart = true;
                }
                BoardFocus::Settings => {
                    if let Some(board_idx) = self.board_selection.get() {
                        if let Some(board) = self.boards.get_mut(board_idx) {
                            let board_id = board.id;
                            let temp_file = std::env::temp_dir()
                                .join(format!("kanban-board-{}-settings.json", board_id));
                            if let Err(e) = App::edit_entity_json_impl::<BoardSettingsDto, _>(
                                board,
                                terminal,
                                event_handler,
                                temp_file,
                            ) {
                                tracing::error!("Failed to edit board settings: {}", e);
                            }
                            should_restart = true;
                        }
                    }
                }
                BoardFocus::Sprints => {
                    if let Some(sprint_idx) = self.sprint_selection.get() {
                        if let Some(board_idx) = self.board_selection.get() {
                            if let Some(board) = self.boards.get(board_idx) {
                                let actual_idx_and_sprint_id = {
                                    let board_sprints: Vec<_> = self
                                        .sprints
                                        .iter()
                                        .enumerate()
                                        .filter(|(_, s)| s.board_id == board.id)
                                        .collect();
                                    board_sprints.get(sprint_idx).map(|(idx, s)| (*idx, s.id))
                                };
                                if let Some((actual_idx, sprint_id)) = actual_idx_and_sprint_id {
                                    let temp_file = std::env::temp_dir()
                                        .join(format!("kanban-sprint-{}-metadata.json", sprint_id));
                                    if let Some(sprint_mut) = self.sprints.get_mut(actual_idx) {
                                        if let Err(e) = App::edit_entity_json_impl::<SprintMetadataDto, _>(
                                            sprint_mut,
                                            terminal,
                                            event_handler,
                                            temp_file,
                                        ) {
                                            tracing::error!("Failed to edit sprint metadata: {}", e);
                                        }
                                        should_restart = true;
                                    }
                                }
                            }
                        }
                    }
                }
                BoardFocus::Columns => {
                    if let Some(column_idx) = self.column_selection.get() {
                        if let Some(board_idx) = self.board_selection.get() {
                            if let Some(board) = self.boards.get(board_idx) {
                                let actual_idx_and_column_id = {
                                    let board_columns: Vec<_> = self
                                        .columns
                                        .iter()
                                        .enumerate()
                                        .filter(|(_, c)| c.board_id == board.id)
                                        .collect();
                                    board_columns.get(column_idx).map(|(idx, c)| (*idx, c.id))
                                };
                                if let Some((actual_idx, column_id)) = actual_idx_and_column_id {
                                    let temp_file = std::env::temp_dir()
                                        .join(format!("kanban-column-{}-metadata.json", column_id));
                                    if let Some(column_mut) = self.columns.get_mut(actual_idx) {
                                        if let Err(e) = App::edit_entity_json_impl::<ColumnMetadataDto, _>(
                                            column_mut,
                                            terminal,
                                            event_handler,
                                            temp_file,
                                        ) {
                                            tracing::error!("Failed to edit column metadata: {}", e);
                                        }
                                        should_restart = true;
                                    }
                                }
                            }
                        }
                    }
                }
            },
            KeyCode::Char('n') => {
                if self.board_focus == BoardFocus::Sprints {
                    self.handle_create_sprint_key();
                } else if self.board_focus == BoardFocus::Columns {
                    self.handle_create_column_key();
                }
            }
            KeyCode::Char('r') => {
                if self.board_focus == BoardFocus::Columns {
                    self.handle_rename_column_key();
                }
            }
            KeyCode::Char('d') => {
                if self.board_focus == BoardFocus::Columns {
                    self.handle_delete_column_key();
                }
            }
            KeyCode::Char('J') => {
                if self.board_focus == BoardFocus::Columns {
                    self.handle_move_column_down();
                }
            }
            KeyCode::Char('K') => {
                if self.board_focus == BoardFocus::Columns {
                    self.handle_move_column_up();
                }
            }
            KeyCode::Char('j') | KeyCode::Down => match self.board_focus {
                BoardFocus::Sprints => {
                    if let Some(board_idx) = self.board_selection.get() {
                        if let Some(board) = self.boards.get(board_idx) {
                            let sprint_count = self
                                .sprints
                                .iter()
                                .filter(|s| s.board_id == board.id)
                                .count();
                            let current_idx = self.sprint_selection.get().unwrap_or(0);
                            if current_idx >= sprint_count - 1 && sprint_count > 0 {
                                self.board_focus = BoardFocus::Columns;
                                self.column_selection.set(Some(0));
                            } else {
                                self.sprint_selection.next(sprint_count);
                            }
                        }
                    }
                }
                BoardFocus::Columns => {
                    if let Some(board_idx) = self.board_selection.get() {
                        if let Some(board) = self.boards.get(board_idx) {
                            let column_count = self
                                .columns
                                .iter()
                                .filter(|col| col.board_id == board.id)
                                .count();
                            let current_idx = self.column_selection.get().unwrap_or(0);
                            if current_idx >= column_count - 1 && column_count > 0 {
                                self.board_focus = BoardFocus::Name;
                                self.sprint_selection.set(Some(0));
                            } else {
                                self.column_selection.next(column_count);
                            }
                        }
                    }
                }
                _ => {
                    self.board_focus = match self.board_focus {
                        BoardFocus::Name => BoardFocus::Description,
                        BoardFocus::Description => BoardFocus::Settings,
                        BoardFocus::Settings => BoardFocus::Sprints,
                        BoardFocus::Sprints => BoardFocus::Columns,
                        BoardFocus::Columns => BoardFocus::Name,
                    };
                    if self.board_focus == BoardFocus::Sprints {
                        self.sprint_selection.set(Some(0));
                    } else if self.board_focus == BoardFocus::Columns {
                        self.column_selection.set(Some(0));
                    }
                }
            },
            KeyCode::Char('k') | KeyCode::Up => match self.board_focus {
                BoardFocus::Sprints => {
                    let current_idx = self.sprint_selection.get().unwrap_or(0);
                    if current_idx == 0 {
                        self.board_focus = BoardFocus::Settings;
                    } else {
                        self.sprint_selection.prev();
                    }
                }
                BoardFocus::Columns => {
                    let current_idx = self.column_selection.get().unwrap_or(0);
                    if current_idx == 0 {
                        self.board_focus = BoardFocus::Sprints;
                        self.sprint_selection.set(Some(0));
                    } else {
                        self.column_selection.prev();
                    }
                }
                _ => {
                    self.board_focus = match self.board_focus {
                        BoardFocus::Name => BoardFocus::Columns,
                        BoardFocus::Description => BoardFocus::Name,
                        BoardFocus::Settings => BoardFocus::Description,
                        BoardFocus::Sprints => BoardFocus::Settings,
                        BoardFocus::Columns => BoardFocus::Sprints,
                    };
                    if self.board_focus == BoardFocus::Columns {
                        self.column_selection.set(Some(0));
                    }
                }
            },
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.board_focus == BoardFocus::Sprints {
                    if let Some(sprint_idx) = self.sprint_selection.get() {
                        if let Some(board_idx) = self.board_selection.get() {
                            if let Some(board) = self.boards.get(board_idx) {
                                let board_sprints: Vec<_> = self
                                    .sprints
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, s)| s.board_id == board.id)
                                    .collect();
                                if let Some((actual_idx, _)) = board_sprints.get(sprint_idx) {
                                    self.active_sprint_index = Some(*actual_idx);
                                    self.active_board_index = Some(board_idx);
                                    if let Some(sprint) = self.sprints.get(*actual_idx) {
                                        self.populate_sprint_task_lists(sprint.id);
                                    }
                                    self.mode = AppMode::SprintDetail;
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Char('p') => {
                if self.board_focus == BoardFocus::Settings {
                    if let Some(board_idx) = self.board_selection.get() {
                        if let Some(board) = self.boards.get(board_idx) {
                            let current_prefix =
                                board.branch_prefix.clone().unwrap_or_else(String::new);
                            self.input.set(current_prefix);
                            self.mode = AppMode::SetBranchPrefix;
                        }
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
                self.mode = AppMode::BoardDetail;
                self.board_focus = BoardFocus::Sprints;
                self.active_sprint_index = None;
            }
            KeyCode::Char('a') => {
                self.handle_activate_sprint_key();
            }
            KeyCode::Char('c') => {
                self.handle_complete_sprint_key();
            }
            KeyCode::Char('o') => {
                let sort_idx = self.get_current_sort_field_selection_index();
                self.sort_field_selection.set(Some(sort_idx));
                self.mode = AppMode::OrderCards;
            }
            KeyCode::Char('O') => {
                if let Some(current_order) = self.current_sort_order {
                    let new_order = match current_order {
                        kanban_domain::SortOrder::Ascending => kanban_domain::SortOrder::Descending,
                        kanban_domain::SortOrder::Descending => kanban_domain::SortOrder::Ascending,
                    };
                    self.current_sort_order = Some(new_order);

                    if let Some(field) = self.current_sort_field {
                        self.apply_sort_to_sprint_lists(field, new_order);
                        tracing::info!("Toggled sort order to: {:?}", new_order);
                    }
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if let Some(sprint_idx) = self.active_sprint_index {
                    if let Some(sprint) = self.sprints.get(sprint_idx) {
                        if sprint.status == kanban_domain::SprintStatus::Completed {
                            self.sprint_task_panel = SprintTaskPanel::Uncompleted;
                        }
                    }
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if let Some(sprint_idx) = self.active_sprint_index {
                    if let Some(sprint) = self.sprints.get(sprint_idx) {
                        if sprint.status == kanban_domain::SprintStatus::Completed {
                            self.sprint_task_panel = SprintTaskPanel::Completed;
                        }
                    }
                }
            }
            _ => {
                let action = {
                    let active_component = match self.sprint_task_panel {
                        SprintTaskPanel::Uncompleted => &mut self.sprint_uncompleted_component,
                        SprintTaskPanel::Completed => &mut self.sprint_completed_component,
                    };
                    active_component.handle_key(key_code)
                };

                if let Some(action) = action {
                    use crate::card_list_component::CardListAction;

                    match action {
                        CardListAction::Select(card_id) => {
                            if let Some(card_idx) = self.cards.iter().position(|c| c.id == card_id)
                            {
                                self.active_card_index = Some(card_idx);
                                self.mode = AppMode::CardDetail;
                                self.card_focus = CardFocus::Title;
                            }
                        }
                        CardListAction::Edit(card_id) => {
                            if let Some(card_idx) = self.cards.iter().position(|c| c.id == card_id)
                            {
                                self.active_card_index = Some(card_idx);
                                self.mode = AppMode::CardDetail;
                                self.card_focus = CardFocus::Title;
                            }
                        }
                        CardListAction::Complete(card_id) => {
                            if let Some(card) = self.cards.iter_mut().find(|c| c.id == card_id) {
                                use kanban_domain::CardStatus;
                                let new_status = if card.status == CardStatus::Done {
                                    CardStatus::Todo
                                } else {
                                    CardStatus::Done
                                };
                                card.update_status(new_status);
                                tracing::info!("Card status updated to: {:?}", new_status);
                            }
                        }
                        CardListAction::TogglePriority(card_id) => {
                            if let Some(card_idx) = self.cards.iter().position(|c| c.id == card_id)
                            {
                                self.active_card_index = Some(card_idx);
                                let priority_idx = self.get_current_priority_selection_index();
                                self.priority_selection.set(Some(priority_idx));
                                self.mode = AppMode::SetCardPriority;
                            }
                        }
                        CardListAction::AssignSprint(card_id) => {
                            if let Some(card_idx) = self.cards.iter().position(|c| c.id == card_id)
                            {
                                self.active_card_index = Some(card_idx);
                                if let Some(board_idx) = self.active_board_index {
                                    if let Some(board) = self.boards.get(board_idx) {
                                        let sprint_count = self
                                            .sprints
                                            .iter()
                                            .filter(|s| s.board_id == board.id)
                                            .count();
                                        if sprint_count > 0 {
                                            let selection_idx =
                                                self.get_current_sprint_selection_index();
                                            self.sprint_assign_selection.set(Some(selection_idx));
                                            self.mode = AppMode::AssignCardToSprint;
                                        }
                                    }
                                }
                            }
                        }
                        CardListAction::ReassignSprint(card_id) => {
                            if let Some(card_idx) = self.cards.iter().position(|c| c.id == card_id)
                            {
                                self.active_card_index = Some(card_idx);
                                if let Some(board_idx) = self.active_board_index {
                                    if let Some(board) = self.boards.get(board_idx) {
                                        let sprint_count = self
                                            .sprints
                                            .iter()
                                            .filter(|s| s.board_id == board.id)
                                            .count();
                                        if sprint_count > 0 {
                                            let selection_idx =
                                                self.get_current_sprint_selection_index();
                                            self.sprint_assign_selection.set(Some(selection_idx));
                                            self.mode = AppMode::AssignCardToSprint;
                                        }
                                    }
                                }
                            }
                        }
                        CardListAction::Sort => {
                            let sort_idx = self.get_current_sort_field_selection_index();
                            self.sort_field_selection.set(Some(sort_idx));
                            self.mode = AppMode::OrderCards;
                        }
                        CardListAction::OrderCards => {
                            if let Some(current_order) = self.current_sort_order {
                                let new_order = match current_order {
                                    kanban_domain::SortOrder::Ascending => {
                                        kanban_domain::SortOrder::Descending
                                    }
                                    kanban_domain::SortOrder::Descending => {
                                        kanban_domain::SortOrder::Ascending
                                    }
                                };
                                self.current_sort_order = Some(new_order);

                                if let Some(field) = self.current_sort_field {
                                    self.apply_sort_to_sprint_lists(field, new_order);
                                    tracing::info!("Toggled sort order to: {:?}", new_order);
                                }
                            }
                        }
                        CardListAction::MoveColumn(card_id, is_right) => {
                            if let Some(card_idx) = self.cards.iter().position(|c| c.id == card_id)
                            {
                                if let Some(card) = self.cards.get_mut(card_idx) {
                                    if let Some(board_idx) = self.active_board_index {
                                        if let Some(board) = self.boards.get(board_idx) {
                                            let current_col = card.column_id;
                                            let mut columns: Vec<_> = self
                                                .columns
                                                .iter()
                                                .filter(|c| c.board_id == board.id)
                                                .collect();
                                            columns.sort_by_key(|col| col.position);

                                            if let Some(current_idx) =
                                                columns.iter().position(|c| c.id == current_col)
                                            {
                                                let new_idx = if is_right {
                                                    (current_idx + 1).min(columns.len() - 1)
                                                } else {
                                                    current_idx.saturating_sub(1)
                                                };

                                                if let Some(new_col) = columns.get(new_idx) {
                                                    let was_in_last =
                                                        current_idx == columns.len() - 1;
                                                    let moving_to_last =
                                                        new_idx == columns.len() - 1;

                                                    card.column_id = new_col.id;

                                                    // Update status based on movement
                                                    if !is_right
                                                        && was_in_last
                                                        && columns.len() > 1
                                                        && card.status
                                                            == kanban_domain::CardStatus::Done
                                                    {
                                                        // Moving left from last column: uncomplete
                                                        card.update_status(
                                                            kanban_domain::CardStatus::Todo,
                                                        );
                                                        tracing::info!(
                                                            "Moved card {} left from last column (marked as incomplete)",
                                                            card.title
                                                        );
                                                    } else if is_right
                                                        && moving_to_last
                                                        && columns.len() > 1
                                                        && card.status
                                                            != kanban_domain::CardStatus::Done
                                                    {
                                                        // Moving right to last column: complete
                                                        card.update_status(
                                                            kanban_domain::CardStatus::Done,
                                                        );
                                                        tracing::info!(
                                                            "Moved card {} to last column (marked as complete)",
                                                            card.title
                                                        );
                                                    } else {
                                                        let direction =
                                                            if is_right { "right" } else { "left" };
                                                        tracing::info!(
                                                            "Moved card {} to {}",
                                                            card.title,
                                                            direction
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        CardListAction::Create => {
                            self.mode = AppMode::CreateCard;
                            self.input.clear();
                        }
                        CardListAction::ToggleMultiSelect(card_id) => {
                            let component = match self.sprint_task_panel {
                                SprintTaskPanel::Uncompleted => {
                                    &mut self.sprint_uncompleted_component
                                }
                                SprintTaskPanel::Completed => &mut self.sprint_completed_component,
                            };
                            component.toggle_multi_select(card_id);
                        }
                        CardListAction::ClearMultiSelect => {
                            let component = match self.sprint_task_panel {
                                SprintTaskPanel::Uncompleted => {
                                    &mut self.sprint_uncompleted_component
                                }
                                SprintTaskPanel::Completed => &mut self.sprint_completed_component,
                            };
                            component.clear_multi_select();
                        }
                        CardListAction::SelectAll => {
                            let component = match self.sprint_task_panel {
                                SprintTaskPanel::Uncompleted => {
                                    &mut self.sprint_uncompleted_component
                                }
                                SprintTaskPanel::Completed => &mut self.sprint_completed_component,
                            };
                            component.select_all();
                        }
                    }
                }

                // Sync component selection back to CardList for rendering
                let (active_component, active_card_list) = match self.sprint_task_panel {
                    SprintTaskPanel::Uncompleted => (
                        &self.sprint_uncompleted_component,
                        &mut self.sprint_uncompleted_cards,
                    ),
                    SprintTaskPanel::Completed => (
                        &self.sprint_completed_component,
                        &mut self.sprint_completed_cards,
                    ),
                };
                active_card_list.set_selected_index(active_component.get_selected_index());
            }
        }
    }
}
