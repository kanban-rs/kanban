use super::{App, AppMode};
use std::sync::{Arc, Mutex};

impl App {
    pub fn with_error_log<R>(&self, f: impl FnOnce(&crate::error_log::ErrorLogState) -> R) -> R {
        let log = self.error_log.lock().unwrap_or_else(|e| e.into_inner());
        f(&log)
    }

    pub fn with_error_log_mut<R>(
        &mut self,
        f: impl FnOnce(&mut crate::error_log::ErrorLogState) -> R,
    ) -> R {
        let mut log = self.error_log.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut log)
    }

    pub fn open_error_log(&mut self) {
        let entry_count = self.with_error_log_mut(|log| {
            log.has_unread_errors = false;
            log.unread_count = 0;
            log.entries.len()
        });
        self.ui_state.error_log_list.update_item_count(entry_count);
        self.ui_state.error_log_list.set_scroll_offset(0);
        self.push_mode(AppMode::ErrorLog);
    }

    pub fn set_error_log(&mut self, error_log: Arc<Mutex<crate::error_log::ErrorLogState>>) {
        self.error_log = error_log;
    }

    pub fn error_log_arc(&self) -> Arc<Mutex<crate::error_log::ErrorLogState>> {
        Arc::clone(&self.error_log)
    }

    pub(in crate::app) fn handle_error_log_mode(&mut self, key_code: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        match key_code {
            KeyCode::Esc | KeyCode::Char('q') => self.pop_mode(),
            KeyCode::Char('j') | KeyCode::Down => {
                self.ui_state.error_log_list.navigate_down();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.ui_state.error_log_list.navigate_up();
            }
            _ => {}
        }
    }
}
