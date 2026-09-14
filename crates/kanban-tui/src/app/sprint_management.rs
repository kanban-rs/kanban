use super::App;
use kanban_domain::KanbanError;

impl App {
    pub(in crate::app) fn migrate_sprint_logs(&mut self) -> usize {
        match self.ctx.migrate_sprint_logs() {
            Ok(n) => n,
            Err(KanbanError::Unsupported {
                operation: "list_all_cards",
            }) => {
                tracing::debug!("Sprint log migration skipped: backend does not support list_all_cards");
                0
            }
            Err(e) => {
                tracing::error!("Failed to migrate sprint logs: {}", e);
                0
            }
        }
    }
}
