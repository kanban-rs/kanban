use super::{Keybinding, KeybindingAction, KeybindingContext, KeybindingProvider};

pub struct SearchModeProvider;

impl KeybindingProvider for SearchModeProvider {
    fn get_context(&self) -> KeybindingContext {
        KeybindingContext::new(
            "Search Mode",
            vec![
                Keybinding::new(
                    "ESC",
                    "clear",
                    "Clear search and return",
                    KeybindingAction::Escape,
                ),
                Keybinding::new(
                    "Enter",
                    "apply",
                    "Apply filter and browse results",
                    KeybindingAction::SelectItem,
                ),
                Keybinding::new(
                    "Type",
                    "query",
                    "Enter search query",
                    KeybindingAction::Search,
                ),
                Keybinding::new(
                    "n/N",
                    "navigate",
                    "Next/previous hit (after applying)",
                    KeybindingAction::NavigateDown,
                ),
            ],
        )
    }
}

pub struct DialogInputProvider {
    dialog_name: String,
}

impl DialogInputProvider {
    pub fn new(dialog_name: impl Into<String>) -> Self {
        Self {
            dialog_name: dialog_name.into(),
        }
    }
}

impl KeybindingProvider for DialogInputProvider {
    fn get_context(&self) -> KeybindingContext {
        KeybindingContext::new(
            format!("{} - Input Dialog", self.dialog_name),
            vec![
                Keybinding::new(
                    "ESC",
                    "cancel",
                    "Cancel and close dialog",
                    KeybindingAction::Escape,
                ),
                Keybinding::new(
                    "Enter",
                    "confirm",
                    "Confirm and apply",
                    KeybindingAction::SelectItem,
                ),
                Keybinding::new("Type", "input", "Enter text", KeybindingAction::EditCard),
                Keybinding::new(
                    "Backspace",
                    "delete",
                    "Delete previous character",
                    KeybindingAction::EditCard,
                ),
                Keybinding::new(
                    "←/→",
                    "move",
                    "Move cursor left/right",
                    KeybindingAction::NavigateLeft,
                ),
                Keybinding::new(
                    "Home/End",
                    "jump",
                    "Jump to start/end of line",
                    KeybindingAction::NavigateLeft,
                ),
            ],
        )
    }
}

pub struct DialogSelectionProvider {
    dialog_name: String,
}

impl DialogSelectionProvider {
    pub fn new(dialog_name: impl Into<String>) -> Self {
        Self {
            dialog_name: dialog_name.into(),
        }
    }
}

impl KeybindingProvider for DialogSelectionProvider {
    fn get_context(&self) -> KeybindingContext {
        KeybindingContext::new(
            format!("{} - Selection Dialog", self.dialog_name),
            vec![
                Keybinding::new(
                    "ESC",
                    "cancel",
                    "Cancel and close dialog",
                    KeybindingAction::Escape,
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
                    "select",
                    "Select and confirm",
                    KeybindingAction::SelectItem,
                ),
            ],
        )
    }
}

pub struct DeleteConfirmProvider {
    what: String,
}

impl DeleteConfirmProvider {
    pub fn new(what: impl Into<String>) -> Self {
        Self { what: what.into() }
    }
}

impl KeybindingProvider for DeleteConfirmProvider {
    fn get_context(&self) -> KeybindingContext {
        KeybindingContext::new(
            format!("Delete {} - Confirm", self.what),
            vec![
                Keybinding::new("ESC", "cancel", "Cancel deletion", KeybindingAction::Escape),
                Keybinding::new("n", "no", "Do not delete", KeybindingAction::Escape),
                Keybinding::new("y", "yes", "Confirm deletion", KeybindingAction::SelectItem),
                Keybinding::new(
                    "Enter",
                    "yes",
                    "Confirm deletion",
                    KeybindingAction::SelectItem,
                ),
            ],
        )
    }
}

pub struct ErrorLogProvider;

impl KeybindingProvider for ErrorLogProvider {
    fn get_context(&self) -> KeybindingContext {
        KeybindingContext::new(
            "Error Log",
            vec![
                Keybinding::new(
                    "ESC/q",
                    "close",
                    "Close error log",
                    KeybindingAction::Escape,
                ),
                Keybinding::new(
                    "j/k",
                    "scroll",
                    "Scroll up/down",
                    KeybindingAction::NavigateDown,
                ),
            ],
        )
    }
}

pub struct FilterOptionsProvider;

impl KeybindingProvider for FilterOptionsProvider {
    fn get_context(&self) -> KeybindingContext {
        KeybindingContext::new(
            "Filter Options",
            vec![
                Keybinding::new(
                    "ESC",
                    "cancel",
                    "Cancel and close filters",
                    KeybindingAction::Escape,
                ),
                Keybinding::new(
                    "j/↓",
                    "down",
                    "Navigate down",
                    KeybindingAction::NavigateDown,
                ),
                Keybinding::new("k/↑", "up", "Navigate up", KeybindingAction::NavigateUp),
                Keybinding::new(
                    "Space",
                    "toggle",
                    "Toggle filter option",
                    KeybindingAction::ToggleFilter,
                ),
                Keybinding::new(
                    "Enter",
                    "apply",
                    "Apply selected filters",
                    KeybindingAction::SelectItem,
                ),
            ],
        )
    }
}

pub struct ConfirmSprintPrefixCollisionProvider;

impl KeybindingProvider for ConfirmSprintPrefixCollisionProvider {
    fn get_context(&self) -> KeybindingContext {
        KeybindingContext::new(
            "Confirm Sprint Prefix",
            vec![
                Keybinding::new(
                    "y",
                    "yes",
                    "Use this prefix anyway",
                    KeybindingAction::ConfirmPrefixCollision,
                ),
                Keybinding::new(
                    "Enter",
                    "yes",
                    "Use this prefix anyway",
                    KeybindingAction::ConfirmPrefixCollision,
                ),
                Keybinding::new(
                    "n",
                    "no",
                    "Go back and choose a different prefix",
                    KeybindingAction::RejectPrefixCollision,
                ),
                Keybinding::new(
                    "ESC",
                    "cancel",
                    "Cancel",
                    KeybindingAction::CancelPrefixCollision,
                ),
            ],
        )
    }
}

pub struct ConflictResolutionProvider;

impl KeybindingProvider for ConflictResolutionProvider {
    fn get_context(&self) -> KeybindingContext {
        KeybindingContext::new(
            "Resolve Conflict",
            vec![
                Keybinding::new(
                    "o",
                    "overwrite",
                    "Keep your changes and overwrite the file on disk",
                    KeybindingAction::ForceOverwriteConflict,
                ),
                Keybinding::new(
                    "t",
                    "take theirs",
                    "Discard your changes and reload from disk",
                    KeybindingAction::TakeTheirsConflict,
                ),
                Keybinding::new(
                    "ESC",
                    "retry later",
                    "Cancel and decide again later",
                    KeybindingAction::CancelConflictResolution,
                ),
            ],
        )
    }
}

pub struct ExternalChangeDetectedProvider;

impl KeybindingProvider for ExternalChangeDetectedProvider {
    fn get_context(&self) -> KeybindingContext {
        KeybindingContext::new(
            "External Change",
            vec![
                Keybinding::new(
                    "r",
                    "reload",
                    "Reload from disk (discards your local changes)",
                    KeybindingAction::ReloadDiscardLocal,
                ),
                Keybinding::new(
                    "k",
                    "keep local",
                    "Keep local changes (discards the external write)",
                    KeybindingAction::KeepLocalChanges,
                ),
                Keybinding::new(
                    "ESC",
                    "dismiss",
                    "Dismiss and continue with current state",
                    KeybindingAction::DismissExternalChange,
                ),
            ],
        )
    }
}

#[cfg(test)]
mod confirm_dialog_tests {
    use super::*;

    #[test]
    fn test_confirm_sprint_prefix_collision_provider_advertises_real_keys_not_list_nav() {
        let context = ConfirmSprintPrefixCollisionProvider.get_context();
        let keys: Vec<&str> = context.bindings.iter().map(|b| b.key.as_str()).collect();
        assert!(!keys.contains(&"j/↓"), "must not advertise dead list-nav 'j'");
        assert!(!keys.contains(&"k/↑"), "must not advertise dead list-nav 'k'");
        assert!(keys.contains(&"y"), "must advertise the real confirm key 'y'");
        assert!(keys.contains(&"n"), "must advertise the real reject key 'n'");

        let y_binding = context.bindings.iter().find(|b| b.key == "y").unwrap();
        assert_eq!(y_binding.action, KeybindingAction::ConfirmPrefixCollision);
        let n_binding = context.bindings.iter().find(|b| b.key == "n").unwrap();
        assert_eq!(n_binding.action, KeybindingAction::RejectPrefixCollision);
        let esc_binding = context.bindings.iter().find(|b| b.key == "ESC").unwrap();
        assert_eq!(esc_binding.action, KeybindingAction::CancelPrefixCollision);
    }

    #[test]
    fn test_conflict_resolution_provider_advertises_real_keys_not_list_nav() {
        let context = ConflictResolutionProvider.get_context();
        let keys: Vec<&str> = context.bindings.iter().map(|b| b.key.as_str()).collect();
        assert!(!keys.contains(&"j/↓"), "must not advertise dead list-nav 'j'");
        assert!(!keys.contains(&"k/↑"), "must not advertise dead list-nav 'k'");
        assert!(
            !keys.contains(&"Enter/Space"),
            "must not advertise dead 'Enter' (nothing is selectable in this dialog)"
        );
        assert!(keys.contains(&"o"), "must advertise the real force-overwrite key 'o'");
        assert!(keys.contains(&"t"), "must advertise the real take-theirs key 't'");

        let o_binding = context.bindings.iter().find(|b| b.key == "o").unwrap();
        assert_eq!(o_binding.action, KeybindingAction::ForceOverwriteConflict);
        let t_binding = context.bindings.iter().find(|b| b.key == "t").unwrap();
        assert_eq!(t_binding.action, KeybindingAction::TakeTheirsConflict);
        let esc_binding = context.bindings.iter().find(|b| b.key == "ESC").unwrap();
        assert_eq!(esc_binding.action, KeybindingAction::CancelConflictResolution);
    }

    #[test]
    fn test_external_change_detected_provider_advertises_real_keys_not_list_nav() {
        let context = ExternalChangeDetectedProvider.get_context();
        let keys: Vec<&str> = context.bindings.iter().map(|b| b.key.as_str()).collect();
        assert!(!keys.contains(&"j/↓"), "must not advertise dead list-nav 'j'");
        assert!(
            !keys.contains(&"Enter/Space"),
            "must not advertise dead 'Enter' (nothing is selectable in this dialog)"
        );
        assert!(keys.contains(&"r"), "must advertise the real reload key 'r'");
        assert!(keys.contains(&"k"), "must advertise the real keep-local key 'k'");

        let r_binding = context.bindings.iter().find(|b| b.key == "r").unwrap();
        assert_eq!(r_binding.action, KeybindingAction::ReloadDiscardLocal);
        let k_binding = context.bindings.iter().find(|b| b.key == "k").unwrap();
        assert_eq!(k_binding.action, KeybindingAction::KeepLocalChanges);
        // The 'k' description must make the destructive discard explicit, not
        // imply harmless list navigation (the exact bug this card fixes).
        assert!(
            k_binding.description.to_lowercase().contains("discard"),
            "'k' must be described as discarding the external write, not 'up': {}",
            k_binding.description
        );
        let esc_binding = context.bindings.iter().find(|b| b.key == "ESC").unwrap();
        assert_eq!(esc_binding.action, KeybindingAction::DismissExternalChange);
    }
}
