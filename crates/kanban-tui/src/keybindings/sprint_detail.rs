use super::{Keybinding, KeybindingAction, KeybindingContext, KeybindingProvider};

pub struct SprintDetailProvider;

impl KeybindingProvider for SprintDetailProvider {
    fn get_context(&self) -> KeybindingContext {
        KeybindingContext::new(
            "Sprint Detail",
            vec![
                Keybinding::new("?", "help", "Show help", KeybindingAction::ShowHelp),
                Keybinding::new(
                    "q",
                    "quit",
                    "Exit sprint detail view",
                    KeybindingAction::Escape,
                ),
                Keybinding::new(
                    "ESC",
                    "back",
                    "Return to project detail",
                    KeybindingAction::Escape,
                ),
                Keybinding::new(
                    "a",
                    "activate",
                    "Activate this sprint",
                    KeybindingAction::EditBoard,
                ),
                Keybinding::new(
                    "c",
                    "complete",
                    "Complete this sprint",
                    KeybindingAction::ToggleCompletion,
                ),
                Keybinding::new(
                    "p",
                    "s-prefix",
                    "Set sprint prefix",
                    KeybindingAction::EditBoard,
                ),
                Keybinding::new(
                    "C",
                    "c-prefix",
                    "Set card prefix override",
                    KeybindingAction::EditBoard,
                ),
                Keybinding::new(
                    "o",
                    "sort",
                    "Sort tasks by field",
                    KeybindingAction::OrderCards,
                ),
                Keybinding::new(
                    "O",
                    "toggle order",
                    "Toggle sort order",
                    KeybindingAction::ToggleSortOrder,
                ),
                Keybinding::new(
                    "h",
                    "left panel",
                    "Switch to uncompleted panel",
                    KeybindingAction::NavigateLeft,
                ),
                Keybinding::new(
                    "l",
                    "right panel",
                    "Switch to completed panel",
                    KeybindingAction::NavigateRight,
                ),
                Keybinding::new(
                    "j/↓",
                    "down",
                    "Navigate down",
                    KeybindingAction::NavigateDown,
                ),
                Keybinding::new("k/↑", "up", "Navigate up", KeybindingAction::NavigateUp),
                Keybinding::new(
                    "v",
                    "select",
                    "Select task for bulk operation",
                    KeybindingAction::ToggleCardSelection,
                ),
                Keybinding::new("n", "new", "Create new task", KeybindingAction::CreateCard),
                Keybinding::new(
                    "e",
                    "edit",
                    "Edit selected task",
                    KeybindingAction::EditCard,
                ),
                Keybinding::new(
                    "s",
                    "assign",
                    "Assign task to sprint",
                    KeybindingAction::AssignToSprint,
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
                    "M",
                    "move all",
                    "Move all uncompleted tasks to a planning sprint",
                    KeybindingAction::CarryOver,
                ),
                Keybinding::new(
                    "d",
                    "archive",
                    "Archive selected task(s)",
                    KeybindingAction::ArchiveCard,
                ),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sprint_detail_provider_advertises_d_archive() {
        let context = SprintDetailProvider.get_context();
        let d_binding = context.bindings.iter().find(|b| b.key == "d").expect(
            "SprintDetailProvider must advertise 'd', which actually archives selected tasks",
        );
        assert_eq!(d_binding.action, KeybindingAction::ArchiveCard);
    }
}
