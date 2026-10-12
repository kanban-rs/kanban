use super::settings_handlers::BACKGROUND_HIGHLIGHT_CONFIG_INDEX;
use crate::app::{App, DialogMode, SettingsFocus};
use crossterm::event::KeyCode;

impl App {
    pub fn settings_background_highlight_row_selected(&self) -> bool {
        self.focus.settings_focus == SettingsFocus::Configuration
            && self.selection.settings_config.get() == Some(BACKGROUND_HIGHLIGHT_CONFIG_INDEX)
    }

    pub fn open_background_highlight_editor(&mut self) {
        self.open_dialog(DialogMode::EditBackgroundHighlight);
    }

    pub fn handle_edit_background_highlight_dialog(&mut self, key_code: KeyCode) {
        let _ = key_code;
    }
}
