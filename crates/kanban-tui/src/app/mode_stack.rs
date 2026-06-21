use super::{App, AppMode, MigrationState};

impl App {
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn handle_quit_key(&mut self) {
        let needs_pending_confirm =
            self.ctx.save_coordinator.has_pending_saves() && !self.quit_with_pending;
        let needs_migration_confirm =
            matches!(self.migration_state, MigrationState::Migrating { .. })
                && !self.quit_with_migration;

        if needs_pending_confirm || needs_migration_confirm {
            if needs_pending_confirm && needs_migration_confirm {
                self.set_error(
                    "⏳ Saves pending and migration in progress... press 'q' again to force quit"
                        .to_string(),
                );
            } else if needs_pending_confirm {
                self.set_error(
                    "⏳ Saves pending... press 'q' again to force quit, or wait for completion"
                        .to_string(),
                );
                tracing::warn!("Quit attempted with pending saves, requiring confirmation");
            } else {
                self.set_error(
                    "Migration in progress... press 'q' again to abort and quit".to_string(),
                );
            }
            self.quit_with_pending = true;
            self.quit_with_migration = true;
            return;
        }

        self.quit();
    }

    pub fn push_mode(&mut self, new_mode: AppMode) {
        self.mode_stack.push(self.mode.clone());
        self.mode = new_mode;
    }

    pub fn pop_mode(&mut self) {
        self.mode = self.mode_stack.pop().unwrap_or(AppMode::Normal);
    }

    pub fn is_dialog_mode(&self) -> bool {
        matches!(self.mode, AppMode::Dialog(_))
    }

    pub fn get_base_mode(&self) -> &AppMode {
        if self.is_dialog_mode() {
            self.mode_stack.last().unwrap_or(&AppMode::Normal)
        } else {
            &self.mode
        }
    }
}
