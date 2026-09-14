use super::App;

impl App {
    pub(in crate::app) fn migrate_sprint_logs(&mut self) -> usize {
        match self.ctx.migrate_sprint_logs() {
            Ok(n) => n,
            Err(e) if e.is_unsupported() => {
                tracing::debug!("Sprint log migration skipped: {}", e);
                0
            }
            Err(e) => {
                tracing::error!("Failed to migrate sprint logs: {}", e);
                0
            }
        }
    }
}
