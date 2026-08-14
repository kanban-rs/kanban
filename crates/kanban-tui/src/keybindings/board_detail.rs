use super::{Keybinding, KeybindingAction, KeybindingContext, KeybindingProvider};
use crate::app::BoardFocus;

pub struct BoardDetailProvider {
    focus: BoardFocus,
}

impl BoardDetailProvider {
    pub fn new(focus: BoardFocus) -> Self {
        Self { focus }
    }
}

impl KeybindingProvider for BoardDetailProvider {
    fn get_context(&self) -> KeybindingContext {
        let focus_name = match self.focus {
            BoardFocus::Name => "Name",
            BoardFocus::Description => "Description",
            BoardFocus::Settings => "Settings",
            BoardFocus::Sprints => "Sprints",
            BoardFocus::Columns => "Columns",
        };

        let mut bindings = vec![
            Keybinding::new("?", "help", "Show help", KeybindingAction::ShowHelp),
            Keybinding::new(
                "q",
                "quit",
                "Exit project detail view",
                KeybindingAction::Escape,
            ),
            Keybinding::new(
                "ESC",
                "back",
                "Return to project list",
                KeybindingAction::Escape,
            ),
            Keybinding::new(
                "1",
                "panel 1",
                "Focus project name panel",
                KeybindingAction::FocusPanel(0),
            ),
            Keybinding::new(
                "2",
                "panel 2",
                "Focus description panel",
                KeybindingAction::FocusPanel(1),
            ),
            Keybinding::new(
                "3",
                "panel 3",
                "Focus settings panel",
                KeybindingAction::FocusPanel(2),
            ),
            Keybinding::new(
                "4",
                "panel 4",
                "Focus sprints panel",
                KeybindingAction::FocusPanel(3),
            ),
            Keybinding::new(
                "5",
                "panel 5",
                "Focus columns panel",
                KeybindingAction::FocusPanel(4),
            ),
            Keybinding::new(
                "e",
                "edit",
                "Edit current panel",
                KeybindingAction::EditBoard,
            ),
            Keybinding::new(
                "p",
                "prefix",
                "Set branch prefix",
                KeybindingAction::EditBoard,
            ),
            Keybinding::new("u", "undo", "Undo last action", KeybindingAction::Undo),
            Keybinding::new(
                "U",
                "redo",
                "Redo last undone action",
                KeybindingAction::Redo,
            ),
        ];

        match self.focus {
            BoardFocus::Sprints => {
                bindings.extend(vec![
                    Keybinding::new(
                        "n",
                        "new",
                        "Create new sprint",
                        KeybindingAction::CreateSprint,
                    ),
                    Keybinding::new(
                        "j/↓",
                        "down",
                        "Navigate down",
                        KeybindingAction::NavigateDown,
                    ),
                    Keybinding::new("k/↑", "up", "Navigate up", KeybindingAction::NavigateUp),
                    Keybinding::new(
                        "Enter/Space",
                        "detail",
                        "Open sprint detail",
                        KeybindingAction::SelectItem,
                    ),
                ]);
            }
            BoardFocus::Columns => {
                bindings.extend(vec![
                    Keybinding::new(
                        "n",
                        "new",
                        "Create new column",
                        KeybindingAction::CreateColumn,
                    ),
                    Keybinding::new(
                        "r",
                        "rename",
                        "Rename selected column",
                        KeybindingAction::RenameColumn,
                    ),
                    Keybinding::new(
                        "d",
                        "delete",
                        "Delete selected column",
                        KeybindingAction::DeleteColumn,
                    ),
                    Keybinding::new(
                        "s",
                        "default status",
                        "Set default status for selected column",
                        KeybindingAction::SetColumnDefaultStatus,
                    ),
                    Keybinding::new(
                        "j/↓",
                        "down",
                        "Navigate down",
                        KeybindingAction::NavigateDown,
                    ),
                    Keybinding::new("k/↑", "up", "Navigate up", KeybindingAction::NavigateUp),
                    Keybinding::new(
                        "J",
                        "move up",
                        "Reorder column up",
                        KeybindingAction::MoveColumnUp,
                    ),
                    Keybinding::new(
                        "K",
                        "move down",
                        "Reorder column down",
                        KeybindingAction::MoveColumnDown,
                    ),
                ]);
            }
            _ => {}
        }

        KeybindingContext::new(format!("Project Detail - {} Panel", focus_name), bindings)
    }
}
