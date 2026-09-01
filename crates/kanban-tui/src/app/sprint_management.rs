use super::App;
use kanban_domain::LoadState;
use uuid::Uuid;

impl App {
    pub(in crate::app) fn check_ended_sprints(&self) -> Option<Vec<Uuid>> {
        let LoadState::Loaded(sprints) = self.model.sprints_state() else {
            return None;
        };
        let ended_sprints: Vec<_> = sprints
            .iter()
            .filter(|s| s.is_ended(chrono::Utc::now()))
            .collect();

        if !ended_sprints.is_empty() {
            tracing::warn!(
                "Found {} ended sprint(s) that need attention:",
                ended_sprints.len()
            );
            for sprint in &ended_sprints {
                if let Some(board) = self
                    .model
                    .boards_state()
                    .loaded_or_empty()
                    .iter()
                    .find(|b| b.id == sprint.board_id)
                {
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

        Some(ended_sprints.into_iter().map(|s| s.id).collect())
    }

    pub(in crate::app) fn migrate_sprint_logs(&mut self) -> usize {
        match self.ctx.migrate_sprint_logs() {
            Ok(n) => n,
            Err(e) => {
                tracing::error!("Failed to migrate sprint logs: {}", e);
                0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use kanban_domain::{FieldUpdate, KanbanOperations, LoadState, SprintStatus, SprintUpdate};

    fn seed_ended_sprint(app: &mut App) -> uuid::Uuid {
        let board = app.ctx.create_board("Board".into(), None).unwrap();
        let sprint = app.ctx.create_sprint(board.id, None, None).unwrap();
        app.ctx
            .update_sprint(
                sprint.id,
                SprintUpdate {
                    status: Some(SprintStatus::Active),
                    end_date: FieldUpdate::Set(chrono::Utc::now() - chrono::Duration::days(1)),
                    ..Default::default()
                },
            )
            .unwrap();
        sprint.id
    }

    #[test]
    fn test_check_ended_sprints_does_not_scan_an_unloaded_sprint_tier() {
        let mut app = App::test_default();
        seed_ended_sprint(&mut app);
        assert!(matches!(app.model.sprints_state(), LoadState::NotLoaded));

        let ended = app.check_ended_sprints();

        assert_eq!(
            ended, None,
            "a NotLoaded sprint tier must decline to scan, not report zero ended sprints"
        );
    }

    #[test]
    fn test_check_ended_sprints_reports_ended_sprints_in_the_loaded_tier() {
        let mut app = App::test_default();
        let sprint_id = seed_ended_sprint(&mut app);
        let _ = app.model.load_from_snapshot(app.ctx.snapshot().unwrap());
        assert!(matches!(app.model.sprints_state(), LoadState::Loaded(_)));

        let ended = app.check_ended_sprints();

        assert_eq!(ended, Some(vec![sprint_id]));
    }
}
