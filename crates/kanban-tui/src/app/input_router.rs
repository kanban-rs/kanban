use super::{App, AppMode, DialogMode, Focus};
use crate::events::EventHandler;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Instant;

impl App {
    pub(in crate::app) fn handle_key_event(
        &mut self,
        key: crossterm::event::KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
    ) -> bool {
        use crossterm::event::KeyCode;
        let mut should_restart_events = false;

        // Clear banner on any key press
        if self.ui_state.banner.is_some() {
            self.clear_banner();
            return false;
        }

        let is_input_mode = matches!(
            self.mode,
            AppMode::Search
                | AppMode::Dialog(DialogMode::CreateBoard)
                | AppMode::Dialog(DialogMode::CreateCard)
                | AppMode::Dialog(DialogMode::CreateSprint)
                | AppMode::Dialog(DialogMode::RenameBoard)
                | AppMode::Dialog(DialogMode::ExportBoard)
                | AppMode::Dialog(DialogMode::ExportAll)
                | AppMode::Dialog(DialogMode::SetCardPoints)
                | AppMode::Dialog(DialogMode::SetBranchPrefix)
                | AppMode::Dialog(DialogMode::CreateColumn)
                | AppMode::Dialog(DialogMode::RenameColumn)
                | AppMode::Dialog(DialogMode::SetSprintPrefix)
                | AppMode::Dialog(DialogMode::SetSprintCardPrefix)
                | AppMode::Dialog(DialogMode::ChooseStorageFile)
        );

        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
            && !is_input_mode
            && !matches!(
                self.mode,
                AppMode::ArchivedCardsView
                    | AppMode::ArchivedBoardsView
                    | AppMode::Dialog(DialogMode::DeleteBoardConfirm)
                    | AppMode::Dialog(DialogMode::DeleteColumnConfirm)
                    | AppMode::Dialog(DialogMode::DeletePermanentBoardConfirm)
            )
        {
            self.handle_quit_key();
            return false;
        }

        if matches!(key.code, KeyCode::F(12)) && !matches!(self.mode, AppMode::ErrorLog) {
            self.open_error_log();
            return false;
        }

        if matches!(key.code, KeyCode::Char('?'))
            && !is_input_mode
            && !matches!(self.mode, AppMode::Help(_))
        {
            let previous_mode = self.mode.clone();
            let provider = crate::keybindings::KeybindingRegistry::get_provider(self);
            let context = provider.get_context();
            self.ui_state
                .help_list
                .update_item_count(context.bindings.len());
            self.ui_state.help_list.set_scroll_offset(0);
            self.mode = AppMode::Help(Box::new(previous_mode));
            return false;
        }

        // Handle Ctrl+a for select all cards
        if matches!(self.mode, AppMode::Normal)
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('a'))
        {
            self.pending_key = None;
            self.handle_select_all_cards_in_view();
            return false;
        }

        match self.mode {
            AppMode::Normal => {
                should_restart_events = self.handle_normal_key(key, terminal, event_handler);
            }
            AppMode::CardDetail => {
                should_restart_events =
                    self.handle_card_detail_key(key.code, terminal, event_handler);
            }
            AppMode::BoardDetail => {
                should_restart_events =
                    self.handle_board_detail_key(key.code, terminal, event_handler);
            }
            AppMode::SprintDetail => self.handle_sprint_detail_key(key.code),
            AppMode::Search => self.handle_search_mode(key.code),
            AppMode::ArchivedCardsView => {
                should_restart_events =
                    self.handle_archived_cards_view_mode(key, terminal, event_handler);
            }
            AppMode::ArchivedBoardsView => self.handle_archived_boards_view_mode(key.code),
            AppMode::Settings => {
                should_restart_events = self.handle_settings_key(key.code, terminal, event_handler);
            }
            AppMode::Help(_) => self.handle_help_mode(key.code),
            AppMode::ErrorLog => self.handle_error_log_mode(key.code),
            AppMode::Dialog(ref dialog) => match dialog {
                DialogMode::CreateBoard => self.handle_create_board_dialog(key.code),
                DialogMode::CreateCard => self.handle_create_card_dialog(key.code),
                DialogMode::CreateSprint => self.handle_create_sprint_dialog(key.code),
                DialogMode::RenameBoard => self.handle_rename_board_dialog(key.code),
                DialogMode::ExportBoard => self.handle_export_board_dialog(key.code),
                DialogMode::ExportAll => self.handle_export_all_dialog(key.code),
                DialogMode::ImportBoard => self.handle_import_board_popup(key.code),
                DialogMode::SetCardPoints => {
                    should_restart_events = self.handle_set_card_points_dialog(key.code);
                }
                DialogMode::SetCardPriority => self.handle_set_card_priority_popup(key.code),
                DialogMode::SetMultipleCardsPriority => {
                    self.handle_set_multiple_cards_priority_popup(key.code)
                }
                DialogMode::SetBranchPrefix => self.handle_set_branch_prefix_dialog(key.code),
                DialogMode::SetSprintPrefix => self.handle_set_sprint_prefix_dialog(key.code),
                DialogMode::SetSprintCardPrefix => {
                    self.handle_set_sprint_card_prefix_dialog(key.code)
                }
                DialogMode::OrderCards => {
                    should_restart_events = self.handle_order_cards_popup(key.code);
                }
                DialogMode::AssignCardToSprint => self.handle_assign_card_to_sprint_popup(key.code),
                DialogMode::AssignMultipleCardsToSprint => {
                    self.handle_assign_multiple_cards_to_sprint_popup(key.code)
                }
                DialogMode::CreateColumn => self.handle_create_column_dialog(key.code),
                DialogMode::RenameColumn => self.handle_rename_column_dialog(key.code),
                DialogMode::DeleteColumnConfirm => {
                    self.handle_delete_column_confirm_popup(key.code)
                }
                DialogMode::DeleteBoardConfirm => self.handle_delete_board_confirm_popup(key.code),
                DialogMode::SelectTaskListView => self.handle_select_task_list_view_popup(key.code),
                DialogMode::ConfirmSprintPrefixCollision => {
                    self.handle_confirm_sprint_prefix_collision_popup(key.code)
                }
                DialogMode::FilterOptions => self.handle_filter_options_popup(key.code),
                DialogMode::ConflictResolution => self.handle_conflict_resolution_popup(key.code),
                DialogMode::ExternalChangeDetected => {
                    self.handle_external_change_detected_popup(key.code)
                }
                DialogMode::ManageParents => self.handle_manage_parents_popup(key.code),
                DialogMode::ManageChildren => self.handle_manage_children_popup(key.code),
                DialogMode::CarryOverSprint => self.handle_carry_over_sprint_popup(key.code),
                DialogMode::ExportBoards => self.handle_export_boards_dialog(key.code),
                DialogMode::ChooseStorageFile => self.handle_choose_storage_file_dialog(key.code),
                DialogMode::DeletePermanentBoardConfirm => {
                    self.handle_delete_permanent_board_confirm_popup(key.code)
                }
            },
        }
        should_restart_events
    }

    /// The shared Normal-mode key dispatch for both panels. The archived-cards
    /// view delegates here too (after intercepting its restore/delete extension
    /// keys), so an archived card is driven by the exact same card handlers a
    /// live card is — the only archival distinction survives at the tasks-panel
    /// data source and the display styling, never in an operation branch.
    fn handle_normal_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
    ) -> bool {
        use crossterm::event::KeyCode;
        let mut should_restart_events = false;
        match key.code {
            KeyCode::Char('/') => {
                self.pending_key = None;
                if self.focus.active == Focus::Cards {
                    self.filter.search.activate();
                    self.mode = AppMode::Search;
                }
            }
            KeyCode::Char('g') => {
                if self.pending_key == Some('g') {
                    self.pending_key = None;
                    self.handle_jump_to_top();
                } else {
                    self.pending_key = Some('g');
                }
            }
            KeyCode::Char('G') => {
                self.pending_key = None;
                self.handle_jump_to_bottom();
            }
            KeyCode::Char('{') => {
                self.pending_key = None;
                self.handle_jump_half_viewport_up();
            }
            KeyCode::Char('}') => {
                self.pending_key = None;
                self.handle_jump_half_viewport_down();
            }
            KeyCode::Char('n') => {
                self.pending_key = None;
                match self.focus.active {
                    Focus::Boards => self.handle_create_board_key(),
                    Focus::Cards => self.handle_create_card_key(),
                }
            }
            KeyCode::Char('r') => {
                self.pending_key = None;
                self.handle_rename_board_key();
            }
            KeyCode::Char('e') => {
                self.pending_key = None;
                match self.focus.active {
                    Focus::Boards => self.handle_edit_board_key(),
                    Focus::Cards => {
                        should_restart_events = self.handle_edit_card_key(terminal, event_handler);
                    }
                }
            }
            KeyCode::Char('x') => {
                self.pending_key = None;
                self.handle_export_board_key();
            }
            KeyCode::Char('X') => {
                self.pending_key = None;
                self.handle_export_all_key();
            }
            KeyCode::Char('d') => {
                self.pending_key = None;
                // Mirror the card removal flow: `d` is the primary removal
                // action on both panels (delete a board / archive a card).
                match self.focus.active {
                    Focus::Boards => self.handle_delete_board_key(),
                    Focus::Cards => self.handle_archive_card(),
                }
            }
            KeyCode::Char('D') => {
                self.pending_key = None;
                // `D` toggles the archived view of the focused panel: the
                // archived-cards view on the cards panel, the archived-boards
                // view on the boards panel.
                match self.focus.active {
                    Focus::Cards => self.handle_toggle_archived_cards_view(),
                    Focus::Boards => self.handle_toggle_archived_boards_view(),
                }
            }
            KeyCode::Char('i') => {
                self.pending_key = None;
                self.handle_import_board_key();
            }
            KeyCode::Char('a') => {
                self.pending_key = None;
                self.handle_assign_to_sprint_key();
            }
            KeyCode::Char('c') => {
                self.pending_key = None;
                self.handle_toggle_card_completion();
            }
            KeyCode::Char('s') => {
                self.pending_key = None;
                if self.focus.active == Focus::Cards {
                    self.handle_manage_children_from_list();
                }
            }
            KeyCode::Char('o') => {
                self.pending_key = None;
                self.handle_order_cards_key();
            }
            KeyCode::Char('O') => {
                self.pending_key = None;
                self.handle_toggle_sort_order_key();
            }
            KeyCode::Char('T') => {
                self.pending_key = None;
                self.handle_open_filter_dialog();
            }
            KeyCode::Char('t') => {
                self.pending_key = None;
                self.handle_toggle_sprint_filter();
            }
            KeyCode::Char('v') => {
                self.pending_key = None;
                self.handle_card_selection_toggle();
            }
            KeyCode::Char('V') => {
                self.pending_key = None;
                self.handle_toggle_task_list_view();
            }
            KeyCode::Char('p') => {
                self.pending_key = None;
                if self.focus.active == Focus::Cards {
                    self.handle_set_card_priority_key();
                }
            }
            KeyCode::Char('P') => {
                self.pending_key = None;
                self.handle_set_selected_cards_priority();
            }
            KeyCode::Char('H') => {
                self.pending_key = None;
                self.handle_move_card_left();
            }
            KeyCode::Char('L') => {
                self.pending_key = None;
                self.handle_move_card_right();
            }
            KeyCode::Char('h') => {
                self.pending_key = None;
                self.handle_kanban_column_left();
            }
            KeyCode::Char('l') => {
                self.pending_key = None;
                self.handle_kanban_column_right();
            }
            KeyCode::Char('1') => {
                self.pending_key = None;
                self.handle_column_or_focus_switch(0);
            }
            KeyCode::Char('2') => {
                self.pending_key = None;
                self.handle_column_or_focus_switch(1);
            }
            KeyCode::Char('3') => {
                self.pending_key = None;
                self.handle_column_or_focus_switch(2);
            }
            KeyCode::Char('4') => {
                self.pending_key = None;
                self.handle_column_or_focus_switch(3);
            }
            KeyCode::Char('5') => {
                self.pending_key = None;
                self.handle_column_or_focus_switch(4);
            }
            KeyCode::Char('6') => {
                self.pending_key = None;
                self.handle_column_or_focus_switch(5);
            }
            KeyCode::Char('7') => {
                self.pending_key = None;
                self.handle_column_or_focus_switch(6);
            }
            KeyCode::Char('8') => {
                self.pending_key = None;
                self.handle_column_or_focus_switch(7);
            }
            KeyCode::Char('9') => {
                self.pending_key = None;
                self.handle_column_or_focus_switch(8);
            }
            KeyCode::Esc => {
                self.pending_key = None;
                self.handle_escape_key();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.pending_key = None;
                self.handle_navigation_down();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.pending_key = None;
                self.handle_navigation_up();
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.pending_key = None;
                self.handle_selection_activate();
            }
            KeyCode::Char('u') => {
                self.pending_key = None;
                if let Err(e) = self.undo() {
                    self.set_error(format!("Undo failed: {}", e));
                }
            }
            KeyCode::Char('U') => {
                self.pending_key = None;
                if let Err(e) = self.redo() {
                    self.set_error(format!("Redo failed: {}", e));
                }
            }
            KeyCode::Char('S') => {
                self.pending_key = None;
                self.handle_open_settings();
            }
            _ => {
                self.pending_key = None;
            }
        }
        should_restart_events
    }

    fn handle_search_mode(&mut self, key_code: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        match key_code {
            KeyCode::Char(c) => {
                self.filter.search.input.insert_char(c);
            }
            KeyCode::Backspace => {
                self.filter.search.input.backspace();
            }
            KeyCode::Enter => {
                self.mode = AppMode::Normal;
            }
            KeyCode::Esc => {
                self.filter.search.deactivate();
                self.mode = AppMode::Normal;
            }
            _ => {}
        }
    }

    /// Key handling for the archived-cards view. The archived list is the
    /// ordinary cards panel showing a different SET; every card operation
    /// (detail, edit, move, priority, sprint-assign, navigation...) reuses the
    /// SAME shared Normal-mode dispatch (`handle_normal_key`) so an archived card
    /// is substitutable for a live one. Only the consumption-site extension keys
    /// are intercepted here: `r` restores and `x` permanently deletes the
    /// highlighted archived card, and `Esc`/`q` toggles back to the live set.
    pub fn handle_archived_cards_view_mode(
        &mut self,
        key: crossterm::event::KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
    ) -> bool {
        // The archived-set extension keys (restore/delete/toggle) are handled
        // terminal-free; everything else reuses the shared Normal-mode card
        // handlers, proving reuse: an archived card flows through the identical
        // operation paths as a live one.
        if self.handle_archived_cards_extension_key(key) {
            false
        } else {
            self.handle_normal_key(key, terminal, event_handler)
        }
    }

    /// The archived-set consumption-site affordances layered on top of the
    /// shared card list: `r` restores and `x` permanently deletes the
    /// highlighted archived card(s), and `Esc`/`q` toggles back to the live set.
    /// Returns `true` when the key was one of these extension keys (and was
    /// consumed here), `false` when it should fall through to the shared
    /// Normal-mode dispatch. Terminal-free, so it is unit-testable headlessly.
    pub fn handle_archived_cards_extension_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        use crossterm::event::KeyCode;
        // The archived list lives on the cards panel.
        self.focus.active = Focus::Cards;

        match key.code {
            KeyCode::Char('r') => {
                self.pending_key = None;
                self.handle_restore_card();
                true
            }
            KeyCode::Char('x') => {
                self.pending_key = None;
                self.handle_delete_card_permanent();
                true
            }
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.pending_key = None;
                self.handle_toggle_archived_cards_view();
                true
            }
            _ => false,
        }
    }

    /// Key handling for the archived-boards view. The archived list is an
    /// ordinary boards panel showing a different SET; navigation and activation
    /// reuse the SAME shared handlers as the live projects panel (no separate
    /// operation dispatch). Only the consumption-site keys differ: `r` restores
    /// and `x` permanently deletes the highlighted archived board, and `Esc`/`q`
    /// toggles back to the live set. Once a board is activated with `Enter` it is
    /// THE active board and every view/operation is board-agnostic.
    pub fn handle_archived_boards_view_mode(&mut self, key_code: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        // While browsing the list (no board activated) the Boards panel is the
        // context. Once a board is active the focus is on Cards; leave it.
        if self.focus.active != Focus::Boards && self.selection.active_board_id.is_none() {
            self.focus.active = Focus::Boards;
        }

        match key_code {
            KeyCode::Char('r') => self.handle_restore_board(),
            KeyCode::Char('x') => self.handle_delete_archived_board_key(),
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q')
                if self.selection.active_board_id.is_none() =>
            {
                self.handle_toggle_archived_boards_view();
            }
            KeyCode::Char('g') => {
                if self.pending_key == Some('g') {
                    self.pending_key = None;
                    self.handle_jump_to_top();
                } else {
                    self.pending_key = Some('g');
                }
            }
            // Everything else reuses the shared Normal-mode boards handlers:
            // activation, navigation, jumps, undo/redo — proving reuse.
            other => {
                self.pending_key = None;
                self.handle_shared_boards_key(other);
            }
        }
    }

    /// Dispatch the boards-panel keys shared between the live and archived
    /// projects views. Board-set-agnostic: it drives the same navigation and
    /// activation handlers regardless of which set the panel shows.
    fn handle_shared_boards_key(&mut self, key_code: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        match key_code {
            KeyCode::Enter | KeyCode::Char(' ') => self.handle_selection_activate(),
            KeyCode::Char('j') | KeyCode::Down => self.handle_navigation_down(),
            KeyCode::Char('k') | KeyCode::Up => self.handle_navigation_up(),
            KeyCode::Char('G') => self.handle_jump_to_bottom(),
            KeyCode::Char('u') => {
                if let Err(e) = self.undo() {
                    self.set_error(format!("Undo failed: {e}"));
                }
                self.prepare_frame();
                self.selection.board.clamp(self.displayed_boards().len());
            }
            KeyCode::Char('U') => {
                if let Err(e) = self.redo() {
                    self.set_error(format!("Redo failed: {e}"));
                }
                self.prepare_frame();
                self.selection.board.clamp(self.displayed_boards().len());
            }
            _ => {}
        }
    }

    /// Scrolls the help list so the selected item is visible.
    ///
    /// Two passes are needed because `get_adjusted_viewport_height` reserves rows
    /// for scroll indicators, and an indicator can appear or disappear after the
    /// first `ensure_selected_visible` call — changing the available height. A
    /// second pass with the updated height corrects any residual mis-alignment.
    pub(in crate::app) fn scroll_help_into_view(&mut self) {
        let raw = crate::ui::help_popup_viewport_height(self.view.last_frame_area);
        if raw == 0 {
            return;
        }
        let h0 = self.ui_state.help_list.get_adjusted_viewport_height(raw);
        self.ui_state.help_list.ensure_selected_visible(h0);
        let h1 = self.ui_state.help_list.get_adjusted_viewport_height(raw);
        if h1 != h0 {
            self.ui_state.help_list.ensure_selected_visible(h1);
        }
    }

    fn handle_help_mode(&mut self, key_code: crossterm::event::KeyCode) {
        use crate::keybindings::KeybindingRegistry;
        use crossterm::event::KeyCode;

        match key_code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.ui_state.help_pending_action = None;
                self.ui_state.help_list.navigate_down();
                self.scroll_help_into_view();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.ui_state.help_pending_action = None;
                self.ui_state.help_list.navigate_up();
                self.scroll_help_into_view();
            }
            KeyCode::Char('h') | KeyCode::Char('l') => {
                self.ui_state.help_pending_action = None;
            }
            KeyCode::Enter => {
                self.ui_state.help_pending_action = None;
                if let Some(index) = self.ui_state.help_list.get_selected_index() {
                    let provider = KeybindingRegistry::get_provider(self);
                    let context = provider.get_context();

                    if let Some(binding) = context.bindings.get(index) {
                        if let AppMode::Help(previous_mode) = &self.mode {
                            self.mode = (**previous_mode).clone();
                        } else {
                            self.mode = AppMode::Normal;
                        }
                        self.ui_state.help_list.reset();

                        self.execute_action(&binding.action);
                    }
                }
            }
            KeyCode::Esc | KeyCode::Char('?') => {
                self.ui_state.help_pending_action = None;
                if let AppMode::Help(previous_mode) = &self.mode {
                    self.mode = (**previous_mode).clone();
                } else {
                    self.mode = AppMode::Normal;
                }
                self.ui_state.help_list.reset();
            }
            _ => {
                let provider = KeybindingRegistry::get_provider(self);
                let context = provider.get_context();

                if let Some((index, binding)) = context
                    .bindings
                    .iter()
                    .enumerate()
                    .find(|(_, b)| Self::keycode_matches_binding_key(&key_code, &b.key))
                {
                    self.ui_state.help_list.jump_to(index);
                    self.scroll_help_into_view();
                    self.ui_state.help_pending_action = Some((Instant::now(), binding.action));
                }
            }
        }
    }
}
