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
    fn from_app(app: &App) -> kanban_domain::KanbanResult<Self>
    where
        Self: Sized;

    /// Apply snapshot to app state (overwrites).
    fn apply_to_app(&self, app: &mut App) -> kanban_domain::KanbanResult<()>;
}

impl TuiSnapshot for Snapshot {
    fn from_app(app: &App) -> kanban_domain::KanbanResult<Self> {
        app.ctx.snapshot()
    }

    fn apply_to_app(&self, app: &mut App) -> kanban_domain::KanbanResult<()> {
        app.ctx.apply_snapshot(self.clone())?;

        // Sync sort field/order from active board to preserve user's selection
        // after reload. The snapshot's `boards` carries every head (live and
        // archived), so resolving by id works for either.
        if let Some(board_id) = app.selection.active_board_id {
            if let Some(board) = self.boards.iter().find(|b| b.id == board_id) {
                app.filter.current_sort_field = Some(board.task_sort_field);
                app.filter.current_sort_order = Some(board.task_sort_order);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
struct AlwaysFailsSnapshotBackend {
    inner: kanban_backend_memory::InMemoryStore,
}

#[cfg(test)]
impl AlwaysFailsSnapshotBackend {
    fn as_backend() -> std::sync::Arc<dyn kanban_backend::KanbanBackend> {
        std::sync::Arc::new(Self {
            inner: kanban_backend_memory::InMemoryStore::new(),
        })
    }
}

#[cfg(test)]
impl kanban_domain::DataStore for AlwaysFailsSnapshotBackend {
    fn get_prefix(&self, name: &str) -> kanban_domain::KanbanResult<Option<kanban_domain::Prefix>> {
        self.inner.get_prefix(name)
    }
    fn list_prefixes(&self) -> kanban_domain::KanbanResult<Vec<kanban_domain::Prefix>> {
        self.inner.list_prefixes()
    }
    fn upsert_prefix(&self, prefix: kanban_domain::Prefix) -> kanban_domain::KanbanResult<()> {
        self.inner.upsert_prefix(prefix)
    }
    fn get_board(
        &self,
        id: uuid::Uuid,
    ) -> kanban_domain::KanbanResult<Option<kanban_domain::Board>> {
        self.inner.get_board(id)
    }
    fn list_boards(&self) -> kanban_domain::KanbanResult<Vec<kanban_domain::Board>> {
        self.inner.list_boards()
    }
    fn upsert_board(&self, board: kanban_domain::Board) -> kanban_domain::KanbanResult<()> {
        self.inner.upsert_board(board)
    }
    fn delete_board(&self, id: uuid::Uuid) -> kanban_domain::KanbanResult<()> {
        self.inner.delete_board(id)
    }
    fn get_column(
        &self,
        id: uuid::Uuid,
    ) -> kanban_domain::KanbanResult<Option<kanban_domain::Column>> {
        self.inner.get_column(id)
    }
    fn list_columns_by_board(
        &self,
        board_id: uuid::Uuid,
    ) -> kanban_domain::KanbanResult<Vec<kanban_domain::Column>> {
        self.inner.list_columns_by_board(board_id)
    }
    fn list_all_columns(&self) -> kanban_domain::KanbanResult<Vec<kanban_domain::Column>> {
        self.inner.list_all_columns()
    }
    fn upsert_column(&self, column: kanban_domain::Column) -> kanban_domain::KanbanResult<()> {
        self.inner.upsert_column(column)
    }
    fn delete_column(&self, id: uuid::Uuid) -> kanban_domain::KanbanResult<()> {
        self.inner.delete_column(id)
    }
    fn delete_columns_by_board(&self, board_id: uuid::Uuid) -> kanban_domain::KanbanResult<()> {
        self.inner.delete_columns_by_board(board_id)
    }
    fn get_card(&self, id: uuid::Uuid) -> kanban_domain::KanbanResult<Option<kanban_domain::Card>> {
        self.inner.get_card(id)
    }
    fn list_all_cards(&self) -> kanban_domain::KanbanResult<Vec<kanban_domain::Card>> {
        self.inner.list_all_cards()
    }
    fn list_cards_by_column(
        &self,
        column_id: uuid::Uuid,
    ) -> kanban_domain::KanbanResult<Vec<kanban_domain::Card>> {
        self.inner.list_cards_by_column(column_id)
    }
    fn list_cards_by_sprint(
        &self,
        sprint_id: uuid::Uuid,
    ) -> kanban_domain::KanbanResult<Vec<kanban_domain::Card>> {
        self.inner.list_cards_by_sprint(sprint_id)
    }
    fn count_cards_in_column(&self, column_id: uuid::Uuid) -> kanban_domain::KanbanResult<usize> {
        self.inner.count_cards_in_column(column_id)
    }
    fn upsert_card(&self, card: kanban_domain::Card) -> kanban_domain::KanbanResult<()> {
        self.inner.upsert_card(card)
    }
    fn delete_card(&self, id: uuid::Uuid) -> kanban_domain::KanbanResult<()> {
        self.inner.delete_card(id)
    }
    fn delete_cards_by_columns(
        &self,
        column_ids: &[uuid::Uuid],
    ) -> kanban_domain::KanbanResult<()> {
        self.inner.delete_cards_by_columns(column_ids)
    }
    fn clear_sprint_from_cards(
        &self,
        sprint_id: uuid::Uuid,
        cleared_at: chrono::DateTime<chrono::Utc>,
    ) -> kanban_domain::KanbanResult<()> {
        self.inner.clear_sprint_from_cards(sprint_id, cleared_at)
    }
    fn count_cards_in_column_excluding(
        &self,
        column_id: uuid::Uuid,
        exclude_ids: &[uuid::Uuid],
    ) -> kanban_domain::KanbanResult<usize> {
        self.inner
            .count_cards_in_column_excluding(column_id, exclude_ids)
    }
    fn get_archived_card(
        &self,
        card_id: uuid::Uuid,
    ) -> kanban_domain::KanbanResult<Option<kanban_domain::ArchivedCard>> {
        self.inner.get_archived_card(card_id)
    }
    fn list_archived_cards(&self) -> kanban_domain::KanbanResult<Vec<kanban_domain::ArchivedCard>> {
        self.inner.list_archived_cards()
    }
    fn insert_archived_card(
        &self,
        ac: kanban_domain::ArchivedCard,
    ) -> kanban_domain::KanbanResult<()> {
        self.inner.insert_archived_card(ac)
    }
    fn delete_archived_card(&self, card_id: uuid::Uuid) -> kanban_domain::KanbanResult<()> {
        self.inner.delete_archived_card(card_id)
    }
    fn get_sprint(
        &self,
        id: uuid::Uuid,
    ) -> kanban_domain::KanbanResult<Option<kanban_domain::Sprint>> {
        self.inner.get_sprint(id)
    }
    fn list_sprints_by_board(
        &self,
        board_id: uuid::Uuid,
    ) -> kanban_domain::KanbanResult<Vec<kanban_domain::Sprint>> {
        self.inner.list_sprints_by_board(board_id)
    }
    fn list_all_sprints(&self) -> kanban_domain::KanbanResult<Vec<kanban_domain::Sprint>> {
        self.inner.list_all_sprints()
    }
    fn upsert_sprint(&self, sprint: kanban_domain::Sprint) -> kanban_domain::KanbanResult<()> {
        self.inner.upsert_sprint(sprint)
    }
    fn delete_sprint(&self, id: uuid::Uuid) -> kanban_domain::KanbanResult<()> {
        self.inner.delete_sprint(id)
    }
    fn delete_sprints_by_board(&self, board_id: uuid::Uuid) -> kanban_domain::KanbanResult<()> {
        self.inner.delete_sprints_by_board(board_id)
    }
    fn get_graph(&self) -> kanban_domain::KanbanResult<kanban_domain::DependencyGraph> {
        self.inner.get_graph()
    }
    fn set_graph(&self, graph: kanban_domain::DependencyGraph) -> kanban_domain::KanbanResult<()> {
        self.inner.set_graph(graph)
    }
    fn snapshot(&self) -> kanban_domain::KanbanResult<Snapshot> {
        Err(kanban_domain::KanbanError::Database(
            "simulated transient read failure".to_string(),
        ))
    }
    fn apply_snapshot(&self, snapshot: Snapshot) -> kanban_domain::KanbanResult<()> {
        self.inner.apply_snapshot(snapshot)
    }
}

#[cfg(test)]
impl kanban_domain::CommandStore for AlwaysFailsSnapshotBackend {
    fn append_batch(
        &self,
        batch: &kanban_domain::CommandBatch,
    ) -> kanban_domain::KanbanResult<u64> {
        self.inner.append_batch(batch)
    }
    fn batch_count(&self) -> kanban_domain::KanbanResult<u64> {
        self.inner.batch_count()
    }
    fn load_batches(
        &self,
        offset: u64,
        limit: u64,
    ) -> kanban_domain::KanbanResult<Vec<kanban_domain::CommandBatch>> {
        self.inner.load_batches(offset, limit)
    }
}

#[cfg(test)]
impl kanban_backend::KanbanBackend for AlwaysFailsSnapshotBackend {
    fn as_data_store(&self) -> &dyn kanban_domain::DataStore {
        self
    }

    fn with_transaction(
        &self,
        f: kanban_backend::TransactionFn<'_>,
    ) -> kanban_domain::KanbanResult<()> {
        self.inner.with_transaction(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{Board, DependencyGraph, SortField};

    #[test]
    fn test_from_app_propagates_a_failed_snapshot_read() {
        let mut app = App::test_default();
        app.ctx
            .replace_backend(AlwaysFailsSnapshotBackend::as_backend());

        let result = Snapshot::from_app(&app);

        assert!(
            result.is_err(),
            "from_app must propagate a failed backend read, not fall back to Snapshot::default()"
        );
    }

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
            prefixes: Vec::new(),
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
        let board_id = board.id;

        let snapshot = Snapshot {
            archived_boards: Vec::new(),
            boards: vec![board],
            columns: vec![],
            cards: vec![],
            archived_cards: vec![],
            sprints: vec![],
            graph: DependencyGraph::new(),
            prefixes: Vec::new(),
        };

        // Create a minimal app with the active board set by id.
        let mut app = App::test_default();
        app.selection.active_board_id = Some(board_id);
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
