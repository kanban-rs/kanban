use crate::app::{App, AppMode, CardField, DialogMode, Focus};
use crate::card_list::CardListId;
use crate::events::EventHandler;
use kanban_domain::commands::{
    BoardCommand, CardCommand, Command, CreateCard, RestoreCard, SetBoardTaskSort, UpdateCard,
};
use kanban_domain::{ArchivedCard, CardStatus, CardUpdate, KanbanOperations};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

impl App {
    pub fn handle_create_card_key(&mut self) {
        if self.focus.active == Focus::Cards && self.active_board().is_some() {
            if let Some(board) = self.active_board().cloned() {
                self.dialog_input.create_card_sprint_picker.reset_for_board(
                    self.model.sprints(),
                    &board,
                    chrono::Utc::now(),
                );
            }
            self.open_dialog(DialogMode::CreateCard);
            self.input.clear();
        }
    }

    fn get_focused_column_id(&mut self) -> Option<uuid::Uuid> {
        if let Some(task_list) = self.view.strategy.get_active_task_list() {
            if let CardListId::Column(column_id) = task_list.id {
                return Some(column_id);
            }
        }
        None
    }

    pub fn handle_toggle_card_completion(&mut self) {
        if self.focus.active != Focus::Cards {
            return;
        }

        if !self.multi_select.selected_cards.is_empty() {
            self.toggle_selected_cards_completion();
        } else {
            self.toggle_card_completion();
        }
    }

    pub fn handle_card_selection_toggle(&mut self) {
        if self.focus.active == Focus::Cards {
            if self.multi_select.selection_mode_active {
                // Exit selection mode (keep selections)
                self.multi_select.selection_mode_active = false;
            } else {
                // Enter selection mode and select current card
                self.multi_select.selection_mode_active = true;
                if let Some(card) = self.get_selected_card_in_context() {
                    self.multi_select.selected_cards.insert(card.id);
                }
            }
        }
    }

    pub fn handle_clear_card_selection(&mut self) {
        self.multi_select.selected_cards.clear();
    }

    pub fn handle_select_all_cards_in_view(&mut self) {
        if self.focus.active != Focus::Cards {
            return;
        }

        if let Some(task_list) = self.view.strategy.get_active_task_list() {
            for card_id in &task_list.cards {
                self.multi_select.selected_cards.insert(*card_id);
            }
            if !task_list.cards.is_empty() {
                self.multi_select.selection_mode_active = true;
            }
        }
    }

    pub fn handle_set_card_priority_key(&mut self) {
        if self.focus.active != Focus::Cards {
            return;
        }
        let Some(card_id) = self.get_selected_card_id() else {
            return;
        };
        if self.activate_card(card_id) {
            let priority_idx = self.get_current_priority_selection_index();
            self.dialog_input.priority_selection.set(Some(priority_idx));
            self.open_dialog(DialogMode::SetCardPriority);
        }
    }

    pub fn handle_set_selected_cards_priority(&mut self) {
        if self.focus.active != Focus::Cards || self.multi_select.selected_cards.is_empty() {
            return;
        }

        self.dialog_input.priority_selection.set(Some(0));
        self.open_dialog(DialogMode::SetMultipleCardsPriority);
    }

    pub fn handle_assign_to_sprint_key(&mut self) {
        if self.focus.active != Focus::Cards {
            return;
        }

        if !self.multi_select.selected_cards.is_empty() {
            if let Some(board) = self.active_board().cloned() {
                self.dialog_input
                    .assign_sprint_picker
                    .reset_for_bulk_card_assignment(
                        self.model.sprints(),
                        &board,
                        chrono::Utc::now(),
                    );
            }
            self.open_dialog(DialogMode::AssignMultipleCardsToSprint);
        } else if self.get_selected_card_id().is_some() {
            let board_id = match self.active_board() {
                Some(b) => b.id,
                None => return,
            };
            let now = chrono::Utc::now();
            let has_assignable = {
                let sprints = self.model.sprints();
                let entries =
                    crate::components::sprint_assign_list::build_entries(sprints, board_id, now);
                entries.iter().any(|e| {
                    matches!(
                        e,
                        crate::components::sprint_assign_list::SprintAssignEntry::ActiveOrPlanned(
                            _
                        ) | crate::components::sprint_assign_list::SprintAssignEntry::Completed(_)
                            | crate::components::sprint_assign_list::SprintAssignEntry::Ended(_)
                    )
                })
            };
            if !has_assignable {
                return;
            }
            if let Some(selected_card) = self.get_selected_card_in_context() {
                self.set_active_card_or_clear(selected_card.id);
            }
            let current_sprint_id = self
                .selection
                .active_card_id
                .and_then(|id| self.model.card_by_id(id))
                .and_then(|c| c.sprint_id);
            // Re-borrow board after the &mut self call above.
            if let Some(board) = self.active_board().cloned() {
                self.dialog_input
                    .assign_sprint_picker
                    .reset_for_card_assignment(
                        current_sprint_id,
                        self.model.sprints(),
                        &board,
                        now,
                    );
            }
            self.open_dialog(DialogMode::AssignCardToSprint);
        }
    }

    pub fn handle_order_cards_key(&mut self) {
        if self.focus.active == Focus::Cards && self.selection.active_board_id.is_some() {
            let sort_idx = self.get_current_sort_field_selection_index();
            self.filter.sort_field_selection.set(Some(sort_idx));
            self.open_dialog(DialogMode::OrderCards);
        }
    }

    pub fn handle_toggle_sort_order_key(&mut self) {
        if self.focus.active == Focus::Cards && self.selection.active_board_id.is_some() {
            if let Some(current_order) = self.filter.current_sort_order {
                let new_order = current_order.toggled();
                self.filter.current_sort_order = Some(new_order);

                if let Some(board_id) = self.active_board().map(|b| b.id) {
                    if let Some(field) = self.filter.current_sort_field {
                        let cmd = Command::Board(BoardCommand::SetTaskSort(SetBoardTaskSort {
                            board_id,
                            field,
                            order: new_order,
                        }));

                        if let Err(e) = self.execute_command(cmd) {
                            tracing::error!("Failed to set board task sort: {}", e);
                            self.set_error(format!("Failed to set board task sort: {}", e));
                            return;
                        }
                    }
                }

                tracing::info!("Toggled sort order to: {:?}", new_order);
            }
        }
    }

    pub fn handle_toggle_hide_assigned(&mut self) {
        if self.focus.active == Focus::Cards && self.selection.active_board_id.is_some() {
            self.filter.hide_assigned_cards = !self.filter.hide_assigned_cards;
            let status = if self.filter.hide_assigned_cards {
                "enabled"
            } else {
                "disabled"
            };
            tracing::info!("Hide assigned cards: {}", status);
        }
    }

    pub fn handle_toggle_sprint_filter(&mut self) {
        if self.focus.active == Focus::Cards && self.selection.active_board_id.is_some() {
            if let Some(active_sprint_id) = self.active_board().and_then(|b| b.active_sprint_id) {
                if self
                    .filter
                    .active_sprint_filters
                    .contains(&active_sprint_id)
                {
                    self.filter.active_sprint_filters.remove(&active_sprint_id);
                    tracing::info!("Disabled sprint filter - showing all cards");
                } else {
                    self.filter.active_sprint_filters.clear();
                    self.filter.active_sprint_filters.insert(active_sprint_id);
                    tracing::info!("Enabled sprint filter - showing active sprint only");
                }
            } else {
                let msg = "No active sprint set for filtering";
                tracing::warn!("{}", msg);
                self.set_error(msg);
            }
        }
    }

    pub fn handle_edit_card_key(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
    ) -> bool {
        let mut should_restart = false;
        if self.focus.active == Focus::Cards {
            if let Some(selected_card) = self.get_selected_card_in_context() {
                self.set_active_card_or_clear(selected_card.id);

                if let Err(e) =
                    self.edit_card_field(terminal, event_handler, CardField::Description)
                {
                    tracing::error!("Failed to edit card description: {}", e);
                    self.set_error(format!("Failed to edit card description: {}", e));
                }
                should_restart = true;
            }
        }
        should_restart
    }

    fn toggle_card_completion(&mut self) {
        if let Some(card) = self.get_selected_card_in_context() {
            let card_id = card.id;
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
                self.set_error(format!("Failed to toggle card completion: {}", e));
                return;
            }

            // Refresh the view-layer task list before selecting so column lists are current.
            self.prepare_frame();
            self.select_card_by_id(card_id);
        }
    }

    fn toggle_selected_cards_completion(&mut self) {
        let card_ids: Vec<uuid::Uuid> = self.multi_select.selected_cards.iter().copied().collect();
        let first_card_id = card_ids.first().copied();

        let updates: Vec<(uuid::Uuid, CardUpdate)> = card_ids
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

        let toggled_count = updates.len();
        if !updates.is_empty() {
            if let Err(e) = self.ctx.update_cards(updates) {
                tracing::error!("Failed to toggle card completion: {}", e);
                self.set_error(format!("Failed to toggle card completion: {}", e));
                return;
            }
        }

        tracing::info!("Toggled {} cards completion status", toggled_count);
        self.multi_select.selected_cards.clear();
        self.multi_select.selection_mode_active = false;
        if let Some(card_id) = first_card_id {
            // Refresh the view-layer task list before selecting so column lists are current.
            self.prepare_frame();
            self.select_card_by_id(card_id);
        }
    }

    pub fn create_card(&mut self) {
        if let Some(board_id) = self.selection.active_board_id {
            let focused_col_id = self.get_focused_column_id();
            let board_info = self
                .model
                .board_by_id(board_id)
                .map(|b| (b.id, b.card_counter));

            if let Some((bid, card_number)) = board_info {
                let target_column_id = if let Some(focused_col_id) = focused_col_id {
                    Some(focused_col_id)
                } else {
                    self.model
                        .columns()
                        .iter()
                        .find(|col| col.board_id == bid)
                        .map(|col| col.id)
                };

                let column = if let Some(col_id) = target_column_id {
                    self.model
                        .columns()
                        .iter()
                        .find(|col| col.id == col_id)
                        .cloned()
                } else {
                    None
                };

                let column = match column {
                    Some(col) => col,
                    None => match self.ctx.create_column(bid, "Todo".to_string(), Some(0)) {
                        Ok(col) => col,
                        Err(e) => {
                            tracing::error!("Failed to create column: {}", e);
                            self.set_error(format!("Failed to create column: {}", e));
                            return;
                        }
                    },
                };

                let cards = self.model.cards();
                let position =
                    kanban_domain::card_lifecycle::next_position_in_column(cards, column.id);

                let columns = self.model.columns();
                let mark_as_complete = self
                    .model
                    .board_by_id(board_id)
                    .map(|board| {
                        kanban_domain::card_lifecycle::should_auto_complete_new_card(
                            column.id, board, columns,
                        )
                    })
                    .unwrap_or(false);

                let now = chrono::Utc::now();
                let sprint_id = self
                    .dialog_input
                    .create_card_sprint_picker
                    .selected_sprint_id_for(bid);
                let card_id = uuid::Uuid::new_v4();
                let mut commands: Vec<Command> =
                    vec![Command::Card(CardCommand::Create(CreateCard {
                        id: card_id,
                        card_number,
                        board_id: bid,
                        column_id: column.id,
                        title: self.input.as_str().to_string(),
                        position,
                        options: kanban_domain::CreateCardOptions {
                            sprint_id,
                            ..Default::default()
                        },
                        timestamp: now,
                    }))];

                if mark_as_complete {
                    commands.push(Command::Card(CardCommand::Update(UpdateCard {
                        card_id,
                        updates: CardUpdate {
                            status: Some(CardStatus::Done),
                            ..Default::default()
                        },
                    })));
                }

                // Single batch so a single undo reverses the whole
                // "create card" action even when auto-complete fires.
                if let Err(e) = self.execute_commands_batch(commands) {
                    tracing::error!("Failed to create card: {}", e);
                    self.set_error(format!("Failed to create card: {}", e));
                    return;
                }

                // Refresh the view-layer task list so the new card's ID is
                // present before we try to select it.
                self.prepare_frame();
                self.select_card_by_id(card_id);
            }
        }
    }

    pub fn handle_move_card_left(&mut self) {
        self.handle_move_card(kanban_domain::card_lifecycle::MoveDirection::Left);
    }

    pub fn handle_move_card_right(&mut self) {
        self.handle_move_card(kanban_domain::card_lifecycle::MoveDirection::Right);
    }

    fn handle_move_card(&mut self, direction: kanban_domain::card_lifecycle::MoveDirection) {
        if self.focus.active != Focus::Cards {
            return;
        }

        if !self.multi_select.selected_cards.is_empty() {
            self.move_selected_cards(direction);
            return;
        }

        if let Some(card) = self.get_selected_card_in_context() {
            let board = match self.active_board() {
                Some(b) => b,
                None => return,
            };

            // Use the pure helper only to resolve the target column for the
            // given direction; the service handles any status sync.
            let columns = self.model.columns();
            let cards = self.model.cards();
            let move_result = kanban_domain::card_lifecycle::compute_card_column_move(
                &card, board, columns, cards, direction,
            );
            let move_result = match move_result {
                Some(r) => r,
                None => return,
            };

            let card_id = card.id;
            if let Err(e) = self
                .ctx
                .move_card(card_id, move_result.target_column_id, None)
            {
                let dir = match direction {
                    kanban_domain::card_lifecycle::MoveDirection::Left => "left",
                    kanban_domain::card_lifecycle::MoveDirection::Right => "right",
                };
                tracing::error!("Failed to move card {}: {}", dir, e);
                self.set_error(format!("Failed to move card {}: {}", dir, e));
                return;
            }

            match direction {
                kanban_domain::card_lifecycle::MoveDirection::Right => {
                    self.view.strategy.navigate_right(false);
                }
                kanban_domain::card_lifecycle::MoveDirection::Left => {
                    self.view.strategy.navigate_left(false);
                }
            }
            if self.is_kanban_view() {
                if let Some(current_col_idx) = self.dialog_input.column_selection.get() {
                    match direction {
                        kanban_domain::card_lifecycle::MoveDirection::Left => {
                            if current_col_idx > 0 {
                                self.dialog_input
                                    .column_selection
                                    .set(Some(current_col_idx - 1));
                            }
                        }
                        kanban_domain::card_lifecycle::MoveDirection::Right => {
                            let columns = self.model.columns();
                            let num_cols = self
                                .active_board()
                                .map(|b| columns.iter().filter(|c| c.board_id == b.id).count())
                                .unwrap_or(0);
                            if current_col_idx < num_cols - 1 {
                                self.dialog_input
                                    .column_selection
                                    .set(Some(current_col_idx + 1));
                            }
                        }
                    }
                }
            }

            self.prepare_frame();
            self.select_card_by_id(card_id);
        }
    }

    fn move_selected_cards(&mut self, direction: kanban_domain::card_lifecycle::MoveDirection) {
        let board = match self.active_board() {
            Some(b) => b,
            None => return,
        };

        let card_ids: Vec<uuid::Uuid> = self.multi_select.selected_cards.iter().copied().collect();
        let first_card_id = card_ids.first().copied();

        // Use the pure helper only to resolve the per-card target column;
        // status sync is chained by the service layer's `update_cards`.
        let columns = self.model.columns();
        let cards = self.model.cards();
        let updates: Vec<(uuid::Uuid, CardUpdate)> = card_ids
            .iter()
            .filter_map(|card_id| {
                let card = cards.iter().find(|c| c.id == *card_id)?;
                let result = kanban_domain::card_lifecycle::compute_card_column_move(
                    card, board, columns, cards, direction,
                )?;
                Some((
                    *card_id,
                    CardUpdate {
                        column_id: Some(result.target_column_id),
                        ..Default::default()
                    },
                ))
            })
            .collect();

        let moved_count = updates.len();
        if !updates.is_empty() {
            if let Err(e) = self.ctx.update_cards(updates) {
                let dir = match direction {
                    kanban_domain::card_lifecycle::MoveDirection::Left => "left",
                    kanban_domain::card_lifecycle::MoveDirection::Right => "right",
                };
                tracing::error!("Failed to move cards {}: {}", dir, e);
                self.set_error(format!("Failed to move cards {}: {}", dir, e));
                return;
            }
        }

        tracing::info!("Moved {} cards", moved_count);
        self.multi_select.selected_cards.clear();
        self.multi_select.selection_mode_active = false;
        match direction {
            kanban_domain::card_lifecycle::MoveDirection::Right => {
                self.view.strategy.navigate_right(false);
            }
            kanban_domain::card_lifecycle::MoveDirection::Left => {
                self.view.strategy.navigate_left(false);
            }
        }
        if let Some(card_id) = first_card_id {
            self.prepare_frame();
            self.select_card_by_id(card_id);
        }
    }

    pub fn handle_archive_card(&mut self) {
        if self.focus.active != Focus::Cards {
            return;
        }

        self.animation.archive_anchor = self.cursor_archive_anchor();

        if !self.multi_select.selected_cards.is_empty() {
            self.start_delete_animations_for_selected();
        } else if let Some(card_id) = self.get_selected_card_id() {
            self.start_delete_animation(card_id);
        }
    }

    /// Look up the cursor card's column and position so the post-archive
    /// selection can anchor where the user was actually looking, rather than
    /// inferring it from whichever archived card lands last in HashMap order.
    fn cursor_archive_anchor(&self) -> Option<(uuid::Uuid, i32)> {
        let card_id = self.get_selected_card_id()?;
        self.model
            .cards()
            .iter()
            .find(|c| c.id == card_id)
            .map(|c| (c.column_id, c.position))
    }

    fn start_delete_animations_for_selected(&mut self) {
        let card_ids: Vec<uuid::Uuid> = self.multi_select.selected_cards.iter().copied().collect();
        for card_id in card_ids {
            self.start_delete_animation(card_id);
        }
        self.multi_select.selected_cards.clear();
        self.multi_select.selection_mode_active = false;
    }

    pub fn start_delete_animation(&mut self, card_id: uuid::Uuid) {
        use crate::app::CardAnimation;
        use kanban_domain::AnimationType;
        use std::time::Instant;

        if self.model.cards().iter().any(|c| c.id == card_id) {
            self.animation.animating.insert(
                card_id,
                CardAnimation {
                    animation_type: AnimationType::Archiving,
                    start_time: Instant::now(),
                },
            );
        }
    }

    pub fn select_card_after_deletion(
        &mut self,
        deleted_column_id: uuid::Uuid,
        deleted_position: i32,
    ) {
        // Try to find a card in the same column at or after the deleted position
        if let Some(next_card) = self
            .model
            .cards()
            .iter()
            .find(|c| c.column_id == deleted_column_id && c.position >= deleted_position)
        {
            self.select_card_by_id(next_card.id);
        } else if let Some(prev_card) = self
            .model
            .cards()
            .iter()
            .rev()
            .find(|c| c.column_id == deleted_column_id)
        {
            // Select the last remaining card in the column
            self.select_card_by_id(prev_card.id);
        }
        // Else: no selection (falls back to current behavior - no explicit selection)
    }

    pub fn handle_restore_card(&mut self) {
        if self.mode != AppMode::ArchivedCardsView {
            return;
        }

        if !self.multi_select.selected_cards.is_empty() {
            self.start_restore_animations_for_selected();
        } else if let Some(card_id) = self.get_selected_card_id() {
            self.start_restore_animation(card_id);
        }
    }

    fn start_restore_animations_for_selected(&mut self) {
        let card_ids: Vec<uuid::Uuid> = self.multi_select.selected_cards.iter().copied().collect();
        for card_id in card_ids {
            self.start_restore_animation(card_id);
        }
        self.multi_select.selected_cards.clear();
        self.multi_select.selection_mode_active = false;
    }

    fn start_restore_animation(&mut self, card_id: uuid::Uuid) {
        use crate::app::CardAnimation;
        use kanban_domain::AnimationType;
        use std::time::Instant;

        if self
            .model
            .archived_cards()
            .iter()
            .any(|dc| dc.entity_id == card_id)
        {
            self.animation.animating.insert(
                card_id,
                CardAnimation {
                    animation_type: AnimationType::Restoring,
                    start_time: Instant::now(),
                },
            );
        }
    }

    pub fn restore_card(&mut self, archived_card: ArchivedCard) {
        let card_id = archived_card.entity_id;
        // Reference-marker model: the card stayed LIVE in place while archived, so
        // it keeps its current column/position on restore; there is no "original"
        // location to reconstruct. Read the live card for its column/position and
        // to resolve the restore target if its column was removed.
        let (current_column_id, current_position, card_title) = match self.model.card_by_id(card_id)
        {
            Some(card) => (card.column_id, card.position, card.title.clone()),
            None => return,
        };

        let board_id = self.active_board().map(|b| b.id);

        let columns = self.model.columns();
        let target_column_id = board_id
            .and_then(|bid| {
                kanban_domain::card_lifecycle::resolve_restore_column(
                    current_column_id,
                    bid,
                    columns,
                )
            })
            .unwrap_or(current_column_id);

        let cmd = Command::Card(CardCommand::Restore(RestoreCard {
            card_id,
            column_id: target_column_id,
            position: current_position,
            timestamp: chrono::Utc::now(),
        }));

        if let Err(e) = self.execute_command(cmd) {
            tracing::error!("Failed to restore card: {}", e);
            self.set_error(format!("Failed to restore card: {}", e));
            return;
        }

        tracing::info!("Card '{}' restored to original position", card_title);
    }

    pub fn handle_delete_card_permanent(&mut self) {
        if self.mode != AppMode::ArchivedCardsView {
            return;
        }

        if !self.multi_select.selected_cards.is_empty() {
            self.start_permanent_delete_animations_for_selected();
        } else if let Some(card_id) = self.get_selected_card_id() {
            self.start_permanent_delete_animation(card_id);
        }
    }

    fn start_permanent_delete_animations_for_selected(&mut self) {
        let card_ids: Vec<uuid::Uuid> = self.multi_select.selected_cards.iter().copied().collect();
        for card_id in card_ids {
            self.start_permanent_delete_animation(card_id);
        }
        self.multi_select.selected_cards.clear();
        self.multi_select.selection_mode_active = false;
    }

    fn start_permanent_delete_animation(&mut self, card_id: uuid::Uuid) {
        use crate::app::CardAnimation;
        use kanban_domain::AnimationType;
        use std::time::Instant;

        if self
            .model
            .archived_cards()
            .iter()
            .any(|dc| dc.entity_id == card_id)
        {
            self.animation.animating.insert(
                card_id,
                CardAnimation {
                    animation_type: AnimationType::Deleting,
                    start_time: Instant::now(),
                },
            );
        }
    }

    pub fn handle_toggle_archived_cards_view(&mut self) {
        match self.mode {
            // `push_mode` snapshots whichever context we're toggling from (a live
            // board's Normal mode, or a drilled-in archived board), so `pop_mode`
            // below always returns to the correct origin — not a hardcoded
            // `Normal`, which would strand a drilled-in archived board.
            AppMode::Normal | AppMode::ArchivedBoardsView => {
                self.push_mode(AppMode::ArchivedCardsView);
                self.prepare_frame();

                // Initialize selection in view strategy
                if let Some(list) = self.view.strategy.get_active_task_list_mut() {
                    if !list.is_empty() {
                        list.set_selected_index(Some(0));
                        list.ensure_selected_visible(self.view.viewport_height);
                    }
                }
                self.needs_redraw = true;
            }
            AppMode::ArchivedCardsView => {
                self.pop_mode();
                self.prepare_frame();

                // Re-initialize selection when returning to normal view
                if let Some(list) = self.view.strategy.get_active_task_list_mut() {
                    if !list.is_empty() {
                        list.set_selected_index(Some(0));
                        list.ensure_selected_visible(self.view.viewport_height);
                    }
                }
                self.needs_redraw = true;
            }
            _ => {}
        }
    }

    pub fn handle_manage_children_from_list(&mut self) {
        // Get the currently selected card from the list view
        let card = match self.get_selected_card_in_context() {
            Some(c) => c,
            None => return,
        };

        let card_id = card.id;

        // Get the board ID for filtering
        let board_id = match self.active_board() {
            Some(board) => board.id,
            None => return,
        };

        // Get ancestors to exclude (would create cycle)
        let graph = self.model.graph();
        let ancestors = graph.ancestors(card_id);

        // Get cards from current board, excluding self and ancestors
        let columns = self.model.columns();
        let column_ids: std::collections::HashSet<_> = columns
            .iter()
            .filter(|c| c.board_id == board_id)
            .map(|c| c.id)
            .collect();

        let cards = self.model.cards();
        let eligible_cards: Vec<_> = cards
            .iter()
            .filter(|c| column_ids.contains(&c.column_id))
            .filter(|c| c.id != card_id)
            .filter(|c| !ancestors.contains(&c.id))
            .map(|c| c.id)
            .collect();

        // Get current children (for checkbox display)
        let graph = self.model.graph();
        let current_children: std::collections::HashSet<_> =
            graph.children(card_id).into_iter().collect();

        // Store the active card so the popup knows which card we're managing
        self.set_active_card_or_clear(card_id);

        // Set up dialog state
        self.relationship.card_ids = eligible_cards;
        self.relationship.selected = current_children;
        self.relationship.selection.set(Some(0));
        self.relationship.search.clear();

        self.open_dialog(DialogMode::ManageChildren);
    }
}

#[cfg(test)]
mod create_card_factory_tests {
    use crate::App;
    use kanban_domain::KanbanOperations;

    /// Refresh the TUI model from the store so the create handler (which reads
    /// `self.model`) sees prior writes. The event loop does this each frame via
    /// `prepare_frame`; tests pull the snapshot directly.
    fn refresh(app: &mut App) {
        let snap = app.ctx.snapshot().unwrap();
        app.model.load_from_snapshot(snap);
    }

    /// Seed a board with one column through the service, then point the TUI's
    /// active selection at it so `create_card` has a board + focused column.
    fn seed_active_board_with_column(app: &mut App) {
        let board = app
            .ctx
            .create_board("Board".into(), Some("KAN".into()))
            .unwrap();
        app.ctx
            .create_column(board.id, "TODO".into(), Some(0))
            .unwrap();
        refresh(app);
        app.selection.active_board_id = app.model.boards().first().map(|b| b.id);
    }

    /// KAN-796: the TUI card-create entry point funnels through the Card factory
    /// (`Card::create` via the `CreateCard` command), so a created card carries
    /// the factory-seeded server-managed `card_number` (1 for the first card)
    /// rather than diverging from the board counter.
    #[test]
    fn test_tui_create_card_routes_through_factory_seeds_number() {
        let mut app = App::test_default();
        seed_active_board_with_column(&mut app);

        app.input.set("Ship it".to_string());
        app.create_card();
        app.input.clear();

        let cards = app.ctx.data_store().list_all_cards().unwrap();
        let card = cards
            .iter()
            .find(|c| c.title == "Ship it")
            .expect("created card present in store");
        // Factory seeds the user-facing number from the board counter.
        assert_eq!(card.card_number, 1);
        // Factory uses one clock for both timestamps at create.
        assert_eq!(card.created_at, card.updated_at);
    }

    /// Two successive TUI creates funnel through the factory + board counter, so
    /// the second card's `card_number` is bumped (server-managed allocation),
    /// never a stale repeat of the first.
    #[test]
    fn test_tui_create_card_bumps_board_counter_through_factory() {
        let mut app = App::test_default();
        seed_active_board_with_column(&mut app);

        for title in ["First", "Second"] {
            app.input.set(title.to_string());
            app.create_card();
            app.input.clear();
        }

        let cards = app.ctx.data_store().list_all_cards().unwrap();
        let first = cards.iter().find(|c| c.title == "First").unwrap();
        let second = cards.iter().find(|c| c.title == "Second").unwrap();
        assert_eq!(first.card_number, 1);
        assert_eq!(
            second.card_number, 2,
            "the factory bumps the board counter on each create"
        );
    }
}
