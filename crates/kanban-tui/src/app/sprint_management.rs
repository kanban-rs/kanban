use super::App;

impl App {
    pub(in crate::app) fn check_ended_sprints(&self) {
        let ended_sprints: Vec<_> = self
            .model
            .sprints()
            .iter()
            .filter(|s| s.is_ended(chrono::Utc::now()))
            .collect();

        if !ended_sprints.is_empty() {
            tracing::warn!(
                "Found {} ended sprint(s) that need attention:",
                ended_sprints.len()
            );
            for sprint in &ended_sprints {
                if let Some(board) = self.model.boards().iter().find(|b| b.id == sprint.board_id) {
                    tracing::warn!(
                        "  - {} (ended: {})",
                        sprint.formatted_name(board, None),
                        sprint
                            .end_date
                            .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    );
                }
            }
        }
    }

    pub(in crate::app) fn migrate_sprint_logs(&mut self) {
        if let Err(e) = self.ctx.migrate_sprint_logs() {
            tracing::error!("Failed to migrate sprint logs: {}", e);
        }
    }
}
