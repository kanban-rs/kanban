use super::{Keybinding, KeybindingAction, KeybindingContext, KeybindingProvider};
use crate::app::CardFocus;

pub struct CardDetailProvider {
    focus: CardFocus,
}

impl CardDetailProvider {
    pub fn new(focus: CardFocus) -> Self {
        Self { focus }
    }
}

impl KeybindingProvider for CardDetailProvider {
    fn get_context(&self) -> KeybindingContext {
        let focus_name = match self.focus {
            CardFocus::Title => "Title",
            CardFocus::Metadata => "Metadata",
            CardFocus::Description => "Description",
            CardFocus::Parents => "Parents",
            CardFocus::Children => "Children",
        };

        let mut bindings = vec![
            Keybinding::new("?", "help", "Show help", KeybindingAction::ShowHelp),
            Keybinding::new(
                "q",
                "quit",
                "Exit card detail view",
                KeybindingAction::Escape,
            ),
            Keybinding::new(
                "ESC",
                "back",
                "Return to task list",
                KeybindingAction::Escape,
            ),
            Keybinding::new(
                "1",
                "panel 1",
                "Focus task title panel",
                KeybindingAction::FocusPanel(0),
            ),
            Keybinding::new(
                "2",
                "panel 2",
                "Focus metadata panel",
                KeybindingAction::FocusPanel(1),
            ),
            Keybinding::new(
                "3",
                "panel 3",
                "Focus description panel",
                KeybindingAction::FocusPanel(2),
            ),
            Keybinding::new(
                "4",
                "panel 4",
                "Focus parents panel",
                KeybindingAction::FocusPanel(3),
            ),
            Keybinding::new(
                "5",
                "panel 5",
                "Focus children panel",
                KeybindingAction::FocusPanel(4),
            ),
        ];

        // Only show edit keybinding for editable panels
        match self.focus {
            CardFocus::Title | CardFocus::Metadata | CardFocus::Description => {
                bindings.push(Keybinding::new(
                    "e",
                    "edit",
                    "Edit current panel",
                    KeybindingAction::EditCard,
                ));
            }
            CardFocus::Parents => {
                bindings.push(Keybinding::new(
                    "r",
                    "set parents",
                    "Manage parent cards",
                    KeybindingAction::ManageParents,
                ));
            }
            CardFocus::Children => {
                bindings.push(Keybinding::new(
                    "R",
                    "set children",
                    "Manage child cards",
                    KeybindingAction::ManageChildren,
                ));
            }
        }

        // Always show these bindings
        bindings.extend([
            Keybinding::new(
                "r",
                "set parents",
                "Manage parent cards",
                KeybindingAction::ManageParents,
            ),
            Keybinding::new(
                "R",
                "set children",
                "Manage child cards",
                KeybindingAction::ManageChildren,
            ),
            Keybinding::new(
                "d",
                "delete",
                "Delete this task",
                KeybindingAction::DeleteCard,
            ),
            Keybinding::new(
                "y",
                "copy branch",
                "Copy branch name to clipboard",
                KeybindingAction::CopyBranchName,
            ),
            Keybinding::new(
                "Y",
                "copy cmd",
                "Copy git checkout command",
                KeybindingAction::CopyGitCheckoutCommand,
            ),
            Keybinding::new(
                "a",
                "assign",
                "Assign task to sprint",
                KeybindingAction::AssignToSprint,
            ),
            Keybinding::new("u", "undo", "Undo last action", KeybindingAction::Undo),
            Keybinding::new(
                "U",
                "redo",
                "Redo last undone action",
                KeybindingAction::Redo,
            ),
        ]);

        KeybindingContext::new(format!("Card Detail - {} Panel", focus_name), bindings)
    }
}
