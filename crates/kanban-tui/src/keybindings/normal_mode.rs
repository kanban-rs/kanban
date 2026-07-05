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
                    "delete",
                    "Delete selected project",
                    KeybindingAction::DeleteBoard,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boards_provider_binds_d_to_delete_board() {
        // Mirrors the card removal flow where `d` is the primary removal key;
        // `D` is reserved for the archived-boards view (added with archival).
        let ctx = NormalModeBoardsProvider.get_context();
        let matches: Vec<_> = ctx.bindings.iter().filter(|b| b.key == "d").collect();
        assert_eq!(
            matches.len(),
            1,
            "exactly one 'd' binding on the boards panel"
        );
        assert_eq!(matches[0].action, KeybindingAction::DeleteBoard);
        assert!(
            ctx.bindings.iter().all(|b| b.key != "D"),
            "boards panel leaves 'D' free for the future archived-boards view"
        );
    }

    #[test]
    fn test_delete_board_action_is_distinct_from_delete_column() {
        assert_ne!(
            KeybindingAction::DeleteBoard,
            KeybindingAction::DeleteColumn
        );
    }
}
