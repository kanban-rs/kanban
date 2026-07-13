//! Snapshot functionality - uses domain Snapshot with TUI extensions.
//!
//! The core Snapshot type is in kanban-domain. This module provides
//! TUI-specific extension methods for App integration.
//! Serialization (to_json_bytes/from_json_bytes) lives on domain Snapshot directly.

use crate::app::App;

use kanban_domain::Snapshot;

/// Extension trait for App-specific snapshot operations.
///
/// These methods bridge between the domain Snapshot and the TUI App.
pub trait TuiSnapshot {
    /// Create a snapshot from current app state.
    fn from_app(app: &App) -> Self;

    /// Apply snapshot to app state (overwrites).
    fn apply_to_app(&self, app: &mut App) -> kanban_domain::KanbanResult<()>;
}

impl TuiSnapshot for Snapshot {
    fn from_app(app: &App) -> Self {
        app.ctx.snapshot().unwrap_or_default()
    }

    fn apply_to_app(&self, app: &mut App) -> kanban_domain::KanbanResult<()> {
        app.ctx.apply_snapshot(self.clone())?;

        // Sync sort field/order from active board to preserve user's selection after reload
        if let Some(board_idx) = app.selection.active_board_index {
            if let Some(board) = self.boards.get(board_idx) {
                app.filter.current_sort_field = Some(board.task_sort_field);
                app.filter.current_sort_order = Some(board.task_sort_order);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{Board, DependencyGraph, SortField};

    #[test]
    fn test_snapshot_serialization() {
        use kanban_persistence::{snapshot_from_json_bytes, snapshot_to_json_bytes};

        let snapshot = Snapshot {
            archived_boards: Vec::new(),
            boards: vec![],
            columns: vec![],
            cards: vec![],
            archived_cards: vec![],
            sprints: vec![],
            graph: DependencyGraph::new(),
        };

        let bytes = snapshot_to_json_bytes(&snapshot).unwrap();
        let restored = snapshot_from_json_bytes(&bytes).unwrap();

        assert_eq!(restored.boards.len(), 0);
    }

    #[test]
    fn test_apply_to_app_syncs_sort_field_from_board() {
        // Create a board with Position sort field
        let mut board = Board::new("Test", None::<String>);
        board.update_task_sort(SortField::Position, kanban_domain::SortOrder::Ascending);

        let snapshot = Snapshot {
            archived_boards: Vec::new(),
            boards: vec![board],
            columns: vec![],
            cards: vec![],
            archived_cards: vec![],
            sprints: vec![],
            graph: DependencyGraph::new(),
        };

        // Create a minimal app with active_board_index set
        let mut app = App::test_default();
        app.selection.active_board_index = Some(0);
        app.filter.current_sort_field = Some(SortField::Default);

        // Apply snapshot - should sync sort field from board
        snapshot.apply_to_app(&mut app).unwrap();

        // After apply, current_sort_field should match the board's task_sort_field
        assert_eq!(
            app.filter.current_sort_field,
            Some(SortField::Position),
            "apply_to_app should sync current_sort_field from active board"
        );
    }
}
