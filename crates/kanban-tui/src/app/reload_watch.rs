use super::App;

impl App {
    pub(in crate::app) async fn auto_reload_from_external_change(&mut self) {
        match self.ctx.reload().await {
            Ok(()) => {
                self.ctx.mark_clean();
                self.ctx.clear_conflict();
                self.reload_model();
                self.prepare_frame();
                self.needs_redraw = true;
                tracing::info!("Auto-reloaded state from external file change");
            }
            Err(e) => {
                tracing::error!("Failed to reload from disk: {}", e);
            }
        }
    }
}
