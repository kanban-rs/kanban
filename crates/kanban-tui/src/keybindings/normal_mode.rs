use super::{Keybinding, KeybindingAction, KeybindingContext, KeybindingProvider};

pub struct NormalModeBoardsProvider;

impl KeybindingProvider for NormalModeBoardsProvider {
    fn get_context(&self) -> KeybindingContext {
        KeybindingContext::new(
            "Normal Mode - Projects Panel",
            vec![
                Keybinding::new("?", "help", "Show help", KeybindingAction::ShowHelp),
                Keybinding::new("q", "quit", "Quit application", KeybindingAction::Escape),
                Keybinding::new(
                    "1",
                    "panel 1",
                    "Focus projects panel",
                    KeybindingAction::FocusPanel(0),
                ),
                Keybinding::new(
                    "2",
                    "panel 2",
                    "Focus tasks panel",
                    KeybindingAction::FocusPanel(1),
                ),
                Keybinding::new(
                    "n",
                    "new",
                    "Create new project",
                    KeybindingAction::CreateBoard,
                ),
                Keybinding::new(
                    "r",
                    "rename",
                    "Rename selected project",
                    KeybindingAction::RenameBoard,
                ),
                Keybinding::new(
                    "e",
                    "edit",
                    "Edit selected project",
                    KeybindingAction::EditBoard,
                ),
                Keybinding::new(
                    "x",
                    "export",
                    "Export selected project",
                    KeybindingAction::ExportBoard,
                ),
                Keybinding::new(
                    "X",
                    "export all",
                    "Export all projects",
                    KeybindingAction::ExportAll,
                ),
                Keybinding::new(
                    "i",
                    "import",
                    "Import project from file",
                    KeybindingAction::ImportBoard,
                ),
                Keybinding::new(
                    "d",
                    "archive",
                    "Archive selected project",
                    KeybindingAction::DeleteBoard,
                ),
                Keybinding::new(
                    "D",
                    "archived",
                    "View archived projects",
                    KeybindingAction::ToggleArchivedBoardsView,
                ),
                Keybinding::new(
                    "j/↓",
                    "down",
                    "Navigate down",
                    KeybindingAction::NavigateDown,
                ),
                Keybinding::new("k/↑", "up", "Navigate up", KeybindingAction::NavigateUp),
                Keybinding::new("gg", "top", "Jump to top", KeybindingAction::JumpToTop),
                Keybinding::new(
                    "G",
                    "bottom",
                    "Jump to bottom",
                    KeybindingAction::JumpToBottom,
                ),
                Keybinding::new(
                    "Enter/Space",
                    "detail",
                    "View project detail",
                    KeybindingAction::SelectItem,
                ),
                Keybinding::new("u", "undo", "Undo last action", KeybindingAction::Undo),
                Keybinding::new(
                    "U",
                    "redo",
                    "Redo last undone action",
                    KeybindingAction::Redo,
                ),
                Keybinding::new(
                    "S",
                    "settings",
                    "Open settings view",
                    KeybindingAction::OpenSettings,
                ),
            ],
        )
    }
}

pub struct ArchivedCardsViewProvider;

impl KeybindingProvider for ArchivedCardsViewProvider {
    fn get_context(&self) -> KeybindingContext {
        KeybindingContext::new(
            "Archived Cards View",
            vec![
                Keybinding::new("?", "help", "Show help", KeybindingAction::ShowHelp),
                Keybinding::new(
                    "j/↓",
                    "down",
                    "Navigate down",
                    KeybindingAction::NavigateDown,
                ),
                Keybinding::new("k/↑", "up", "Navigate up", KeybindingAction::NavigateUp),
                Keybinding::new("gg", "top", "Jump to top", KeybindingAction::JumpToTop),
                Keybinding::new(
                    "G",
                    "bottom",
                    "Jump to bottom",
                    KeybindingAction::JumpToBottom,
                ),
                Keybinding::new(
                    "{",
                    "half up",
                    "Jump half viewport up",
                    KeybindingAction::JumpHalfViewportUp,
                ),
                Keybinding::new(
                    "}",
                    "half down",
                    "Jump half viewport down",
                    KeybindingAction::JumpHalfViewportDown,
                ),
                Keybinding::new(
                    "r",
                    "restore",
                    "Restore selected task(s)",
                    KeybindingAction::RestoreCard,
                ),
                Keybinding::new(
                    "x",
                    "delete",
                    "Delete selected task(s)",
                    KeybindingAction::DeleteCard,
                ),
                Keybinding::new(
                    "v",
                    "select",
                    "Select task for bulk operation",
                    KeybindingAction::ToggleCardSelection,
                ),
                Keybinding::new(
                    "V",
                    "view",
                    "Toggle task list view",
                    KeybindingAction::ToggleTaskListView,
                ),
                Keybinding::new(
                    "q/Esc",
                    "back",
                    "Back to normal view",
                    KeybindingAction::Escape,
                ),
                Keybinding::new("u", "undo", "Undo last action", KeybindingAction::Undo),
                Keybinding::new(
                    "U",
                    "redo",
                    "Redo last undone action",
                    KeybindingAction::Redo,
                ),
            ],
        )
    }
}

pub struct ArchivedBoardsViewProvider;

impl KeybindingProvider for ArchivedBoardsViewProvider {
    fn get_context(&self) -> KeybindingContext {
        KeybindingContext::new(
            "Archived Projects View",
            vec![
                Keybinding::new("?", "help", "Show help", KeybindingAction::ShowHelp),
                Keybinding::new(
                    "j/↓",
                    "down",
                    "Navigate down",
                    KeybindingAction::NavigateDown,
                ),
                Keybinding::new("k/↑", "up", "Navigate up", KeybindingAction::NavigateUp),
                Keybinding::new("gg", "top", "Jump to top", KeybindingAction::JumpToTop),
                Keybinding::new(
                    "G",
                    "bottom",
                    "Jump to bottom",
                    KeybindingAction::JumpToBottom,
                ),
                Keybinding::new(
                    "r",
                    "restore",
                    "Restore selected project",
                    KeybindingAction::RestoreBoard,
                ),
                Keybinding::new(
                    "x",
                    "delete",
                    "Permanently delete selected project",
                    KeybindingAction::DeleteArchivedBoard,
                ),
                Keybinding::new(
                    "q/Esc",
                    "back",
                    "Back to projects view",
                    KeybindingAction::Escape,
                ),
                Keybinding::new("u", "undo", "Undo last action", KeybindingAction::Undo),
                Keybinding::new(
                    "U",
                    "redo",
                    "Redo last undone action",
                    KeybindingAction::Redo,
                ),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boards_provider_binds_d_to_remove_and_shift_d_to_archived_view() {
        // Mirrors the card removal flow where `d` is the primary removal key
        // (now archive) and `D` toggles the archived-boards view.
        let ctx = NormalModeBoardsProvider.get_context();
        let matches: Vec<_> = ctx.bindings.iter().filter(|b| b.key == "d").collect();
        assert_eq!(
            matches.len(),
            1,
            "exactly one 'd' binding on the boards panel"
        );
        assert_eq!(matches[0].action, KeybindingAction::DeleteBoard);
        let shift_d: Vec<_> = ctx.bindings.iter().filter(|b| b.key == "D").collect();
        assert_eq!(
            shift_d.len(),
            1,
            "exactly one 'D' binding on the boards panel"
        );
        assert_eq!(
            shift_d[0].action,
            KeybindingAction::ToggleArchivedBoardsView,
            "'D' toggles the archived-boards view"
        );
    }

    #[test]
    fn test_archived_boards_view_provider_binds_restore_and_delete() {
        let ctx = ArchivedBoardsViewProvider.get_context();
        assert!(ctx
            .bindings
            .iter()
            .any(|b| b.key == "r" && b.action == KeybindingAction::RestoreBoard));
        assert!(ctx
            .bindings
            .iter()
            .any(|b| b.key == "x" && b.action == KeybindingAction::DeleteArchivedBoard));
    }

    #[test]
    fn test_delete_board_action_is_distinct_from_delete_column() {
        assert_ne!(
            KeybindingAction::DeleteBoard,
            KeybindingAction::DeleteColumn
        );
    }
}
