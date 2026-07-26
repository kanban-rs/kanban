use super::{App, AppMode, Focus};
use crate::events::EventHandler;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

#[derive(Debug, PartialEq, Eq)]
pub(in crate::app) enum EditCardDispatch {
    CardList,
    CardDetail,
    SprintDetail(uuid::Uuid),
    SettingsConfig,
    Noop,
}

impl App {
    pub(in crate::app) fn keycode_matches_binding_key(
        key_code: &crossterm::event::KeyCode,
        binding_key: &str,
    ) -> bool {
        use crossterm::event::KeyCode;

        match key_code {
            KeyCode::Char(c) => {
                // Check if the entire binding_key is a single char match (handles "/" correctly)
                if binding_key.len() == 1 && binding_key.starts_with(*c) {
                    return true;
                }
                // Check if any part after splitting on '/' matches
                binding_key
                    .split('/')
                    .any(|k| k.trim().len() == 1 && k.trim().starts_with(*c))
            }
            KeyCode::Enter => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "Enter" || trimmed == "ENTER"
            }),
            KeyCode::Esc => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "Esc" || trimmed == "ESC"
            }),
            KeyCode::Backspace => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "Backspace" || trimmed == "BACKSPACE"
            }),
            KeyCode::Home => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "Home" || trimmed == "HOME"
            }),
            KeyCode::End => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "End" || trimmed == "END"
            }),
            KeyCode::Down => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "↓" || trimmed == "Down" || trimmed == "DOWN"
            }),
            KeyCode::Up => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "↑" || trimmed == "Up" || trimmed == "UP"
            }),
            KeyCode::Left => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "←" || trimmed == "Left" || trimmed == "LEFT"
            }),
            KeyCode::Right => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "→" || trimmed == "Right" || trimmed == "RIGHT"
            }),
            _ => false,
        }
    }

    /// Which real handler `EditCard` reaches for the app's current mode/focus.
    /// Pure and terminal-free so the dispatch decision is unit-testable on its
    /// own, mirroring `edit_key_active`.
    pub(in crate::app) fn resolve_edit_card_dispatch(&self) -> EditCardDispatch {
        if self.edit_key_active() {
            EditCardDispatch::CardList
        } else if self.mode == AppMode::CardDetail {
            EditCardDispatch::CardDetail
        } else if self.mode == AppMode::SprintDetail {
            match self.sprint_detail_selected_card_id() {
                Some(card_id) => EditCardDispatch::SprintDetail(card_id),
                None => EditCardDispatch::Noop,
            }
        } else if self.mode == AppMode::Settings {
            // Matches handle_settings_key's own Char('e') arm, which opens the
            // config editor regardless of settings_focus.
            EditCardDispatch::SettingsConfig
        } else {
            EditCardDispatch::Noop
        }
    }

    /// SprintDetail's `EditCard` target: identical to `CardListAction::Edit`'s
    /// effect on the shared `CardListComponent` dispatch, needs no terminal.
    pub(in crate::app) fn open_sprint_detail_card_for_edit(&mut self, card_id: uuid::Uuid) {
        if self.activate_card(card_id) {
            let parents = self.get_current_card_parents();
            let children = self.get_current_card_children();
            self.relationship
                .parents_list
                .update_item_count(parents.len());
            self.relationship
                .children_list
                .update_item_count(children.len());
            self.push_mode(AppMode::CardDetail);
            self.focus.card_focus = crate::app::CardFocus::Title;
        }
    }

    // EditCard can launch the external editor, which needs the terminal — kept
    // out of execute_action (terminal-free, unit-testable) and called directly
    // by both Help-mode call sites instead.
    pub(in crate::app) fn execute_edit_card_action(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
    ) -> bool {
        match self.resolve_edit_card_dispatch() {
            EditCardDispatch::CardList => self.handle_edit_card_key(terminal, event_handler),
            EditCardDispatch::CardDetail => {
                self.edit_card_detail_focused_field(terminal, event_handler)
            }
            EditCardDispatch::SprintDetail(card_id) => {
                self.open_sprint_detail_card_for_edit(card_id);
                false
            }
            EditCardDispatch::SettingsConfig => self.open_config_editor(terminal, event_handler),
            EditCardDispatch::Noop => false,
        }
    }

    /// Single entry point for firing a Help-menu binding, shared by the
    /// Enter-immediate and deferred jump-then-fire call sites so they can't
    /// drift on which actions need the terminal restarted afterward.
    pub(in crate::app) fn dispatch_help_action(
        &mut self,
        action: crate::keybindings::KeybindingAction,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
    ) -> bool {
        if action == crate::keybindings::KeybindingAction::EditCard {
            self.execute_edit_card_action(terminal, event_handler)
        } else {
            self.execute_action(&action);
            false
        }
    }

    pub(in crate::app) fn execute_action(&mut self, action: &crate::keybindings::KeybindingAction) {
        use crate::keybindings::KeybindingAction;
        use crossterm::event::KeyCode;

        match action {
            KeybindingAction::NavigateDown => self.handle_navigation_down(),
            KeybindingAction::NavigateUp => self.handle_navigation_up(),
            KeybindingAction::NavigateLeft => self.handle_kanban_column_left(),
            KeybindingAction::NavigateRight => self.handle_kanban_column_right(),
            KeybindingAction::SelectItem => self.handle_selection_activate(),
            KeybindingAction::CreateCard => self.handle_create_card_key(),
            KeybindingAction::CreateBoard => self.handle_create_board_key(),
            KeybindingAction::CreateSprint => self.handle_create_sprint_key(),
            KeybindingAction::CreateColumn => self.handle_create_column_key(),
            KeybindingAction::RenameBoard => self.handle_rename_board_key(),
            KeybindingAction::RenameColumn => self.handle_rename_column_key(),
            KeybindingAction::EditCard => {}
            KeybindingAction::EditBoard => self.handle_edit_board_key(),
            KeybindingAction::ToggleCompletion => self.handle_toggle_card_completion(),
            KeybindingAction::AssignToSprint => self.handle_assign_to_sprint_key(),
            KeybindingAction::ArchiveCard => self.handle_archive_card(),
            KeybindingAction::RestoreCard => self.handle_restore_card(),
            KeybindingAction::DeleteCard => self.handle_delete_card_permanent(),
            KeybindingAction::MoveCardLeft => self.handle_move_card_left(),
            KeybindingAction::MoveCardRight => self.handle_move_card_right(),
            KeybindingAction::MoveColumnUp => self.handle_move_column_up(),
            KeybindingAction::MoveColumnDown => self.handle_move_column_down(),
            KeybindingAction::DeleteColumn => self.handle_delete_column_key(),
            KeybindingAction::DeleteBoard => self.handle_delete_board_key(),
            KeybindingAction::ExportBoard => self.handle_export_board_key(),
            KeybindingAction::ExportAll => self.handle_export_all_key(),
            KeybindingAction::ImportBoard => self.handle_import_board_key(),
            KeybindingAction::OrderCards => self.handle_order_cards_key(),
            KeybindingAction::OrderBoards => self.handle_order_boards_key(),
            KeybindingAction::ToggleSortOrder => self.handle_toggle_sort_order_key(),
            KeybindingAction::ToggleFilter => self.handle_toggle_sprint_filter(),
            KeybindingAction::ToggleHideAssigned => self.handle_open_filter_dialog(),
            KeybindingAction::ToggleArchivedView => self.handle_toggle_archived_cards_view(),
            KeybindingAction::ToggleArchivedBoardsView => self.handle_toggle_archived_boards_view(),
            KeybindingAction::RestoreBoard => self.handle_restore_board(),
            KeybindingAction::DeleteArchivedBoard => self.handle_delete_archived_board(),
            KeybindingAction::ToggleBoardsSortOrder => self.handle_toggle_board_sort_order(),
            KeybindingAction::ToggleTaskListView => self.handle_toggle_task_list_view(),
            KeybindingAction::ToggleCardSelection => self.handle_card_selection_toggle(),
            KeybindingAction::ClearCardSelection => self.handle_clear_card_selection(),
            KeybindingAction::SelectAllCards => self.handle_select_all_cards_in_view(),
            KeybindingAction::SetCardPriority => self.handle_set_card_priority_key(),
            KeybindingAction::SetSelectedCardsPriority => self.handle_set_selected_cards_priority(),
            KeybindingAction::Search => {
                if self.focus.active == Focus::Cards {
                    self.filter.search.activate();
                    self.push_mode(AppMode::Search);
                }
            }
            KeybindingAction::ShowHelp => {}
            KeybindingAction::Escape => self.handle_escape_key(),
            KeybindingAction::FocusPanel(panel) => self.handle_column_or_focus_switch(*panel),
            KeybindingAction::JumpToTop => self.handle_jump_to_top(),
            KeybindingAction::JumpToBottom => self.handle_jump_to_bottom(),
            KeybindingAction::JumpHalfViewportUp => self.handle_jump_half_viewport_up(),
            KeybindingAction::JumpHalfViewportDown => self.handle_jump_half_viewport_down(),
            KeybindingAction::ManageParents => self.handle_manage_parents(),
            KeybindingAction::ManageChildren => self.handle_manage_children(),
            KeybindingAction::CarryOver => self.carry_over_active_sprint_if_eligible(),
            KeybindingAction::Undo => {
                if let Err(e) = self.undo() {
                    self.set_error(format!("Undo failed: {}", e));
                }
            }
            KeybindingAction::Redo => {
                if let Err(e) = self.redo() {
                    self.set_error(format!("Redo failed: {}", e));
                }
            }
            KeybindingAction::OpenSettings => self.handle_open_settings(),
            KeybindingAction::ExportBoards => self.open_export_boards_dialog(),
            KeybindingAction::ConfirmPrefixCollision => {
                self.handle_confirm_sprint_prefix_collision_popup(KeyCode::Enter);
            }
            KeybindingAction::RejectPrefixCollision => {
                self.handle_confirm_sprint_prefix_collision_popup(KeyCode::Char('n'));
            }
            KeybindingAction::CancelPrefixCollision => {
                self.handle_confirm_sprint_prefix_collision_popup(KeyCode::Esc);
            }
            KeybindingAction::ForceOverwriteConflict => {
                self.handle_conflict_resolution_popup(KeyCode::Char('o'));
            }
            KeybindingAction::TakeTheirsConflict => {
                self.handle_conflict_resolution_popup(KeyCode::Char('t'));
            }
            KeybindingAction::CancelConflictResolution => {
                self.handle_conflict_resolution_popup(KeyCode::Esc);
            }
            KeybindingAction::ReloadDiscardLocal => {
                self.handle_external_change_detected_popup(KeyCode::Char('r'));
            }
            KeybindingAction::KeepLocalChanges => {
                self.handle_external_change_detected_popup(KeyCode::Char('k'));
            }
            KeybindingAction::DismissExternalChange => {
                self.handle_external_change_detected_popup(KeyCode::Esc);
            }
            KeybindingAction::CopyBranchName => {
                if self.mode == AppMode::SprintDetail {
                    if let Some(card_id) = self.sprint_detail_selected_card_id() {
                        if self.activate_card(card_id) {
                            self.copy_branch_name();
                        }
                    }
                } else {
                    self.copy_branch_name();
                }
            }
            KeybindingAction::CopyGitCheckoutCommand => {
                if self.mode == AppMode::SprintDetail {
                    if let Some(card_id) = self.sprint_detail_selected_card_id() {
                        if self.activate_card(card_id) {
                            self.copy_git_checkout_command();
                        }
                    }
                } else {
                    self.copy_git_checkout_command();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::DialogMode;
    use crate::keybindings::KeybindingAction;
    use crate::App;

    #[test]
    fn test_execute_action_confirm_prefix_collision_pops_dialog() {
        let mut app = App::test_default();
        app.open_dialog(DialogMode::ConfirmSprintPrefixCollision);

        app.execute_action(&KeybindingAction::ConfirmPrefixCollision);

        assert_ne!(
            app.mode,
            AppMode::Dialog(DialogMode::ConfirmSprintPrefixCollision),
            "ConfirmPrefixCollision must reach the same confirm behavior as pressing Enter/'y' directly"
        );
    }

    #[test]
    fn test_execute_action_reject_prefix_collision_reopens_prefix_dialog() {
        let mut app = App::test_default();
        app.open_dialog(DialogMode::ConfirmSprintPrefixCollision);

        app.execute_action(&KeybindingAction::RejectPrefixCollision);

        assert_eq!(
            app.mode,
            AppMode::Dialog(DialogMode::SetSprintPrefix),
            "RejectPrefixCollision must reach the same 'go back to prefix dialog' behavior as pressing 'n' directly"
        );
    }

    #[test]
    fn test_execute_action_cancel_prefix_collision_pops_dialog() {
        let mut app = App::test_default();
        app.open_dialog(DialogMode::ConfirmSprintPrefixCollision);

        app.execute_action(&KeybindingAction::CancelPrefixCollision);

        assert_ne!(
            app.mode,
            AppMode::Dialog(DialogMode::ConfirmSprintPrefixCollision),
            "CancelPrefixCollision must reach the same cancel behavior as pressing Esc directly"
        );
    }

    #[test]
    fn test_execute_action_force_overwrite_conflict_sets_pending_key() {
        let mut app = App::test_default();
        app.open_dialog(DialogMode::ConflictResolution);

        app.execute_action(&KeybindingAction::ForceOverwriteConflict);

        assert_eq!(
            app.pending_key,
            Some('o'),
            "ForceOverwriteConflict must reach the same behavior as pressing 'o' directly"
        );
        assert_ne!(app.mode, AppMode::Dialog(DialogMode::ConflictResolution));
    }

    #[test]
    fn test_execute_action_take_theirs_conflict_sets_pending_key() {
        let mut app = App::test_default();
        app.open_dialog(DialogMode::ConflictResolution);

        app.execute_action(&KeybindingAction::TakeTheirsConflict);

        assert_eq!(
            app.pending_key,
            Some('t'),
            "TakeTheirsConflict must reach the same behavior as pressing 't' directly"
        );
        assert_ne!(app.mode, AppMode::Dialog(DialogMode::ConflictResolution));
    }

    #[test]
    fn test_execute_action_cancel_conflict_resolution_pops_dialog() {
        let mut app = App::test_default();
        app.open_dialog(DialogMode::ConflictResolution);

        app.execute_action(&KeybindingAction::CancelConflictResolution);

        assert_ne!(
            app.mode,
            AppMode::Dialog(DialogMode::ConflictResolution),
            "CancelConflictResolution must reach the same cancel behavior (incl. clear_conflict()) as pressing Esc directly"
        );
    }

    #[test]
    fn test_execute_action_reload_discard_local_sets_pending_key() {
        let mut app = App::test_default();
        app.open_dialog(DialogMode::ExternalChangeDetected);

        app.execute_action(&KeybindingAction::ReloadDiscardLocal);

        assert_eq!(
            app.pending_key,
            Some('r'),
            "ReloadDiscardLocal must reach the same behavior as pressing 'r' directly"
        );
        assert_ne!(
            app.mode,
            AppMode::Dialog(DialogMode::ExternalChangeDetected)
        );
    }

    #[test]
    fn test_execute_action_keep_local_changes_pops_dialog() {
        let mut app = App::test_default();
        app.open_dialog(DialogMode::ExternalChangeDetected);

        app.execute_action(&KeybindingAction::KeepLocalChanges);

        assert_ne!(
            app.mode,
            AppMode::Dialog(DialogMode::ExternalChangeDetected),
            "KeepLocalChanges must reach the same behavior as pressing 'k' directly"
        );
    }

    #[test]
    fn test_execute_action_dismiss_external_change_pops_dialog() {
        let mut app = App::test_default();
        app.open_dialog(DialogMode::ExternalChangeDetected);

        app.execute_action(&KeybindingAction::DismissExternalChange);

        assert_ne!(
            app.mode,
            AppMode::Dialog(DialogMode::ExternalChangeDetected),
            "DismissExternalChange must reach the same behavior as pressing Esc directly"
        );
    }

    fn seed_board_with_card(app: &mut App) -> uuid::Uuid {
        use kanban_domain::{CreateCardOptions, KanbanOperations};
        let board = app.ctx.create_board("Board".into(), None).unwrap();
        let column = app
            .ctx
            .create_column(board.id, "TODO".into(), None)
            .unwrap();
        let card = app
            .ctx
            .create_card(
                board.id,
                column.id,
                "Task".into(),
                CreateCardOptions::default(),
            )
            .unwrap();
        app.prepare_frame();
        // copy_branch_name/copy_git_checkout_command resolve the board via
        // active_board_id, which real navigation always sets before either
        // CardDetail or SprintDetail is reached.
        app.selection.active_board_id = Some(board.id);
        card.id
    }

    #[test]
    fn test_execute_action_copy_branch_name_in_card_detail_reaches_copy() {
        let mut app = App::test_default();
        let card_id = seed_board_with_card(&mut app);
        app.mode = AppMode::CardDetail;
        app.selection.active_card_id = Some(card_id);

        app.execute_action(&KeybindingAction::CopyBranchName);

        assert!(
            app.ui_state.banner.is_some(),
            "CopyBranchName in CardDetail must reach the copy call (observable via a result banner)"
        );
    }

    #[test]
    fn test_execute_action_copy_git_checkout_command_in_card_detail_reaches_copy() {
        let mut app = App::test_default();
        let card_id = seed_board_with_card(&mut app);
        app.mode = AppMode::CardDetail;
        app.selection.active_card_id = Some(card_id);

        app.execute_action(&KeybindingAction::CopyGitCheckoutCommand);

        assert!(
            app.ui_state.banner.is_some(),
            "CopyGitCheckoutCommand in CardDetail must reach the copy call"
        );
    }

    #[test]
    fn test_execute_action_copy_branch_name_in_sprint_detail_reaches_copy() {
        use crate::app::sprint_view::SprintTaskPanel;
        let mut app = App::test_default();
        let card_id = seed_board_with_card(&mut app);
        app.sprint_view.panel = SprintTaskPanel::Uncompleted;
        app.sprint_view
            .uncompleted_component
            .update_cards(vec![card_id]);
        app.sprint_view
            .uncompleted_component
            .set_selected_index(Some(0));
        app.mode = AppMode::SprintDetail;

        app.execute_action(&KeybindingAction::CopyBranchName);

        assert_eq!(
            app.selection.active_card_id,
            Some(card_id),
            "CopyBranchName in SprintDetail must resolve and activate the panel's selected card"
        );
        assert!(
            app.ui_state.banner.is_some(),
            "CopyBranchName in SprintDetail must reach the copy call"
        );
    }

    #[test]
    fn test_execute_action_carry_over_opens_dialog_for_eligible_sprint() {
        use kanban_domain::{KanbanOperations, SprintStatus};
        let mut app = App::test_default();
        let board = app.ctx.create_board("Board".into(), None).unwrap();
        // A Planning sprint must exist for carry-over to have a target.
        app.ctx
            .create_sprint(board.id, None, Some("Next".into()))
            .unwrap();
        let completed = app
            .ctx
            .create_sprint(board.id, None, Some("Current".into()))
            .unwrap();
        app.ctx.activate_sprint(completed.id, None).unwrap();
        app.ctx.complete_sprint(completed.id).unwrap();
        app.prepare_frame();
        let sprint_idx = app
            .model
            .sprints()
            .iter()
            .position(|s| s.id == completed.id && s.status == SprintStatus::Completed)
            .expect("completed sprint present");
        app.selection.active_sprint_index = Some(sprint_idx);
        app.mode = AppMode::SprintDetail;

        app.execute_action(&KeybindingAction::CarryOver);

        assert_eq!(
            app.mode,
            AppMode::Dialog(DialogMode::CarryOverSprint),
            "CarryOver on an eligible (Completed) sprint must open the carry-over dialog"
        );
    }

    #[test]
    fn test_execute_action_export_boards_opens_dialog() {
        use kanban_domain::KanbanOperations;
        let mut app = App::test_default();
        app.ctx.create_board("Board".into(), None).unwrap();
        app.prepare_frame();
        app.mode = AppMode::Settings;

        app.execute_action(&KeybindingAction::ExportBoards);

        assert_eq!(
            app.mode,
            AppMode::Dialog(DialogMode::ExportBoards),
            "ExportBoards with a live board must open the export dialog"
        );
    }

    #[test]
    fn test_resolve_edit_card_dispatch_targets_card_list_when_edit_key_active() {
        let mut app = App::test_default();
        app.focus.active = Focus::Cards;
        app.mode = AppMode::Normal;

        assert_eq!(app.resolve_edit_card_dispatch(), EditCardDispatch::CardList);
    }

    #[test]
    fn test_resolve_edit_card_dispatch_targets_card_detail_in_card_detail_mode() {
        let mut app = App::test_default();
        let card_id = seed_board_with_card(&mut app);
        app.mode = AppMode::CardDetail;
        app.selection.active_card_id = Some(card_id);

        assert_eq!(
            app.resolve_edit_card_dispatch(),
            EditCardDispatch::CardDetail
        );
    }

    #[test]
    fn test_resolve_edit_card_dispatch_targets_sprint_detail_selected_card() {
        use crate::app::sprint_view::SprintTaskPanel;
        let mut app = App::test_default();
        let card_id = seed_board_with_card(&mut app);
        app.sprint_view.panel = SprintTaskPanel::Uncompleted;
        app.sprint_view
            .uncompleted_component
            .update_cards(vec![card_id]);
        app.sprint_view
            .uncompleted_component
            .set_selected_index(Some(0));
        app.mode = AppMode::SprintDetail;

        assert_eq!(
            app.resolve_edit_card_dispatch(),
            EditCardDispatch::SprintDetail(card_id)
        );
    }

    #[test]
    fn test_resolve_edit_card_dispatch_is_noop_in_sprint_detail_with_no_selection() {
        let mut app = App::test_default();
        app.mode = AppMode::SprintDetail;

        assert_eq!(app.resolve_edit_card_dispatch(), EditCardDispatch::Noop);
    }

    #[test]
    fn test_resolve_edit_card_dispatch_targets_settings_config_in_configuration_focus() {
        use crate::app::SettingsFocus;
        let mut app = App::test_default();
        app.mode = AppMode::Settings;
        app.focus.settings_focus = SettingsFocus::Configuration;

        assert_eq!(
            app.resolve_edit_card_dispatch(),
            EditCardDispatch::SettingsConfig
        );
    }

    #[test]
    fn test_resolve_edit_card_dispatch_is_noop_in_settings_storage_focus() {
        use crate::app::SettingsFocus;
        let mut app = App::test_default();
        app.mode = AppMode::Settings;
        app.focus.settings_focus = SettingsFocus::Storage;

        assert_eq!(app.resolve_edit_card_dispatch(), EditCardDispatch::Noop);
    }

    #[test]
    fn test_open_sprint_detail_card_for_edit_opens_card_detail_on_title() {
        let mut app = App::test_default();
        let card_id = seed_board_with_card(&mut app);
        app.mode = AppMode::SprintDetail;

        app.open_sprint_detail_card_for_edit(card_id);

        assert_eq!(app.mode, AppMode::CardDetail);
        assert_eq!(app.selection.active_card_id, Some(card_id));
        assert_eq!(app.focus.card_focus, crate::app::CardFocus::Title);
    }
}
