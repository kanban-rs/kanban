use super::App;
use crate::components::Banner;

impl App {
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.ui_state.banner = Some(Banner::error(message));
    }

    pub fn set_success(&mut self, message: impl Into<String>) {
        self.ui_state.banner = Some(Banner::success(message));
    }

    pub fn clear_banner(&mut self) {
        self.ui_state.banner = None;
    }

    pub fn set_save_error(&mut self, message: String) {
        self.save_error = Some(message);
    }

    pub fn clear_save_error(&mut self) {
        self.save_error = None;
    }
}
