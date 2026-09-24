use crate::app::{App, AppMode};
use crossterm::event::KeyCode;
use kanban_domain::commands::{ColumnCommand, Command, UpdateColumn};
use kanban_domain::{ColumnUpdate, LoadState, MutationOperations, SortOrder};

const PRIORITY_COUNT: usize = 4;
const DEFAULT_STATUS_COUNT: usize = 5;

impl App {
    pub fn handle_import_board_popup(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
                self.dialog_input.import_selection.clear();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.dialog_input
                    .import_selection
                    .next(self.dialog_input.import_files.len());
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.dialog_input.import_selection.prev();
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(idx) = self.dialog_input.import_selection.get() {
                    if let Some(filename) = self.dialog_input.import_files.get(idx).cloned() {
                        if let Err(e) = self.import_board_from_file(&filename) {
                            tracing::error!("Failed to import board: {}", e);
                            self.set_error(format!("Failed to import board: {}", e));
                        }
                    }
                }
                self.pop_mode();
                self.dialog_input.import_selection.clear();
            }
            _ => {}
        }
    }

    pub fn handle_set_card_priority_popup(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.dialog_input.priority_selection.next(PRIORITY_COUNT);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.dialog_input.priority_selection.prev();
            }
            KeyCode::Enter => {
                if let Some(priority_idx) = self.dialog_input.priority_selection.get() {
                    if let Some(active_id) = self.selection.active_card_id {
                        if let Some(card) = self.model.card_by_id_state(active_id).loaded().copied()
                        {
                            use kanban_domain::{CardPriority, CardUpdate};
                            let priority = match priority_idx {
                                0 => CardPriority::Low,
                                1 => CardPriority::Medium,
                                2 => CardPriority::High,
                                3 => CardPriority::Critical,
                                _ => CardPriority::Medium,
                            };
                            let card_id = card.id;
                            let cmd = kanban_domain::commands::Command::Card(
                                kanban_domain::commands::CardCommand::Update(
                                    kanban_domain::commands::UpdateCard {
                                        card_id,
                                        updates: CardUpdate {
                                            priority: Some(priority),
                                            ..Default::default()
                                        },
                                    },
                                ),
                            );
                            match self.execute_command(cmd) {
                                Ok(inv) => self.resolve_after_command(inv),
                                Err(e) => {
                                    tracing::error!("Failed to update card priority: {}", e);
                                    self.set_error(format!(
                                        "Failed to update card priority: {}",
                                        e
                                    ));
                                }
                            }
                        }
                    }
                }
                self.pop_mode();
            }
            _ => {}
        }
    }

    pub fn handle_set_column_default_status_popup(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
                self.dialog_input.default_status_selection.clear();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.dialog_input
                    .default_status_selection
                    .next(DEFAULT_STATUS_COUNT);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.dialog_input.default_status_selection.prev();
            }
            KeyCode::Enter => {
                if let Some(idx) = self.dialog_input.default_status_selection.get() {
                    if let Some(status) =
                        kanban_view::selection_dialog::default_status_at_popup_index(idx)
                    {
                        if let Some(board) = self.active_board() {
                            let board_id = board.id;
                            if let Some(column_idx) =
                                self.dialog_input.column_list.get_selected_index()
                            {
                                match self.visible_board_columns(board_id) {
                                    LoadState::Loaded(columns) => {
                                        if let Some(column) = columns.get(column_idx) {
                                            let column_id = column.id;
                                            let cmd = Command::Column(ColumnCommand::Update(
                                                UpdateColumn {
                                                    column_id,
                                                    updates: ColumnUpdate {
                                                        default_status: Some(status),
                                                        ..Default::default()
                                                    },
                                                },
                                            ));
                                            match self.execute_command(cmd) {
                                                Ok(inv) => self.resolve_after_command(inv),
                                                Err(e) => {
                                                    tracing::error!(
                                                        "Failed to update column default status: {}",
                                                        e
                                                    );
                                                    self.set_error(format!(
                                                        "Failed to update column default status: {}",
                                                        e
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                    _ => {
                                        self.set_error("Columns are not loaded yet".to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                self.pop_mode();
                self.dialog_input.default_status_selection.clear();
            }
            _ => {}
        }
    }

    pub fn handle_set_multiple_cards_priority_popup(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
                self.dialog_input.priority_selection.clear();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.dialog_input.priority_selection.next(PRIORITY_COUNT);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.dialog_input.priority_selection.prev();
            }
            KeyCode::Enter => {
                if let Some(priority_idx) = self.dialog_input.priority_selection.get() {
                    use kanban_domain::{CardPriority, CardUpdate};
                    let priority = match priority_idx {
                        0 => CardPriority::Low,
                        1 => CardPriority::Medium,
                        2 => CardPriority::High,
                        3 => CardPriority::Critical,
                        _ => CardPriority::Medium,
                    };

                    let card_ids: Vec<uuid::Uuid> =
                        self.multi_select.selected_cards.iter().copied().collect();
                    let mut commands: Vec<kanban_domain::commands::Command> = Vec::new();

                    for card_id in &card_ids {
                        let cmd = kanban_domain::commands::Command::Card(
                            kanban_domain::commands::CardCommand::Update(
                                kanban_domain::commands::UpdateCard {
                                    card_id: *card_id,
                                    updates: CardUpdate {
                                        priority: Some(priority),
                                        ..Default::default()
                                    },
                                },
                            ),
                        );
                        commands.push(cmd);
                    }

                    if !commands.is_empty() {
                        match self.execute_commands_batch(commands) {
                            Ok(inv) => {
                                tracing::info!(
                                    "Set priority to {:?} for {} cards",
                                    priority,
                                    card_ids.len()
                                );
                                self.resolve_after_command(inv);
                            }
                            Err(e) => {
                                tracing::error!("Failed to update cards priority: {}", e);
                                self.set_error(format!("Failed to update cards priority: {}", e));
                            }
                        }
                    }

                    self.multi_select.selected_cards.clear();
                    self.multi_select.selection_mode_active = false;
                }
                self.pop_mode();
                self.dialog_input.priority_selection.clear();
            }
            _ => {}
        }
    }

    pub fn handle_order_cards_popup(&mut self, key_code: KeyCode) -> bool {
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
                self.filter.sort_field_selection.clear();
                false
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.filter
                    .sort_field_selection
                    .next(kanban_view::selection_dialog::SORT_FIELD_POPUP_ORDER.len());
                false
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.filter.sort_field_selection.prev();
                false
            }
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('a') | KeyCode::Char('d') => {
                if let Some(field_idx) = self.filter.sort_field_selection.get() {
                    let field =
                        match kanban_view::selection_dialog::sort_field_at_popup_index(field_idx) {
                            Some(f) => f,
                            None => return false,
                        };

                    let order = if self.filter.current_sort_field == Some(field)
                        && matches!(key_code, KeyCode::Enter | KeyCode::Char(' '))
                    {
                        match self.filter.current_sort_order {
                            Some(SortOrder::Ascending) => SortOrder::Descending,
                            Some(SortOrder::Descending) => SortOrder::Ascending,
                            None => SortOrder::Ascending,
                        }
                    } else {
                        match key_code {
                            KeyCode::Char('d') => SortOrder::Descending,
                            _ => SortOrder::Ascending,
                        }
                    };

                    self.filter.current_sort_field = Some(field);
                    self.filter.current_sort_order = Some(order);

                    if let Some(board_id) = self.active_board().map(|b| b.id) {
                        let cmd = kanban_domain::commands::Command::Board(
                            kanban_domain::commands::BoardCommand::SetTaskSort(
                                kanban_domain::commands::SetBoardTaskSort {
                                    board_id,
                                    field,
                                    order,
                                },
                            ),
                        );
                        match self.execute_command(cmd) {
                            Ok(inv) => self.resolve_after_command(inv),
                            Err(e) => {
                                tracing::error!("Failed to set board task sort: {}", e);
                                self.set_error(format!("Failed to set board task sort: {}", e));
                            }
                        }
                    }

                    let is_sprint_detail = self.selection.active_sprint_id.is_some();
                    self.pop_mode();
                    self.filter.sort_field_selection.clear();

                    tracing::info!("Sorting by {:?} ({:?})", field, order);

                    if is_sprint_detail {
                        self.apply_sort_to_sprint_lists(field, order);
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Key handling for the projects-panel sort field picker, the board-side
    /// analogue of [`handle_order_cards_popup`](Self::handle_order_cards_popup).
    /// `Enter`/`Space` on the already-active field toggles its order; `a`/`d`
    /// force ascending/descending. The chosen field/order is applied to
    /// whichever partition (live or archived) is currently active, via
    /// `apply_board_sort` — only the live choice is persisted to AppConfig.
    pub fn handle_order_boards_popup(&mut self, key_code: KeyCode) {
        use kanban_view::selection_dialog::{
            board_sort_field_at_popup_index, BOARD_SORT_FIELD_POPUP_ORDER,
        };
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
                self.filter.board_sort_field_selection.clear();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.filter
                    .board_sort_field_selection
                    .next(BOARD_SORT_FIELD_POPUP_ORDER.len());
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.filter.board_sort_field_selection.prev();
            }
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('a') | KeyCode::Char('d') => {
                if let Some(field_idx) = self.filter.board_sort_field_selection.get() {
                    let field = match board_sort_field_at_popup_index(field_idx) {
                        Some(f) => f,
                        None => return,
                    };

                    let want_archived = matches!(self.get_base_mode(), AppMode::ArchivedBoardsView);
                    let (current_field, current_order) = self.controller.board_sort(want_archived);
                    let order = if current_field == field
                        && matches!(key_code, KeyCode::Enter | KeyCode::Char(' '))
                    {
                        current_order.toggled()
                    } else {
                        match key_code {
                            KeyCode::Char('d') => SortOrder::Descending,
                            _ => SortOrder::Ascending,
                        }
                    };

                    self.apply_board_sort(field, order);
                    self.pop_mode();
                    self.filter.board_sort_field_selection.clear();
                    tracing::info!("Sorting projects by {:?} ({:?})", field, order);
                }
            }
            _ => {}
        }
    }

    pub fn handle_assign_card_to_sprint_popup(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
                self.dialog_input.assign_sprint_picker.clear();
            }
            KeyCode::Enter => {
                let active_card_id = match self.selection.active_card_id {
                    Some(id) => id,
                    None => {
                        self.pop_mode();
                        self.dialog_input.assign_sprint_picker.clear();
                        return;
                    }
                };
                let card_id = self
                    .model
                    .card_by_id_state(active_card_id)
                    .loaded()
                    .copied()
                    .map(|c| c.id);
                let Some(card_id) = card_id else {
                    self.set_error("Card is not loaded yet".to_string());
                    return;
                };
                let active_board_id = self
                    .selection
                    .active_board_id
                    .and_then(|id| self.model.board_by_id_state(id).loaded().copied())
                    .map(|b| b.id);
                let Some(active_board_id) = active_board_id else {
                    self.set_error("Board is not loaded yet".to_string());
                    return;
                };
                let picker = &self.dialog_input.assign_sprint_picker;
                let board_matches = picker.bound_board_id() == Some(active_board_id);
                let cmd = if !board_matches {
                    None
                } else if let Some(sprint_id) = picker.selected_sprint_id() {
                    Some(kanban_domain::commands::Command::Card(
                        kanban_domain::commands::CardCommand::AssignToSprint(
                            kanban_domain::commands::AssignCardsToSprint {
                                ids: vec![card_id],
                                sprint_id,
                            },
                        ),
                    ))
                } else if picker.explicitly_unassigned() {
                    Some(kanban_domain::commands::Command::Card(
                        kanban_domain::commands::CardCommand::UnassignFromSprint(
                            kanban_domain::commands::UnassignCardFromSprint {
                                card_id,
                                timestamp: chrono::Utc::now(),
                            },
                        ),
                    ))
                } else {
                    None
                };
                if let Some(cmd) = cmd {
                    match self.execute_commands_batch(vec![cmd]) {
                        Ok(inv) => self.resolve_after_command(inv),
                        Err(e) => {
                            tracing::error!("Failed to update card sprint: {}", e);
                            self.set_error(format!("Failed to update card sprint: {}", e));
                        }
                    }
                }
                self.pop_mode();
                self.dialog_input.assign_sprint_picker.clear();
            }
            _ => {
                if let Some(board) = self
                    .selection
                    .active_board_id
                    .and_then(|id| self.model.board_by_id_state(id).loaded().copied())
                {
                    let LoadState::Loaded(sprints) = self.board_sprints_view(board.id) else {
                        self.set_error("Sprints are not loaded yet".to_string());
                        return;
                    };
                    let now = chrono::Utc::now();
                    self.dialog_input
                        .assign_sprint_picker
                        .handle_key(key_code, &sprints, board, now);
                }
            }
        }
    }

    pub fn handle_assign_multiple_cards_to_sprint_popup(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
                self.dialog_input.assign_sprint_picker.clear();
                self.multi_select.selected_cards.clear();
                self.multi_select.selection_mode_active = false;
            }
            KeyCode::Enter => {
                let card_ids: Vec<uuid::Uuid> =
                    self.multi_select.selected_cards.iter().copied().collect();
                let active_board_id = self
                    .selection
                    .active_board_id
                    .and_then(|id| self.model.board_by_id_state(id).loaded().copied())
                    .map(|b| b.id);
                let Some(active_board_id) = active_board_id else {
                    self.set_error("Board is not loaded yet".to_string());
                    return;
                };
                let picker = &self.dialog_input.assign_sprint_picker;
                let board_matches = picker.bound_board_id() == Some(active_board_id);
                let cmds: Vec<kanban_domain::commands::Command> = if !board_matches {
                    Vec::new()
                } else if let Some(sprint_id) = picker.selected_sprint_id() {
                    vec![kanban_domain::commands::Command::Card(
                        kanban_domain::commands::CardCommand::AssignToSprint(
                            kanban_domain::commands::AssignCardsToSprint {
                                ids: card_ids.clone(),
                                sprint_id,
                            },
                        ),
                    )]
                } else if picker.explicitly_unassigned() {
                    card_ids
                        .iter()
                        .map(|card_id| {
                            kanban_domain::commands::Command::Card(
                                kanban_domain::commands::CardCommand::UnassignFromSprint(
                                    kanban_domain::commands::UnassignCardFromSprint {
                                        card_id: *card_id,
                                        timestamp: chrono::Utc::now(),
                                    },
                                ),
                            )
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                if !cmds.is_empty() {
                    match self.execute_commands_batch(cmds) {
                        Ok(inv) => self.resolve_after_command(inv),
                        Err(e) => {
                            tracing::error!("Failed to update cards' sprint: {}", e);
                            self.set_error(format!("Failed to update cards' sprint: {}", e));
                        }
                    }
                }
                self.pop_mode();
                self.dialog_input.assign_sprint_picker.clear();
                self.multi_select.selected_cards.clear();
                self.multi_select.selection_mode_active = false;
            }
            _ => {
                if let Some(board) = self
                    .selection
                    .active_board_id
                    .and_then(|id| self.model.board_by_id_state(id).loaded().copied())
                {
                    let LoadState::Loaded(sprints) = self.board_sprints_view(board.id) else {
                        self.set_error("Sprints are not loaded yet".to_string());
                        return;
                    };
                    let now = chrono::Utc::now();
                    self.dialog_input
                        .assign_sprint_picker
                        .handle_key(key_code, &sprints, board, now);
                }
            }
        }
    }

    pub fn handle_manage_parents_popup(&mut self, key_code: KeyCode) {
        self.handle_relationship_popup(key_code, true);
    }

    pub fn handle_manage_children_popup(&mut self, key_code: KeyCode) {
        self.handle_relationship_popup(key_code, false);
    }

    pub fn handle_carry_over_sprint_popup(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
                self.dialog_input.carry_over_sprint_selection.clear();
                self.dialog_input.carry_over_source_sprint_id = None;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(source_id) = self.dialog_input.carry_over_source_sprint_id {
                    match self.model.sprint_by_id_state(source_id) {
                        LoadState::Loaded(sprint) => {
                            let board_id = sprint.board_id;
                            match self.board_sprints_view(board_id) {
                                LoadState::Loaded(sprints) => {
                                    let count = sprints
                                        .iter()
                                        .filter(|s| {
                                            s.status == kanban_domain::SprintStatus::Planning
                                        })
                                        .count();
                                    self.dialog_input.carry_over_sprint_selection.next(count);
                                }
                                _ => self.set_error("Sprints are not loaded yet".to_string()),
                            }
                        }
                        LoadState::Missing => {}
                        _ => self.set_error("Sprint is not loaded yet".to_string()),
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.dialog_input.carry_over_sprint_selection.prev();
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(idx) = self.dialog_input.carry_over_sprint_selection.get() {
                    if let Some(source_id) = self.dialog_input.carry_over_source_sprint_id {
                        match self.model.sprint_by_id_state(source_id) {
                            LoadState::Loaded(sprint) => {
                                let board_id = sprint.board_id;
                                match self.board_sprints_view(board_id) {
                                    LoadState::Loaded(sprints) => {
                                        let planning_sprint_ids: Vec<uuid::Uuid> = sprints
                                            .iter()
                                            .filter(|s| {
                                                s.status == kanban_domain::SprintStatus::Planning
                                            })
                                            .map(|s| s.id)
                                            .collect();

                                        if let Some(&to_sprint_id) = planning_sprint_ids.get(idx) {
                                            let sprint_label = match self
                                                .model
                                                .sprint_by_id_state(to_sprint_id)
                                            {
                                                LoadState::Loaded(s) => match self
                                                    .model
                                                    .boards_state()
                                                {
                                                    LoadState::Loaded(boards) => boards
                                                        .iter()
                                                        .find(|b| b.id == board_id)
                                                        .and_then(|b| s.get_name(b))
                                                        .map(|n| n.to_string())
                                                        .unwrap_or_else(|| {
                                                            format!("Sprint {}", s.sprint_number)
                                                        }),
                                                    _ => format!("Sprint {}", s.sprint_number),
                                                },
                                                _ => "sprint".to_string(),
                                            };

                                            match self.ctx.carry_over_sprint_cards_impl(
                                                source_id,
                                                to_sprint_id,
                                            ) {
                                                Ok((count, inv)) => {
                                                    self.resolve_after_command(inv);
                                                    self.set_success(format!(
                                                        "Carried over {} card(s) to {}",
                                                        count, sprint_label
                                                    ));
                                                    self.populate_sprint_task_lists(source_id);
                                                }
                                                Err(e) => {
                                                    tracing::error!("Carry-over failed: {}", e);
                                                    self.set_error(format!(
                                                        "Carry-over failed: {}",
                                                        e
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                    _ => self.set_error("Sprints are not loaded yet".to_string()),
                                }
                            }
                            LoadState::Missing => {}
                            _ => self.set_error("Sprint is not loaded yet".to_string()),
                        }
                    }
                }
                self.pop_mode();
                self.dialog_input.carry_over_sprint_selection.clear();
                self.dialog_input.carry_over_source_sprint_id = None;
            }
            _ => {}
        }
    }

    fn filtered_relationship_card_ids(&self) -> Option<Vec<uuid::Uuid>> {
        if self.relationship.search.is_empty() {
            return Some(self.relationship.card_ids.clone());
        }
        if self.relationship.card_ids.iter().any(|id| {
            let state = self.model.card_by_id_state(*id);
            state.is_not_loaded() || state.is_failed()
        }) {
            return None;
        }
        let search_lower = self.relationship.search.to_lowercase();
        Some(
            self.relationship
                .card_ids
                .iter()
                .filter(|card_id| {
                    self.model
                        .card_by_id_state(**card_id)
                        .loaded()
                        .map(|c| c.title.to_lowercase().contains(&search_lower))
                        .unwrap_or(false)
                })
                .copied()
                .collect(),
        )
    }

    fn handle_relationship_popup(&mut self, key_code: KeyCode, is_parent_mode: bool) {
        // Handle search mode separately
        if self.relationship.search_active {
            match key_code {
                KeyCode::Esc => {
                    // Exit search mode but stay in dialog
                    self.relationship.search_active = false;
                }
                KeyCode::Enter => {
                    // Confirm search and exit search mode
                    self.relationship.search_active = false;
                }
                KeyCode::Backspace => {
                    self.relationship.search.pop();
                    if !self.update_relationship_selection_after_search() {
                        self.relationship.selection.clear();
                    }
                }
                KeyCode::Char(c) => {
                    self.relationship.search.push(c);
                    if !self.update_relationship_selection_after_search() {
                        self.relationship.search.pop();
                    }
                }
                _ => {}
            }
            return;
        }

        // Navigation mode
        match key_code {
            KeyCode::Esc => {
                self.pop_mode();
                self.relationship.card_ids.clear();
                self.relationship.selected.clear();
                self.relationship.selection.clear();
                self.relationship.search.clear();
                self.relationship.search_active = false;
            }
            KeyCode::Char('/') => {
                // Enter search mode
                self.relationship.search_active = true;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let Some(filtered_cards) = self.filtered_relationship_card_ids() else {
                    self.set_error("Cards are not loaded yet");
                    return;
                };
                self.relationship.selection.next(filtered_cards.len());
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.relationship.selection.prev();
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                let Some(filtered_cards) = self.filtered_relationship_card_ids() else {
                    self.set_error("Cards are not loaded yet");
                    return;
                };
                // Toggle relationship
                if let Some(idx) = self.relationship.selection.get() {
                    if let Some(selected_card_id) = filtered_cards.get(idx).copied() {
                        if let Some(active_id) = self.selection.active_card_id {
                            if let Some(current_card) =
                                self.model.card_by_id_state(active_id).loaded().copied()
                            {
                                let current_card_id = current_card.id;

                                let (child_id, parent_id) = if is_parent_mode {
                                    (current_card_id, selected_card_id)
                                } else {
                                    (selected_card_id, current_card_id)
                                };
                                let was_selected =
                                    self.relationship.selected.contains(&selected_card_id);
                                let result = if was_selected {
                                    self.ctx.detach_children_impl(parent_id, vec![child_id])
                                } else {
                                    self.ctx.attach_children_impl(parent_id, vec![child_id])
                                };
                                match result {
                                    Ok(inv) => {
                                        self.resolve_after_command(inv);
                                        if was_selected {
                                            self.relationship.selected.remove(&selected_card_id);
                                        } else {
                                            self.relationship.selected.insert(selected_card_id);
                                        }
                                    }
                                    Err(e) => {
                                        // Surface the rejection to the user
                                        // (cycle / self-ref / duplicate / unknown
                                        // card). Without this the popup would
                                        // look like a silent no-op.
                                        self.set_error(format!(
                                            "Failed to toggle relationship: {e}"
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn update_relationship_selection_after_search(&mut self) -> bool {
        let Some(filtered) = self.filtered_relationship_card_ids() else {
            self.set_error("Cards are not loaded yet");
            return false;
        };
        if filtered.is_empty() {
            self.relationship.selection.clear();
        } else {
            self.relationship.selection.set(Some(0));
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{AppMode, DialogMode};
    use crate::test_helpers::{
        load_with_card_order, setup_reload_resort_fixture, ReloadResortFixture,
    };
    use crate::App;
    use crossterm::event::KeyCode;
    use kanban_domain::{
        CardPriority, CreateCardOptions, DerivedProjections, EntityIds, Invalidation,
        KanbanOperations, Snapshot, SprintStatus, SprintUpdate,
    };
    use std::collections::HashSet;

    fn assign_dialog_fixture(app: &mut App) -> (ReloadResortFixture, uuid::Uuid) {
        let fx = setup_reload_resort_fixture(app);
        let sprint = app.ctx.create_sprint(fx.board_id, None, None).unwrap();
        load_with_card_order(app, &[fx.a_id, fx.p_id, fx.b_id, fx.c_id, fx.d_id]);

        let sprints = app
            .model
            .board_sprints_state(fx.board_id)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .to_vec();
        let board = app
            .model
            .boards_state()
            .loaded_or_empty()
            .iter()
            .find(|b| b.id == fx.board_id)
            .cloned()
            .expect("board exists");
        app.dialog_input
            .assign_sprint_picker
            .reset_for_card_assignment(Some(sprint.id), &sprints, &board, chrono::Utc::now());

        (fx, sprint.id)
    }

    #[test]
    fn test_handle_set_card_priority_popup_after_reload_resort_updates_originally_selected_card_priority(
    ) {
        let mut app = App::test_default();
        let fx = setup_reload_resort_fixture(&mut app);

        app.dialog_input.priority_selection.set(Some(3));
        app.handle_set_card_priority_popup(KeyCode::Enter);

        let cards = app.ctx.data_store().list_all_cards().unwrap();
        let a_card = cards.iter().find(|c| c.id == fx.a_id).expect("A exists");
        let p_card = cards.iter().find(|c| c.id == fx.p_id).expect("P exists");
        assert_eq!(
            a_card.priority,
            CardPriority::Critical,
            "priority popup must update A (the active card by id), not the wrong card at A's stale index"
        );
        assert_ne!(
            p_card.priority,
            CardPriority::Critical,
            "priority popup must leave P unchanged when A is active"
        );
    }

    #[test]
    fn test_handle_assign_card_to_sprint_popup_after_reload_resort_acts_on_originally_selected_card(
    ) {
        let mut app = App::test_default();
        let fx = setup_reload_resort_fixture(&mut app);

        let sprint = app.ctx.create_sprint(fx.board_id, None, None).unwrap();
        load_with_card_order(&mut app, &[fx.a_id, fx.p_id, fx.b_id, fx.c_id, fx.d_id]);

        // Prime the picker with the target sprint pre-checked.
        let sprints = app
            .model
            .board_sprints_state(fx.board_id)
            .loaded()
            .copied()
            .unwrap_or(&[])
            .to_vec();
        let board = app
            .model
            .boards_state()
            .loaded_or_empty()
            .iter()
            .find(|b| b.id == fx.board_id)
            .cloned()
            .expect("board exists");
        app.dialog_input
            .assign_sprint_picker
            .reset_for_card_assignment(Some(sprint.id), &sprints, &board, chrono::Utc::now());

        app.handle_assign_card_to_sprint_popup(KeyCode::Enter);

        let cards = app.ctx.data_store().list_all_cards().unwrap();
        let a_card = cards.iter().find(|c| c.id == fx.a_id).expect("A exists");
        let p_card = cards.iter().find(|c| c.id == fx.p_id).expect("P exists");
        assert_eq!(
            a_card.sprint_id,
            Some(sprint.id),
            "sprint-assign popup must assign A (the active card by id), not the wrong card at A's stale index"
        );
        assert_eq!(
            p_card.sprint_id, None,
            "sprint-assign popup must leave P unassigned when A is active"
        );
    }

    #[test]
    fn test_handle_manage_parents_popup_toggle_after_reload_resort_attaches_to_originally_selected_card(
    ) {
        let mut app = App::test_default();
        let fx = setup_reload_resort_fixture(&mut app);

        app.relationship.card_ids = vec![fx.p_id, fx.b_id, fx.c_id];
        app.relationship.selected = HashSet::from_iter(vec![fx.p_id]);
        app.relationship.selection.set(Some(1));

        app.handle_manage_parents_popup(KeyCode::Enter);

        let graph = app.ctx.data_store().get_graph().unwrap();
        let a_parents: HashSet<_> = graph.parents(fx.a_id).into_iter().collect();
        let p_parents: HashSet<_> = graph.parents(fx.p_id).into_iter().collect();
        assert!(
            a_parents.contains(&fx.b_id),
            "manage_parents toggle must attach B as a parent of A (the active card by id), not as a parent of the wrong card at A's stale index"
        );
        assert!(
            !p_parents.contains(&fx.b_id),
            "manage_parents toggle must not attach B as a parent of P when A is the active card"
        );
    }

    fn refresh(app: &mut App) {
        let snap = Snapshot {
            archived_boards: Vec::new(),
            boards: app.ctx.data_store().list_boards().unwrap(),
            columns: app.ctx.data_store().list_all_columns().unwrap(),
            cards: app.ctx.data_store().list_all_cards().unwrap(),
            archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
            sprints: app.ctx.data_store().list_all_sprints().unwrap(),
            graph: app.ctx.data_store().get_graph().unwrap(),
            prefixes: Vec::new(),
        };
        app.load_snapshot(snap);
    }

    #[test]
    fn test_carry_over_sprint_cards_label_with_a_not_loaded_boards_tier_falls_back_without_a_banner(
    ) {
        let mut app = App::test_default();
        let board = app.ctx.create_board("Board".into(), None).unwrap();
        let source = app
            .ctx
            .create_sprint(board.id, None, Some("Source Sprint".into()))
            .unwrap();
        app.ctx
            .update_sprint(
                source.id,
                SprintUpdate {
                    status: Some(SprintStatus::Completed),
                    ..Default::default()
                },
            )
            .unwrap();
        let target = app
            .ctx
            .create_sprint(board.id, None, Some("Target Sprint".into()))
            .unwrap();
        refresh(&mut app);

        app.dialog_input.carry_over_source_sprint_id = Some(source.id);
        app.dialog_input.carry_over_sprint_selection.set(Some(0));

        let _ = app
            .model
            .invalidate(Invalidation::Entities(EntityIds::boards([
                uuid::Uuid::new_v4(),
            ])));

        app.handle_carry_over_sprint_popup(KeyCode::Enter);

        let banner = app
            .ui_state
            .banner
            .as_ref()
            .expect("carry-over succeeded so a success banner must be set");
        assert!(
            !banner.message.to_lowercase().contains("fail"),
            "must not report failure once the mutation already succeeded, got: {}",
            banner.message
        );
        assert!(
            banner
                .message
                .contains(&format!("Sprint {}", target.sprint_number)),
            "with the boards tier not loaded, the label must fall back to 'Sprint {{number}}' instead of the board-resolved name, got: {}",
            banner.message
        );
    }

    fn seed_relationship_dialog(app: &mut App) -> (uuid::Uuid, uuid::Uuid) {
        let board = app.ctx.create_board("Board".into(), None).unwrap();
        let column = app
            .ctx
            .create_column(board.id, "Todo".into(), None)
            .unwrap();
        let card = app
            .ctx
            .create_card(
                board.id,
                column.id,
                "Findable".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        refresh(app);
        app.relationship.card_ids = vec![card.id];
        (board.id, card.id)
    }

    #[test]
    fn test_relationship_search_with_not_loaded_cards_tier_banners_and_preserves_list() {
        let mut app = App::test_default();
        let (_board_id, _card_id) = seed_relationship_dialog(&mut app);
        app.relationship.selection.set(Some(0));

        let _ = app
            .model
            .invalidate(Invalidation::Entities(EntityIds::cards([
                uuid::Uuid::new_v4(),
            ])));

        app.relationship.search_active = true;
        app.handle_manage_parents_popup(KeyCode::Char('f'));

        assert_eq!(
            app.relationship.selection.get(),
            Some(0),
            "a NotLoaded cards tier must not clear a staged selection"
        );
        assert_eq!(
            app.relationship.card_ids.len(),
            1,
            "a NotLoaded cards tier must not empty the candidate list"
        );
        let banner = app
            .ui_state
            .banner
            .as_ref()
            .expect("a NotLoaded cards tier must banner rather than silently show no matches");
        assert!(
            banner.message.to_lowercase().contains("not loaded"),
            "banner should explain the cards tier is not loaded, got: {}",
            banner.message
        );
    }

    #[test]
    fn test_relationship_search_with_failed_per_id_entry_for_an_unresolvable_id_banners() {
        let mut app = App::test_default();
        let (_board_id, card_id) = seed_relationship_dialog(&mut app);
        let unresolvable_id = uuid::Uuid::new_v4();
        app.relationship.card_ids = vec![card_id, unresolvable_id];
        app.relationship.selection.set(Some(0));

        let changed = app.model.apply_resolved(kanban_domain::Resolved {
            cards: kanban_domain::resolved::Collection {
                by_id: [(
                    unresolvable_id,
                    kanban_domain::LoadState::Failed(std::sync::Arc::new(
                        kanban_domain::KanbanError::unsupported("boom"),
                    )),
                )]
                .into(),
                ..Default::default()
            },
            ..Default::default()
        });
        app.controller.resync(&app.model, changed);

        app.relationship.search_active = true;
        app.handle_manage_parents_popup(KeyCode::Char('f'));

        assert_eq!(
            app.relationship.selection.get(),
            Some(0),
            "a Failed flat cards tier must not clear a staged selection"
        );
        assert_eq!(
            app.relationship.card_ids.len(),
            2,
            "a Failed flat cards tier must not empty the candidate list"
        );
        let banner =
            app.ui_state.banner.as_ref().expect(
                "a Failed flat cards tier must banner rather than silently show no matches",
            );
        assert!(
            banner.message.to_lowercase().contains("not loaded"),
            "banner should explain the cards tier is not loaded, got: {}",
            banner.message
        );
    }

    #[test]
    fn test_relationship_search_char_with_a_not_loaded_cards_tier_leaves_the_buffer_unchanged() {
        let mut app = App::test_default();
        let (_board_id, _card_id) = seed_relationship_dialog(&mut app);
        app.relationship.selection.set(Some(0));

        let _ = app
            .model
            .invalidate(Invalidation::Entities(EntityIds::cards([
                uuid::Uuid::new_v4(),
            ])));

        app.relationship.search_active = true;
        app.handle_manage_parents_popup(KeyCode::Char('f'));

        assert!(
            app.relationship.search.is_empty(),
            "a declined recompute must not leave the typed char in the buffer, got: {:?}",
            app.relationship.search
        );
        let banner = app
            .ui_state
            .banner
            .as_ref()
            .expect("a declined recompute must still banner");
        assert!(
            banner.message.to_lowercase().contains("not loaded"),
            "banner should explain the cards tier is not loaded, got: {}",
            banner.message
        );
    }

    #[test]
    fn test_relationship_search_backspace_with_a_failed_entry_shrinks_and_drops_selection() {
        let mut app = App::test_default();
        let (_board_id, card_id) = seed_relationship_dialog(&mut app);
        let unresolvable_id = uuid::Uuid::new_v4();
        app.relationship.card_ids = vec![card_id, unresolvable_id];
        app.relationship.selection.set(Some(0));
        app.relationship.search = "ab".to_string();

        let changed = app.model.apply_resolved(kanban_domain::Resolved {
            cards: kanban_domain::resolved::Collection {
                by_id: [(
                    unresolvable_id,
                    kanban_domain::LoadState::Failed(std::sync::Arc::new(
                        kanban_domain::KanbanError::unsupported("boom"),
                    )),
                )]
                .into(),
                ..Default::default()
            },
            ..Default::default()
        });
        app.controller.resync(&app.model, changed);

        app.relationship.search_active = true;
        app.handle_manage_parents_popup(KeyCode::Backspace);

        assert_eq!(
            app.relationship.search, "a",
            "backspace always shrinks the buffer, so the filter stays clearable"
        );
        assert!(
            app.relationship.selection.get().is_none(),
            "a declined recompute must drop the selection rather than leave a stale index"
        );
        assert!(
            app.ui_state.banner.is_some(),
            "a declined recompute must tell the user why"
        );
    }

    #[test]
    fn test_assign_sprint_enter_with_not_loaded_board_tier_keeps_dialog_open_and_banners() {
        let mut app = App::test_default();
        let (fx, sprint_id) = assign_dialog_fixture(&mut app);
        app.mode = AppMode::Dialog(DialogMode::AssignCardToSprint);

        let _ = app
            .model
            .invalidate(Invalidation::Entities(EntityIds::boards([
                uuid::Uuid::new_v4(),
            ])));

        app.handle_assign_card_to_sprint_popup(KeyCode::Enter);

        assert_eq!(
            app.mode,
            AppMode::Dialog(DialogMode::AssignCardToSprint),
            "a not-loaded board tier must keep the dialog open"
        );
        assert_eq!(
            app.dialog_input.assign_sprint_picker.selected_sprint_id(),
            Some(sprint_id),
            "the staged sprint pick must survive"
        );
        assert_eq!(
            app.dialog_input.assign_sprint_picker.bound_board_id(),
            Some(fx.board_id),
            "the picker must not be cleared"
        );
        let banner = app
            .ui_state
            .banner
            .as_ref()
            .expect("a not-loaded board tier must banner instead of failing silently");
        assert!(
            banner.message.to_lowercase().contains("not loaded"),
            "banner should explain the tier is not loaded, got: {}",
            banner.message
        );
        let cards = app.ctx.data_store().list_all_cards().unwrap();
        let a_card = cards.iter().find(|c| c.id == fx.a_id).expect("A exists");
        assert_eq!(
            a_card.sprint_id, None,
            "declining on an unloaded tier must not mutate the store"
        );
    }

    #[test]
    fn test_assign_sprint_enter_with_not_loaded_card_tier_banners_instead_of_dead_modal() {
        let mut app = App::test_default();
        let (_fx, sprint_id) = assign_dialog_fixture(&mut app);
        app.mode = AppMode::Dialog(DialogMode::AssignCardToSprint);

        let _ = app
            .model
            .invalidate(Invalidation::Entities(EntityIds::cards([
                uuid::Uuid::new_v4(),
            ])));

        app.handle_assign_card_to_sprint_popup(KeyCode::Enter);

        let banner = app
            .ui_state
            .banner
            .as_ref()
            .expect("a not-loaded card tier must banner instead of leaving a dead modal");
        assert!(
            banner.message.to_lowercase().contains("not loaded"),
            "banner should explain the tier is not loaded, got: {}",
            banner.message
        );
        assert_eq!(
            app.mode,
            AppMode::Dialog(DialogMode::AssignCardToSprint),
            "pin: the dialog must still be open (not a new close-on-miss path)"
        );
        assert_eq!(
            app.dialog_input.assign_sprint_picker.selected_sprint_id(),
            Some(sprint_id),
            "pin: the staged sprint pick must survive"
        );
    }

    #[test]
    fn test_assign_multiple_enter_with_not_loaded_board_tier_keeps_dialog_and_selection() {
        let mut app = App::test_default();
        let (fx, sprint_id) = assign_dialog_fixture(&mut app);
        app.multi_select.selected_cards = HashSet::from_iter([fx.a_id, fx.b_id]);
        app.multi_select.selection_mode_active = true;
        app.mode = AppMode::Dialog(DialogMode::AssignMultipleCardsToSprint);

        let _ = app
            .model
            .invalidate(Invalidation::Entities(EntityIds::boards([
                uuid::Uuid::new_v4(),
            ])));

        app.handle_assign_multiple_cards_to_sprint_popup(KeyCode::Enter);

        assert_eq!(
            app.mode,
            AppMode::Dialog(DialogMode::AssignMultipleCardsToSprint),
            "a not-loaded board tier must keep the dialog open"
        );
        assert_eq!(
            app.multi_select.selected_cards,
            HashSet::from_iter([fx.a_id, fx.b_id]),
            "the multi-selection must survive"
        );
        assert!(
            app.multi_select.selection_mode_active,
            "selection mode must stay active"
        );
        assert_eq!(
            app.dialog_input.assign_sprint_picker.selected_sprint_id(),
            Some(sprint_id),
            "the staged sprint pick must survive"
        );
        assert!(
            app.ui_state.banner.is_some(),
            "a not-loaded board tier must banner instead of failing silently"
        );
    }
}
