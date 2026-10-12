use super::settings_handlers::BACKGROUND_HIGHLIGHT_CONFIG_INDEX;
use crate::app::{App, DialogMode, SettingsFocus};
use crossterm::event::KeyCode;

const BACKGROUND_HIGHLIGHT_INPUT_MAX_LEN: usize = 7;

impl App {
    pub fn settings_background_highlight_row_selected(&self) -> bool {
        self.focus.settings_focus == SettingsFocus::Configuration
            && self.selection.settings_config.get() == Some(BACKGROUND_HIGHLIGHT_CONFIG_INDEX)
    }

    pub fn open_background_highlight_editor(&mut self) {
        let current = self
            .app_config
            .background_highlight_color()
            .map(|c| c.to_string())
            .unwrap_or_default();
        self.input.set(current);
        self.open_dialog(DialogMode::EditBackgroundHighlight);
    }

    pub fn handle_edit_background_highlight_dialog(&mut self, key_code: KeyCode) {
        if let KeyCode::Char(c) = key_code {
            let is_hex_char = c == '#' || c.is_ascii_hexdigit();
            if !is_hex_char || self.input.as_str().len() >= BACKGROUND_HIGHLIGHT_INPUT_MAX_LEN {
                return;
            }
        }
        match crate::dialog::handle_dialog_input(&mut self.input, key_code, true) {
            crate::dialog::DialogAction::Cancel => {
                self.input.clear();
                self.pop_mode();
            }
            crate::dialog::DialogAction::Confirm => self.commit_background_highlight(),
            crate::dialog::DialogAction::None => {}
        }
    }

    fn commit_background_highlight(&mut self) {
        let typed = self.input.as_str();
        let color = if typed.is_empty() {
            None
        } else {
            match typed.parse::<kanban_core::HexColor>() {
                Ok(color) => Some(color),
                Err(_) => return,
            }
        };
        let mut config = self.app_config.clone();
        config.background_highlight = color.map(|c| c.to_string());
        self.set_app_config(config);
        self.input.clear();
        self.pop_mode();
        if let Err(e) = kanban_service::config::save_background_highlight(&self.app_config, color) {
            tracing::error!("Failed to save background highlight: {}", e);
            self.set_error(format!("Failed to save background highlight: {}", e));
        }
    }
}
