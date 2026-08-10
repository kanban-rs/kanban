use super::KanbanContext;
use crate::backend::KanbanBackend;
use kanban_core::{AppConfig, AppType};
use kanban_domain::{
    ArchivedCard, Board, Card, Column, DataStore, DependencyGraph, KanbanResult, Snapshot, Sprint,
};
use std::sync::Arc;
use uuid::Uuid;

impl KanbanContext {
    /// Zero-I/O constructor. Wraps `backend` without reading any data.
    /// Use [`open`][Self::open] instead when a lazy backend's load
    /// errors should surface at construction time.
    pub fn open_deferred(backend: Arc<dyn KanbanBackend>, config: AppConfig) -> Self {
        Self {
            backend,
            app_config: config,
            undo_stack: crate::undo_stack::UndoStack::new(),
            dirty: false,
            conflict_pending: false,
            session_id: Uuid::new_v4(),
            app_type: AppType::Unknown,
        }
    }

    /// Set the application type for command attribution. Call immediately after open_deferred().
    pub fn with_app_type(mut self, app_type: AppType) -> Self {
        self.app_type = app_type;
        self
    }

    /// The session ID, stable for this context's lifetime. Each surface
    /// (CLI, MCP, TUI) opens one context per process, so in practice this
    /// is one ID per process run.
    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    /// Wraps `backend` and forces a lazy backend's I/O so any
    /// deserialization or read failure surfaces here, before the
    /// caller starts mutating.
    pub async fn open(backend: Arc<dyn KanbanBackend>, config: AppConfig) -> KanbanResult<Self> {
        let ctx = Self::open_deferred(backend, config);
        ctx.backend.batch_count()?;
        Ok(ctx)
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    pub fn app_config(&self) -> &AppConfig {
        &self.app_config
    }

    pub fn data_store(&self) -> &dyn DataStore {
        self.backend.as_data_store()
    }

    pub fn backend(&self) -> Arc<dyn KanbanBackend> {
        Arc::clone(&self.backend)
    }

    /// Metadata for the underlying persistence store: format version, writer
    /// kanban version, writer commit, last save time. Returns `None` for
    /// in-memory backends or before the underlying file has been loaded.
    /// Surfaced by the TUI F12 diagnostics panel.
    pub fn persistence_metadata(&self) -> Option<kanban_persistence::PersistenceMetadata> {
        self.backend.local_persistence()?.persistence_metadata()
    }

    /// Replace the active backend, discarding all undo/redo history.
    pub fn replace_backend(&mut self, backend: Arc<dyn KanbanBackend>) {
        tracing::info!("Replacing backend; undo/redo history discarded");
        self.backend = backend;
        self.undo_stack.clear();
        self.dirty = false;
    }

    pub fn boards(&self) -> KanbanResult<Vec<Board>> {
        self.backend.list_boards()
    }

    /// LIVE-scoped (C3b): excludes archived-board columns. TUI/display reads use
    /// this; raw all-columns is `self.backend.list_all_columns()`.
    pub fn columns(&self) -> KanbanResult<Vec<Column>> {
        self.list_live_columns_impl()
    }

    pub fn cards(&self) -> KanbanResult<Vec<Card>> {
        self.list_live_cards_impl()
    }

    pub fn sprints(&self) -> KanbanResult<Vec<Sprint>> {
        self.list_live_sprints_impl()
    }

    pub fn archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.backend.list_archived_cards()
    }

    pub fn graph(&self) -> KanbanResult<DependencyGraph> {
        self.backend.get_graph()
    }

    /// Canonical board-existence check (KAN-248): returns the board or
    /// `NotFound`. The single FK guard used before dispatching any operation
    /// that targets a board, so board-membership validation cannot be skipped
    /// or done inconsistently across call sites.
    pub fn require_board(&self, id: Uuid) -> KanbanResult<Board> {
        self.backend
            .get_board(id)?
            .ok_or_else(|| kanban_domain::KanbanError::not_found("Board", id))
    }

    /// Canonical column-membership check (KAN-248): returns the column or
    /// `NotFound`. The single FK guard used before dispatching any operation
    /// that targets a column (create/replace/move), mirroring the command-tier
    /// `CommandContext::require_column` in name + behavior.
    pub fn require_column(&self, id: Uuid) -> KanbanResult<Column> {
        self.backend
            .get_column(id)?
            .ok_or_else(|| kanban_domain::KanbanError::not_found("Column", id))
    }

    pub fn snapshot(&self) -> KanbanResult<Snapshot> {
        self.backend.snapshot()
    }

    pub fn apply_snapshot(&self, snapshot: Snapshot) -> KanbanResult<()> {
        let mut snapshot = snapshot;
        // Trusted seam, but completion ids that resolve to no column in this
        // snapshot must not diverge per backend: JSON would store the dangling
        // id while SQLite's foreign key rejects the whole import. Prune them so
        // both backends accept and agree; order of the survivors is preserved.
        let columns = std::mem::take(&mut snapshot.columns);
        for board in &mut snapshot.boards {
            let board_id = board.id;
            board.completion_column_ids.retain(|id| {
                columns
                    .iter()
                    .any(|c| c.id == *id && c.board_id == board_id)
            });
        }
        snapshot.columns = columns;
        self.backend.apply_snapshot(snapshot)
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn has_conflict(&self) -> bool {
        self.conflict_pending
    }

    pub fn set_conflict(&mut self) {
        self.conflict_pending = true;
    }

    pub fn clear_conflict(&mut self) {
        self.conflict_pending = false;
    }

    pub fn set_conflict_pending(&mut self, v: bool) {
        self.conflict_pending = v;
    }
}
