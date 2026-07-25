use super::{App, AppMode, Focus};

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
            KeybindingAction::CarryOver => {}
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
            KeybindingAction::ExportBoards => {}
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
}
