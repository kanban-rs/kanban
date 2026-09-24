use super::{App, AppMode, DialogMode};
use kanban_core::ClientId;
use kanban_domain::Invalidation;
use kanban_service::api::ChangeEventFrame;

impl App {
    #[doc(hidden)]
    pub fn handle_remote_change_frame(&mut self, frame: ChangeEventFrame) {
        let own_id = ClientId::from(self.ctx.backend().instance_id());
        if frame.issued_by == own_id && !frame.issued_by.is_nil() {
            return;
        }

        self.needs_redraw = true;

        if self.ctx.save_coordinator.has_pending_saves() {
            tracing::debug!("Remote change frame ignored: own save in flight");
        } else if !self.ctx.is_dirty() {
            let inv = frame
                .invalidation
                .as_ref()
                .map(Invalidation::from)
                .unwrap_or(Invalidation::All);
            self.resolve_after_command(inv);
        } else if self.mode != AppMode::Dialog(DialogMode::ConflictResolution)
            && self.mode != AppMode::Dialog(DialogMode::ExternalChangeDetected)
        {
            self.open_dialog(DialogMode::ExternalChangeDetected);
        }
    }
}
