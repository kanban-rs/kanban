use super::App;

impl App {
    pub(in crate::app) fn migrate_sprint_logs(&mut self) -> usize {
        match self.ctx.migrate_sprint_logs() {
            Ok(n) => n,
            // `KanbanContext::migrate_sprint_logs` leaves the undo stack and
            // `dirty` flag untouched on every error path (see its doc
            // comment), and `is_unsupported()` additionally means nothing
            // was written, so this is always a genuine no-op: quiet by
            // design.
            Err(e) if e.is_unsupported() => {
                tracing::debug!("Sprint log migration skipped: backend declined: {e}");
                0
            }
            Err(e) => {
                tracing::error!("Failed to migrate sprint logs: {}", e);
                0
            }
        }
    }
}
