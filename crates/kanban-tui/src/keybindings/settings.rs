use super::{Keybinding, KeybindingAction, KeybindingContext, KeybindingProvider};
use crate::app::SettingsFocus;

pub struct SettingsViewProvider {
    focus: SettingsFocus,
}

impl SettingsViewProvider {
    pub fn new(focus: SettingsFocus) -> Self {
        Self { focus }
    }
}

impl KeybindingProvider for SettingsViewProvider {
    fn get_context(&self) -> KeybindingContext {
        let mut bindings = vec![
            Keybinding::new(
                "1",
                "config",
                "Focus Configuration",
                KeybindingAction::FocusPanel(0),
            ),
            Keybinding::new(
                "2",
                "file",
                "Focus Config File",
                KeybindingAction::FocusPanel(1),
            ),
            Keybinding::new(
                "3",
                "storage",
                "Focus Storage",
                KeybindingAction::FocusPanel(2),
            ),
            Keybinding::new(
                "j/k",
                "navigate",
                "Navigate items",
                KeybindingAction::NavigateDown,
            ),
            Keybinding::new(
                "h/l",
                "columns",
                "Switch columns",
                KeybindingAction::NavigateLeft,
            ),
        ];

        match self.focus {
            SettingsFocus::Configuration => {
                bindings.push(Keybinding::new(
                    "e/Enter",
                    "edit",
                    "Edit configuration in external editor (Enter on Background Highlight edits it inline)",
                    KeybindingAction::EditCard,
                ));
            }
            SettingsFocus::Storage => {
                bindings.push(Keybinding::new(
                    "Enter",
                    "select",
                    "Activate selected item",
                    KeybindingAction::SelectItem,
                ));
            }
            _ => {}
        }

        bindings.push(Keybinding::new(
            "x",
            "export",
            "Export boards",
            KeybindingAction::ExportBoards,
        ));
        bindings.push(Keybinding::new(
            "q/Esc",
            "back",
            "Back to normal view",
            KeybindingAction::Escape,
        ));

        KeybindingContext::new("Settings", bindings)
    }
}
