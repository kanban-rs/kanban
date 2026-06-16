use crate::backend::KanbanBackend;
use kanban_core::{AppConfig, AppType, ClientId, KANBAN_VERSION};
use kanban_domain::commands::{
    AddBlocks, AddRelates, AddSpawns, BoardCommand, CardCommand, ColumnCommand, Command,
    CommandContext, DependencyCommand, RemoveBlocks, RemoveRelates, RemoveSpawns, SprintCommand,
};
use kanban_domain::{
    ArchivedCard, Board, BoardUpdate, Card, CardListFilter, CardStatus, CardSummary, CardUpdate,
    Column, ColumnUpdate, DataStore, DependencyGraph, FieldUpdate, GraphOperations,
    KanbanOperations, RelatesKind, Severity, Snapshot, Sprint, SprintUpdate,
};
use kanban_domain::{KanbanError, KanbanResult};
use kanban_persistence::PersistenceError;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct BatchOperationResult {
    pub succeeded: Vec<Uuid>,
    pub failed: Vec<BatchOperationFailure>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchOperationFailure {
    pub id: Uuid,
    pub error: String,
}

/// Service layer: wraps a pluggable [`KanbanBackend`] with undo/redo history
/// and a unified async `save()` / `reload()` interface.
///
/// Construction is always zero-I/O — data is fetched lazily on the first
/// read, either directly (SQLite, reads are always live) or via a one-time
/// cache-fill on first access (JSON).
///
/// # Undo / Redo model
///
/// Every undoable command captures an **inverse** at execute time. The
/// `(forward, inverse)` pair lives on the per-session [`UndoStack`].
/// Undo executes the inverse against current state through the normal
/// command-execute path — no snapshot apply, no replay. Redo re-executes
/// the forward batch.
///
/// `execute` also appends the forward batch to the `CommandStore` audit
/// log (`backend.append_batch`). The audit log is informational — it
/// records what happened; it does not drive undo. Audit-log UI is KAN-36.
pub struct KanbanContext {
    backend: Arc<dyn KanbanBackend>,
    app_config: AppConfig,
    /// Per-session inverse-command undo state.
    undo_stack: crate::undo_stack::UndoStack,
    dirty: bool,
    conflict_pending: bool,
    /// Generated once at open_deferred; stable for the process lifetime.
    session_id: Uuid,
    /// Which application surface owns this context. Default: Unknown.
    app_type: AppType,
}

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

    /// The stable session ID for this process lifetime.
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
        self.backend.persistence_metadata()
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

    pub fn columns(&self) -> KanbanResult<Vec<Column>> {
        self.backend.list_all_columns()
    }

    pub fn cards(&self) -> KanbanResult<Vec<Card>> {
        self.backend.list_all_cards()
    }

    pub fn sprints(&self) -> KanbanResult<Vec<Sprint>> {
        self.backend.list_all_sprints()
    }

    pub fn archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.backend.list_archived_cards()
    }

    pub fn graph(&self) -> KanbanResult<DependencyGraph> {
        self.backend.get_graph()
    }

    pub fn snapshot(&self) -> KanbanResult<Snapshot> {
        self.backend.snapshot()
    }

    pub fn apply_snapshot(&self, snapshot: Snapshot) -> KanbanResult<()> {
        self.backend.apply_snapshot(snapshot)
    }

    // ── Data migrations ───────────────────────────────────────────────────────

    /// Backfill `sprint_logs` for cards that have a `sprint_id` but empty logs.
    ///
    /// This is a one-time data-migration utility, not a regular operation —
    /// it bypasses the undo stack on purpose. The actual rule for what
    /// constitutes a correctly migrated log lives in
    /// [`kanban_domain::card_lifecycle::migrate_sprint_logs`]; this method
    /// just orchestrates the read → transform → persist-changed loop.
    ///
    /// `sprints` and `boards` are passed by shared reference to the pure
    /// function — they are reference data only, never mutated, so the
    /// persist loop correctly iterates `cards` alone.
    ///
    /// Returns the number of cards that received a backfilled log.
    pub fn migrate_sprint_logs(&mut self) -> KanbanResult<usize> {
        let mut cards = self.backend.list_all_cards()?;
        let sprints = self.backend.list_all_sprints()?;
        let boards = self.backend.list_boards()?;
        let before_logs: Vec<_> = cards.iter().map(|c| c.sprint_logs.clone()).collect();
        let count =
            kanban_domain::card_lifecycle::migrate_sprint_logs(&mut cards, &sprints, &boards);
        if count > 0 {
            // Invalidate the entire undo history — a data migration
            // mutates state outside the command pipeline, so any
            // inverse captured before the migration would now reference
            // stale entity values.
            self.undo_stack.clear();
            tracing::info!("Migrated sprint logs for {} card(s)", count);
            for (card, before) in cards.into_iter().zip(before_logs) {
                if card.sprint_logs != before {
                    self.backend.upsert_card(card)?;
                }
            }
            self.dirty = true;
        }
        Ok(count)
    }

    // ── Undo / Redo ───────────────────────────────────────────────────────────

    /// Execute a batch as one undo unit. Entity mutations, inverse
    /// capture, and audit-log append run inside one transaction —
    /// either all commit or all roll back.
    ///
    /// Each command's inverse is captured against the state the
    /// previous command left behind. The composed inverse is the
    /// per-command inverses in reverse order, so undoing each `Fk_inv`
    /// runs against the state `Fk` itself saw at capture time.
    pub fn execute(&mut self, commands: Vec<Command>) -> KanbanResult<()> {
        let backend = Arc::clone(&self.backend);
        let cmds = &commands;
        let mut per_cmd_inverses: Vec<Vec<Command>> = Vec::new();
        self.backend.with_transaction(&mut || {
            let store: &dyn DataStore = backend.as_data_store();
            let ctx = CommandContext { store };
            for cmd in cmds.iter() {
                per_cmd_inverses.push(cmd.capture_inverse(store)?);
                cmd.execute(&ctx)?;
            }
            let batch = kanban_domain::CommandBatch::new(
                cmds.clone(),
                Uuid::new_v4(),
                // nil locally; the HTTP layer assigns the real client identity (KAN-751)
                ClientId::nil(),
                chrono::Utc::now(),
                self.app_type,
                KANBAN_VERSION.to_string(),
                self.session_id,
            );
            backend.append_batch(&batch)?;
            Ok(())
        })?;
        let inverses: Vec<Command> = per_cmd_inverses.into_iter().rev().flatten().collect();

        self.undo_stack.push(crate::undo_stack::UndoEntry {
            forward: commands,
            inverse: inverses,
        });

        self.dirty = true;
        Ok(())
    }

    /// Undo the most recent batch via inverse-command execution.
    /// The cursor advances only if the inverse commits successfully —
    /// a failed undo leaves the stack ready to retry the same entry.
    pub fn undo(&mut self) -> KanbanResult<bool> {
        let inverse = match self.undo_stack.peek_undo() {
            Some(entry) => entry.inverse.clone(),
            None => return Ok(false),
        };
        let backend = Arc::clone(&self.backend);
        let inv = &inverse;
        self.backend.with_transaction(&mut || {
            let store: &dyn DataStore = backend.as_data_store();
            let ctx = CommandContext { store };
            inv.iter().try_for_each(|cmd| cmd.execute(&ctx))
        })?;
        self.undo_stack.commit_undo();
        self.dirty = true;
        Ok(true)
    }

    /// Redo the next undone batch via forward-command execution.
    /// The cursor advances only if the forward batch commits — a failed
    /// redo leaves the stack ready to retry the same entry.
    pub fn redo(&mut self) -> KanbanResult<bool> {
        let forward = match self.undo_stack.peek_redo() {
            Some(entry) => entry.forward.clone(),
            None => return Ok(false),
        };
        let backend = Arc::clone(&self.backend);
        let fwd = &forward;
        self.backend.with_transaction(&mut || {
            let store: &dyn DataStore = backend.as_data_store();
            let ctx = CommandContext { store };
            fwd.iter().try_for_each(|cmd| cmd.execute(&ctx))
        })?;
        self.undo_stack.commit_redo();
        self.dirty = true;
        Ok(true)
    }

    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Drop the per-session undo/redo history. The audit log is
    /// append-only and is not touched.
    pub fn clear_history(&mut self) -> KanbanResult<()> {
        self.undo_stack.clear();
        Ok(())
    }

    pub fn undo_depth(&self) -> usize {
        self.undo_stack.undo_depth()
    }

    pub fn redo_depth(&self) -> usize {
        self.undo_stack.redo_depth()
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

    // ── Persistence ───────────────────────────────────────────────────────────

    /// Reload state from durable storage, discarding any uncommitted
    /// data cache. Drops the per-session `UndoStack` (entity ids from
    /// before the reload may no longer exist). The audit log is left
    /// untouched — it records what happened, and a reload does not
    /// unhappen it.
    pub async fn reload(&mut self) -> KanbanResult<()> {
        self.backend.reload().await?;
        self.undo_stack.clear();
        self.dirty = false;
        Ok(())
    }

    /// Persist any dirty state to durable storage.
    /// For SQLite this is a WAL checkpoint; for JSON this flushes the cache.
    pub async fn save(&self) -> KanbanResult<()> {
        self.backend.flush().await
    }

    // ── Batch ops ─────────────────────────────────────────────────────────────

    pub fn archive_cards_detailed(&mut self, ids: Vec<Uuid>) -> BatchOperationResult {
        use kanban_domain::commands::ArchiveCards;
        let all_cards = match self.backend.list_all_cards() {
            Ok(c) => c,
            Err(e) => {
                return BatchOperationResult {
                    succeeded: vec![],
                    failed: ids
                        .into_iter()
                        .map(|id| BatchOperationFailure {
                            id,
                            error: e.to_string(),
                        })
                        .collect(),
                };
            }
        };
        let card_ids: std::collections::HashSet<Uuid> = all_cards.iter().map(|c| c.id).collect();
        let mut to_archive = Vec::new();
        let mut failed = Vec::new();
        for id in ids {
            if card_ids.contains(&id) {
                to_archive.push(id);
            } else {
                failed.push(BatchOperationFailure {
                    id,
                    error: KanbanError::not_found("Card", id).to_string(),
                });
            }
        }
        if to_archive.is_empty() {
            return BatchOperationResult {
                succeeded: vec![],
                failed,
            };
        }
        let succeeded = to_archive.clone();
        match self.execute(vec![Command::Card(CardCommand::Archive(ArchiveCards {
            ids: to_archive,
        }))]) {
            Ok(()) => BatchOperationResult { succeeded, failed },
            Err(e) => {
                let err = e.to_string();
                let mut all_failed = failed;
                all_failed.extend(succeeded.into_iter().map(|id| BatchOperationFailure {
                    id,
                    error: err.clone(),
                }));
                BatchOperationResult {
                    succeeded: vec![],
                    failed: all_failed,
                }
            }
        }
    }

    pub fn move_cards_detailed(&mut self, ids: Vec<Uuid>, column_id: Uuid) -> BatchOperationResult {
        // Dedup at the input boundary so the per-id classification loop both
        // (a) reports each invalid id once in `failed` and (b) reports each
        // valid id once in `succeeded`, matching the one `MoveCard` per
        // unique id that `compute_move_positions` will emit. Also avoids
        // redundant get_card calls for the same id.
        let ids = kanban_domain::card_lifecycle::dedup_preserving_order(&ids);
        let mut to_move = Vec::new();
        let mut failed = Vec::new();
        for id in ids {
            match self.backend.get_card(id) {
                Ok(Some(_)) => to_move.push(id),
                Ok(None) => failed.push(BatchOperationFailure {
                    id,
                    error: KanbanError::not_found("Card", id).to_string(),
                }),
                Err(e) => failed.push(BatchOperationFailure {
                    id,
                    error: e.to_string(),
                }),
            }
        }
        if to_move.is_empty() {
            return BatchOperationResult {
                succeeded: vec![],
                failed,
            };
        }
        let succeeded = to_move.clone();

        let chained_status_updates =
            match self.chained_status_updates_for_batch_move(&to_move, column_id) {
                Ok(v) => v,
                Err(e) => {
                    let err = e.to_string();
                    let mut all_failed = failed;
                    all_failed.extend(succeeded.into_iter().map(|id| BatchOperationFailure {
                        id,
                        error: err.clone(),
                    }));
                    return BatchOperationResult {
                        succeeded: vec![],
                        failed: all_failed,
                    };
                }
            };

        let batch = match self.build_move_cards_batch(&to_move, column_id, chained_status_updates) {
            Ok(b) => b,
            Err(e) => {
                let err = e.to_string();
                let mut all_failed = failed;
                all_failed.extend(succeeded.into_iter().map(|id| BatchOperationFailure {
                    id,
                    error: err.clone(),
                }));
                return BatchOperationResult {
                    succeeded: vec![],
                    failed: all_failed,
                };
            }
        };

        match self.execute(batch) {
            Ok(()) => BatchOperationResult { succeeded, failed },
            Err(e) => {
                let err = e.to_string();
                let mut all_failed = failed;
                all_failed.extend(succeeded.into_iter().map(|id| BatchOperationFailure {
                    id,
                    error: err.clone(),
                }));
                BatchOperationResult {
                    succeeded: vec![],
                    failed: all_failed,
                }
            }
        }
    }

    pub fn assign_cards_to_sprint_detailed(
        &mut self,
        ids: Vec<Uuid>,
        sprint_id: Uuid,
    ) -> BatchOperationResult {
        use kanban_domain::commands::AssignCardsToSprint;
        let all_sprints = match self.backend.list_all_sprints() {
            Ok(s) => s,
            Err(e) => {
                return BatchOperationResult {
                    succeeded: vec![],
                    failed: ids
                        .into_iter()
                        .map(|id| BatchOperationFailure {
                            id,
                            error: e.to_string(),
                        })
                        .collect(),
                };
            }
        };
        if !all_sprints.iter().any(|s| s.id == sprint_id) {
            return BatchOperationResult {
                succeeded: vec![],
                failed: ids
                    .into_iter()
                    .map(|id| BatchOperationFailure {
                        id,
                        error: KanbanError::not_found("Sprint", sprint_id).to_string(),
                    })
                    .collect(),
            };
        }
        let all_cards = match self.backend.list_all_cards() {
            Ok(c) => c,
            Err(e) => {
                return BatchOperationResult {
                    succeeded: vec![],
                    failed: ids
                        .into_iter()
                        .map(|id| BatchOperationFailure {
                            id,
                            error: e.to_string(),
                        })
                        .collect(),
                };
            }
        };
        let card_ids: std::collections::HashSet<Uuid> = all_cards.iter().map(|c| c.id).collect();
        let mut to_assign = Vec::new();
        let mut failed = Vec::new();
        for id in ids {
            if card_ids.contains(&id) {
                to_assign.push(id);
            } else {
                failed.push(BatchOperationFailure {
                    id,
                    error: KanbanError::not_found("Card", id).to_string(),
                });
            }
        }
        if to_assign.is_empty() {
            return BatchOperationResult {
                succeeded: vec![],
                failed,
            };
        }
        let succeeded = to_assign.clone();
        match self.execute(vec![Command::Card(CardCommand::AssignToSprint(
            AssignCardsToSprint {
                ids: to_assign,
                sprint_id,
            },
        ))]) {
            Ok(()) => BatchOperationResult { succeeded, failed },
            Err(e) => {
                let err = e.to_string();
                let mut all_failed = failed;
                all_failed.extend(succeeded.into_iter().map(|id| BatchOperationFailure {
                    id,
                    error: err.clone(),
                }));
                BatchOperationResult {
                    succeeded: vec![],
                    failed: all_failed,
                }
            }
        }
    }

    /// KAN-394: given a status that's about to be applied to a card, compute the
    /// target column the card should live in (and the position to use in that
    /// column) to maintain the status ↔ completion column invariant. Returns
    /// None when no chained move is needed.
    ///
    /// The position is computed via a column-scoped `list_cards_by_column`
    /// query — same convention as `KanbanContext::move_card(_, _, None)` — so
    /// we only ever read the target column, never the full cards table.
    fn compute_target_column_for_status(
        &self,
        card_id: Uuid,
        new_status: CardStatus,
    ) -> KanbanResult<Option<(Uuid, i32)>> {
        let Some(card) = self.backend.get_card(card_id)? else {
            return Ok(None);
        };
        let Some(column) = self.backend.get_column(card.column_id)? else {
            return Ok(None);
        };
        let Some(board) = self.backend.get_board(column.board_id)? else {
            return Ok(None);
        };
        let columns = self.backend.list_columns_by_board(board.id)?;
        let Some(target_col) = kanban_domain::card_lifecycle::target_column_for_status(
            &card, new_status, &board, &columns,
        ) else {
            return Ok(None);
        };
        let pos = self.backend.list_cards_by_column(target_col)?.len() as i32;
        Ok(Some((target_col, pos)))
    }

    /// KAN-394: per-card chained status updates for a batch move. For each id
    /// in `ids`, asks the domain whether moving to `new_column_id` requires a
    /// status flip. Returns the cards that need a status update along with
    /// their target status. Cards that aren't found are silently skipped —
    /// individual `MoveCard` commands will surface the not-found error.
    fn chained_status_updates_for_batch_move(
        &self,
        ids: &[Uuid],
        new_column_id: Uuid,
    ) -> KanbanResult<Vec<(Uuid, CardStatus)>> {
        let mut updates = Vec::new();
        for &card_id in ids {
            if let Some(new_status) = self.compute_target_status_for_move(card_id, new_column_id)? {
                updates.push((card_id, new_status));
            }
        }
        Ok(updates)
    }

    /// KAN-428: build the command batch for a multi-card move into one column.
    ///
    /// Validates that every input id is a known card up front so that an
    /// unknown id surfaces as `not_found` rather than being miscounted by
    /// the batch WIP pre-check. When the target column has a WIP limit,
    /// performs a single batch-level pre-check that returns one clean
    /// `WipLimitExceeded` before any per-card command runs. The per-card
    /// `MoveCard::execute` WIP check still runs as belt-and-suspenders, but
    /// since `count_cards_in_column_excluding` is now O(column_size +
    /// exclude.len()), the redundant per-card checks are cheap.
    fn build_move_cards_batch(
        &self,
        ids: &[Uuid],
        column_id: Uuid,
        chained_status_updates: Vec<(Uuid, CardStatus)>,
    ) -> KanbanResult<Vec<Command>> {
        use kanban_domain::commands::{MoveCard, UpdateCard};
        use kanban_domain::DomainError;
        use std::collections::HashSet;

        for &id in ids {
            if self.backend.get_card(id)?.is_none() {
                return Err(KanbanError::not_found("Card", id));
            }
        }

        let existing = self.backend.list_cards_by_column(column_id)?;
        let column = self
            .backend
            .get_column(column_id)?
            .ok_or_else(|| KanbanError::not_found("Column", column_id))?;

        if let Some(limit) = column.wip_limit {
            // `moving_set.len()` is the post-dedup mover count — `compute_move_positions`
            // emits one `MoveCard` per unique id, so the pre-check must use the same
            // count to avoid a false `WipLimitExceeded` when the caller passes
            // duplicates that would actually fit under the limit.
            let moving_set: HashSet<Uuid> = ids.iter().copied().collect();
            let non_moving = existing
                .iter()
                .filter(|c| !moving_set.contains(&c.id))
                .count();
            if non_moving + moving_set.len() > limit as usize {
                return Err(KanbanError::Domain(DomainError::wip_limit_exceeded(
                    column_id,
                    limit as u32,
                )));
            }
        }

        let positions = kanban_domain::card_lifecycle::compute_move_positions(&existing, ids);

        let mut batch: Vec<Command> =
            Vec::with_capacity(positions.len() + chained_status_updates.len());
        for (card_id, new_position) in positions {
            batch.push(Command::Card(CardCommand::Move(MoveCard {
                card_id,
                new_column_id: column_id,
                new_position,
            })));
        }
        for (card_id, new_status) in chained_status_updates {
            batch.push(Command::Card(CardCommand::Update(UpdateCard {
                card_id,
                updates: CardUpdate {
                    status: Some(new_status),
                    ..Default::default()
                },
            })));
        }
        Ok(batch)
    }

    /// KAN-394: given a column the card is about to move to, compute the status
    /// the card should have to maintain the status ↔ completion column invariant.
    /// Returns None when no chained status update is needed.
    fn compute_target_status_for_move(
        &self,
        card_id: Uuid,
        new_column_id: Uuid,
    ) -> KanbanResult<Option<CardStatus>> {
        let Some(card) = self.backend.get_card(card_id)? else {
            return Ok(None);
        };
        let Some(column) = self.backend.get_column(new_column_id)? else {
            return Ok(None);
        };
        let Some(board) = self.backend.get_board(column.board_id)? else {
            return Ok(None);
        };
        let columns = self.backend.list_columns_by_board(board.id)?;
        Ok(
            kanban_domain::card_lifecycle::target_status_for_column_move(
                &card,
                new_column_id,
                &board,
                &columns,
            ),
        )
    }
}

impl KanbanContext {
    fn filter_cards(&self, filter: &CardListFilter) -> KanbanResult<Vec<Card>> {
        let cards = self.backend.list_all_cards()?;
        let board = match filter.board_id {
            Some(bid) => self.backend.get_board(bid)?,
            None => None,
        };
        let columns = match filter.board_id {
            Some(bid) => self.backend.list_columns_by_board(bid)?,
            None => Vec::new(),
        };
        let sprints = match (board.as_ref(), filter.search.as_deref()) {
            (Some(b), Some(q)) if !q.is_empty() => self.backend.list_sprints_by_board(b.id)?,
            _ => Vec::new(),
        };
        Ok(kanban_domain::filter_and_sort_cards(
            &cards,
            &columns,
            &sprints,
            board.as_ref(),
            filter,
        ))
    }
}

// ── KanbanOperations impl ─────────────────────────────────────────────────────

impl KanbanOperations for KanbanContext {
    fn create_board(&mut self, name: String, card_prefix: Option<String>) -> KanbanResult<Board> {
        use kanban_domain::commands::CreateBoard;
        let id = Uuid::new_v4();
        let position = self.backend.list_boards()?.len() as i32;
        let cmd = Command::Board(BoardCommand::Create(CreateBoard {
            id,
            name,
            card_prefix,
            position,
        }));
        self.execute(vec![cmd])?;
        self.get_board(id)?.ok_or_else(|| {
            KanbanError::Internal("Board creation succeeded but board not found".into())
        })
    }

    fn list_boards(&self) -> KanbanResult<Vec<Board>> {
        self.backend.list_boards()
    }

    fn get_board(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.backend.get_board(id)
    }

    fn update_board(&mut self, id: Uuid, updates: BoardUpdate) -> KanbanResult<Board> {
        use kanban_domain::commands::UpdateBoard;
        let cmd = Command::Board(BoardCommand::Update(UpdateBoard {
            board_id: id,
            updates,
        }));
        self.execute(vec![cmd])?;
        self.get_board(id)?
            .ok_or_else(|| KanbanError::not_found("Board", id))
    }

    fn delete_board(&mut self, id: Uuid) -> KanbanResult<()> {
        let commands = crate::cascade::delete_board(self.backend.as_data_store(), id)?;
        self.execute(commands)
    }

    fn create_column(
        &mut self,
        board_id: Uuid,
        name: String,
        position: Option<i32>,
    ) -> KanbanResult<Column> {
        use kanban_domain::commands::CreateColumn;
        let position = match position {
            Some(p) => p,
            None => self.backend.list_columns_by_board(board_id)?.len() as i32,
        };
        let id = Uuid::new_v4();
        let cmd = Command::Column(ColumnCommand::Create(CreateColumn {
            id,
            board_id,
            name,
            position,
        }));
        self.execute(vec![cmd])?;
        self.get_column(id)?.ok_or_else(|| {
            KanbanError::Internal("Column creation succeeded but column not found".into())
        })
    }

    fn list_columns(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.backend.list_columns_by_board(board_id)
    }

    fn get_column(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.backend.get_column(id)
    }

    fn update_column(&mut self, id: Uuid, updates: ColumnUpdate) -> KanbanResult<Column> {
        use kanban_domain::commands::UpdateColumn;
        let cmd = Command::Column(ColumnCommand::Update(UpdateColumn {
            column_id: id,
            updates,
        }));
        self.execute(vec![cmd])?;
        self.get_column(id)?
            .ok_or_else(|| KanbanError::not_found("Column", id))
    }

    fn delete_column(&mut self, id: Uuid) -> KanbanResult<()> {
        use kanban_domain::commands::DeleteColumn;
        let cmd = Command::Column(ColumnCommand::Delete(DeleteColumn { column_id: id }));
        self.execute(vec![cmd])
    }

    fn reorder_column(&mut self, id: Uuid, new_position: i32) -> KanbanResult<Column> {
        let updates = ColumnUpdate {
            name: None,
            position: Some(new_position),
            wip_limit: FieldUpdate::NoChange,
        };
        self.update_column(id, updates)
    }

    fn create_card(
        &mut self,
        board_id: Uuid,
        column_id: Uuid,
        title: String,
        options: kanban_domain::CreateCardOptions,
    ) -> KanbanResult<Card> {
        use kanban_domain::commands::CreateCard;
        let position = self.backend.list_cards_by_column(column_id)?.len() as i32;
        let card_number = self
            .backend
            .get_board(board_id)?
            .map(|b| b.card_counter)
            .unwrap_or(1);
        let id = Uuid::new_v4();
        let cmd = Command::Card(CardCommand::Create(CreateCard {
            id,
            card_number,
            board_id,
            column_id,
            title,
            position,
            options,
            timestamp: chrono::Utc::now(),
        }));
        self.execute(vec![cmd])?;
        self.get_card(id)?.ok_or_else(|| {
            KanbanError::Internal("Card creation succeeded but card not found".into())
        })
    }

    fn list_cards(&self, filter: CardListFilter) -> KanbanResult<Vec<CardSummary>> {
        let cards = self.filter_cards(&filter)?;
        Ok(cards.iter().map(CardSummary::from).collect())
    }

    fn get_card(&self, id: Uuid) -> KanbanResult<Option<Card>> {
        self.backend.get_card(id)
    }

    fn find_cards_by_identifier(&self, identifier: &str) -> KanbanResult<Vec<Card>> {
        use kanban_domain::search::find_cards_by_identifier as search;
        let cards = self.backend.list_all_cards()?;
        let columns = self.backend.list_all_columns()?;
        let boards = self.backend.list_boards()?;
        let sprints = self.backend.list_all_sprints()?;
        Ok(search(identifier, &cards, &columns, &boards, &sprints)
            .into_iter()
            .cloned()
            .collect())
    }

    fn list_all_cards(&self) -> KanbanResult<Vec<Card>> {
        self.backend.list_all_cards()
    }

    fn list_all_columns(&self) -> KanbanResult<Vec<Column>> {
        self.backend.list_all_columns()
    }

    fn list_all_sprints(&self) -> KanbanResult<Vec<Sprint>> {
        self.backend.list_all_sprints()
    }

    fn update_card(&mut self, id: Uuid, updates: CardUpdate) -> KanbanResult<Card> {
        self.update_cards(vec![(id, updates)])?;
        self.get_card(id)?
            .ok_or_else(|| KanbanError::not_found("Card", id))
    }

    fn move_card(
        &mut self,
        id: Uuid,
        column_id: Uuid,
        position: Option<i32>,
    ) -> KanbanResult<Card> {
        use kanban_domain::commands::{MoveCard, UpdateCard};
        let position = match position {
            Some(p) => p,
            None => self.backend.list_cards_by_column(column_id)?.len() as i32,
        };
        let mut batch = vec![Command::Card(CardCommand::Move(MoveCard {
            card_id: id,
            new_column_id: column_id,
            new_position: position,
        }))];

        if let Some(new_status) = self.compute_target_status_for_move(id, column_id)? {
            batch.push(Command::Card(CardCommand::Update(UpdateCard {
                card_id: id,
                updates: CardUpdate {
                    status: Some(new_status),
                    ..Default::default()
                },
            })));
        }

        self.execute(batch)?;
        self.get_card(id)?
            .ok_or_else(|| KanbanError::not_found("Card", id))
    }

    fn archive_card(&mut self, id: Uuid) -> KanbanResult<()> {
        match self.archive_cards(vec![id]) {
            Ok(0) | Err(KanbanError::Domain(kanban_domain::DomainError::Validation(_))) => {
                Err(KanbanError::not_found("Card", id))
            }
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn restore_card(&mut self, id: Uuid, column_id: Option<Uuid>) -> KanbanResult<Card> {
        use kanban_domain::commands::RestoreCard;
        let archived = self
            .backend
            .get_archived_card(id)?
            .ok_or_else(|| KanbanError::not_found("archived card", id))?;

        let target_column = if let Some(col_id) = column_id {
            if self.backend.get_column(col_id)?.is_none() {
                return Err(KanbanError::not_found("Column", col_id));
            }
            col_id
        } else if self
            .backend
            .get_column(archived.original_column_id)?
            .is_some()
        {
            archived.original_column_id
        } else {
            return Err(KanbanError::validation("Original column no longer exists. Specify --column-id to restore to a different column"));
        };

        let position = archived.original_position;
        let cmd = Command::Card(CardCommand::Restore(RestoreCard {
            card_id: id,
            column_id: target_column,
            position,
            timestamp: chrono::Utc::now(),
        }));
        self.execute(vec![cmd])?;
        self.get_card(id)?
            .ok_or_else(|| KanbanError::not_found("Card", id))
    }

    fn delete_card(&mut self, id: Uuid) -> KanbanResult<()> {
        use kanban_domain::commands::DeleteCard;
        let cmd = Command::Card(CardCommand::Delete(DeleteCard { card_id: id }));
        self.execute(vec![cmd])
    }

    fn list_archived_cards(&self) -> KanbanResult<Vec<ArchivedCard>> {
        self.backend.list_archived_cards()
    }

    fn assign_card_to_sprint(&mut self, card_id: Uuid, sprint_id: Uuid) -> KanbanResult<Card> {
        self.assign_cards_to_sprint(vec![card_id], sprint_id)?;
        self.get_card(card_id)?
            .ok_or_else(|| KanbanError::not_found("Card", card_id))
    }

    fn unassign_card_from_sprint(&mut self, card_id: Uuid) -> KanbanResult<Card> {
        use kanban_domain::commands::UnassignCardFromSprint;
        let cmd = Command::Card(CardCommand::UnassignFromSprint(UnassignCardFromSprint {
            card_id,
            timestamp: chrono::Utc::now(),
        }));
        self.execute(vec![cmd])?;
        self.get_card(card_id)?
            .ok_or_else(|| KanbanError::not_found("Card", card_id))
    }

    fn get_card_branch_name(&self, id: Uuid) -> KanbanResult<String> {
        let card = self
            .get_card(id)?
            .ok_or_else(|| KanbanError::not_found("Card", id))?;
        let column = self
            .backend
            .get_column(card.column_id)?
            .ok_or_else(|| KanbanError::not_found("Column", card.column_id))?;
        let board = self
            .backend
            .get_board(column.board_id)?
            .ok_or_else(|| KanbanError::not_found("Board", column.board_id))?;
        let sprints = self.backend.list_all_sprints()?;
        Ok(card.branch_name(
            &board,
            &sprints,
            self.app_config.effective_default_card_prefix(),
        ))
    }

    fn get_card_git_checkout(&self, id: Uuid) -> KanbanResult<String> {
        let card = self
            .get_card(id)?
            .ok_or_else(|| KanbanError::not_found("Card", id))?;
        let column = self
            .backend
            .get_column(card.column_id)?
            .ok_or_else(|| KanbanError::not_found("Column", card.column_id))?;
        let board = self
            .backend
            .get_board(column.board_id)?
            .ok_or_else(|| KanbanError::not_found("Board", column.board_id))?;
        let sprints = self.backend.list_all_sprints()?;
        Ok(card.git_checkout_command(
            &board,
            &sprints,
            self.app_config.effective_default_card_prefix(),
        ))
    }

    fn archive_cards(&mut self, ids: Vec<Uuid>) -> KanbanResult<usize> {
        use kanban_domain::commands::ArchiveCards;
        let before = self.backend.list_archived_cards()?.len();
        self.execute(vec![Command::Card(CardCommand::Archive(ArchiveCards {
            ids,
        }))])?;
        Ok(self.backend.list_archived_cards()?.len() - before)
    }

    fn move_cards(&mut self, ids: Vec<Uuid>, column_id: Uuid) -> KanbanResult<usize> {
        let before = self.backend.list_cards_by_column(column_id)?.len();

        let chained_status_updates = self.chained_status_updates_for_batch_move(&ids, column_id)?;
        let batch = self.build_move_cards_batch(&ids, column_id, chained_status_updates)?;

        self.execute(batch)?;
        let after = self.backend.list_cards_by_column(column_id)?.len();
        Ok(after - before)
    }

    fn update_cards(&mut self, updates: Vec<(Uuid, CardUpdate)>) -> KanbanResult<usize> {
        use kanban_domain::commands::{MoveCard, UpdateCard};
        use std::collections::HashMap;

        let count = updates.len();
        let mut batch: Vec<Command> = Vec::with_capacity(count * 2);
        // Track per-column position offsets within this batch so chained moves
        // into the same target column don't all collapse onto the same
        // position. `compute_target_column_for_status` reads `list_cards_by_column`
        // once per call against the pre-batch state.
        let mut position_offsets: HashMap<Uuid, i32> = HashMap::new();

        enum Chained {
            Move(Uuid, i32),
            Status(CardStatus),
        }

        for (card_id, card_updates) in updates {
            let chained = match (card_updates.status, card_updates.column_id) {
                (Some(new_status), None) => self
                    .compute_target_column_for_status(card_id, new_status)?
                    .map(|(col, base_pos)| {
                        let offset = position_offsets.entry(col).or_insert(0);
                        let pos = base_pos + *offset;
                        *offset += 1;
                        Chained::Move(col, pos)
                    }),
                (None, Some(new_col)) => self
                    .compute_target_status_for_move(card_id, new_col)?
                    .map(Chained::Status),
                _ => None,
            };

            batch.push(Command::Card(CardCommand::Update(UpdateCard {
                card_id,
                updates: card_updates,
            })));

            match chained {
                Some(Chained::Move(col, pos)) => {
                    batch.push(Command::Card(CardCommand::Move(MoveCard {
                        card_id,
                        new_column_id: col,
                        new_position: pos,
                    })));
                }
                Some(Chained::Status(status)) => {
                    batch.push(Command::Card(CardCommand::Update(UpdateCard {
                        card_id,
                        updates: CardUpdate {
                            status: Some(status),
                            ..Default::default()
                        },
                    })));
                }
                None => {}
            }
        }

        self.execute(batch)?;
        Ok(count)
    }

    fn assign_cards_to_sprint(&mut self, ids: Vec<Uuid>, sprint_id: Uuid) -> KanbanResult<usize> {
        use kanban_domain::commands::AssignCardsToSprint;
        let before = self.backend.list_cards_by_sprint(sprint_id)?.len();
        self.execute(vec![Command::Card(CardCommand::AssignToSprint(
            AssignCardsToSprint { ids, sprint_id },
        ))])?;
        let after = self.backend.list_cards_by_sprint(sprint_id)?.len();
        Ok(after - before)
    }

    fn carry_over_sprint_cards(
        &mut self,
        from_sprint_id: Uuid,
        to_sprint_id: Uuid,
    ) -> KanbanResult<usize> {
        use kanban_domain::query::sprint::get_sprint_uncompleted_cards;

        let from_sprint = self
            .get_sprint(from_sprint_id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", from_sprint_id))?;
        if from_sprint.status != kanban_domain::SprintStatus::Completed
            && from_sprint.status != kanban_domain::SprintStatus::Cancelled
        {
            return Err(KanbanError::validation(format!(
                "Source sprint must be Completed or Cancelled, got {:?}",
                from_sprint.status
            )));
        }
        let to_sprint = self
            .get_sprint(to_sprint_id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", to_sprint_id))?;
        if to_sprint.status != kanban_domain::SprintStatus::Planning {
            return Err(KanbanError::validation(format!(
                "Target sprint must be Planning, got {:?}",
                to_sprint.status
            )));
        }

        let all_cards = self.backend.list_all_cards()?;
        let ids: Vec<Uuid> = get_sprint_uncompleted_cards(from_sprint_id, &all_cards)
            .iter()
            .map(|c| c.id)
            .collect();
        self.assign_cards_to_sprint(ids, to_sprint_id)
    }

    fn create_sprint(
        &mut self,
        board_id: Uuid,
        prefix: Option<String>,
        name: Option<String>,
    ) -> KanbanResult<Sprint> {
        use kanban_domain::commands::CreateSprint;

        let default_sprint_prefix = self
            .app_config
            .effective_default_sprint_prefix()
            .to_string();

        let id = Uuid::new_v4();
        let cmd = Command::Sprint(SprintCommand::Create(CreateSprint {
            id,
            board_id,
            name,
            default_sprint_prefix,
            explicit_prefix: prefix,
            auto_consume_name: false,
        }));
        self.execute(vec![cmd])?;
        self.get_sprint(id)?.ok_or_else(|| {
            KanbanError::Internal("Sprint creation succeeded but sprint not found".into())
        })
    }

    fn list_sprints(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.backend.list_sprints_by_board(board_id)
    }

    fn get_sprint(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.backend.get_sprint(id)
    }

    fn update_sprint(&mut self, id: Uuid, updates: SprintUpdate) -> KanbanResult<Sprint> {
        use kanban_domain::commands::UpdateSprint;
        let cmd = Command::Sprint(SprintCommand::Update(UpdateSprint {
            sprint_id: id,
            updates,
        }));
        self.execute(vec![cmd])?;
        self.get_sprint(id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", id))
    }

    fn activate_sprint(&mut self, id: Uuid, duration_days: Option<i32>) -> KanbanResult<Sprint> {
        use kanban_domain::commands::ActivateSprint;
        let duration = duration_days.unwrap_or(14) as u32;
        let cmd = Command::Sprint(SprintCommand::Activate(ActivateSprint {
            sprint_id: id,
            duration_days: duration,
        }));
        self.execute(vec![cmd])?;
        self.get_sprint(id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", id))
    }

    fn complete_sprint(&mut self, id: Uuid) -> KanbanResult<Sprint> {
        use kanban_domain::commands::CompleteSprint;
        let cmd = Command::Sprint(SprintCommand::Complete(CompleteSprint { sprint_id: id }));
        self.execute(vec![cmd])?;
        self.get_sprint(id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", id))
    }

    fn cancel_sprint(&mut self, id: Uuid) -> KanbanResult<Sprint> {
        use kanban_domain::commands::CancelSprint;
        let cmd = Command::Sprint(SprintCommand::Cancel(CancelSprint { sprint_id: id }));
        self.execute(vec![cmd])?;
        self.get_sprint(id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", id))
    }

    fn delete_sprint(&mut self, id: Uuid) -> KanbanResult<()> {
        use kanban_domain::commands::DeleteSprint;
        let cmd = Command::Sprint(SprintCommand::Delete(DeleteSprint {
            sprint_id: id,
            timestamp: chrono::Utc::now(),
        }));
        self.execute(vec![cmd])
    }

    fn export_board(&self, board_id: Option<Uuid>) -> KanbanResult<String> {
        let snapshot = if let Some(id) = board_id {
            let boards: Vec<_> = self
                .backend
                .list_boards()?
                .into_iter()
                .filter(|b| b.id == id)
                .collect();
            let columns = self.backend.list_columns_by_board(id)?;
            let column_ids: Vec<_> = columns.iter().map(|c| c.id).collect();
            let cards: Vec<_> = self
                .backend
                .list_all_cards()?
                .into_iter()
                .filter(|c| column_ids.contains(&c.column_id))
                .collect();
            let sprints = self.backend.list_sprints_by_board(id)?;
            let graph = self.backend.get_graph()?;
            Snapshot {
                boards,
                columns,
                cards,
                archived_cards: vec![],
                sprints,
                graph,
            }
        } else {
            self.backend.snapshot()?
        };

        serde_json::to_string_pretty(&snapshot)
            .map_err(|e| PersistenceError::Serialization(e.to_string()).into())
    }

    fn import_board(&mut self, data: &str) -> KanbanResult<Board> {
        use kanban_domain::commands::ImportEntities;
        use std::collections::HashSet;

        let imported: Snapshot = serde_json::from_str(data)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let board = imported
            .boards
            .first()
            .cloned()
            .ok_or_else(|| KanbanError::validation("No board in import data"))?;

        let imported_column_ids: HashSet<Uuid> = imported.columns.iter().map(|c| c.id).collect();
        let existing_column_ids: HashSet<Uuid> = self
            .backend
            .list_all_columns()?
            .into_iter()
            .map(|c| c.id)
            .collect();
        for card in &imported.cards {
            if !imported_column_ids.contains(&card.column_id)
                && !existing_column_ids.contains(&card.column_id)
            {
                return Err(KanbanError::validation(format!(
                    "Card '{}' references column {} which does not exist in the import or the current store",
                    card.title, card.column_id
                )));
            }
        }

        let commands = vec![Command::Board(BoardCommand::Import(ImportEntities {
            boards: imported.boards,
            columns: imported.columns,
            cards: imported.cards,
            archived_cards: imported.archived_cards,
            sprints: imported.sprints,
            graph: Some(imported.graph),
        }))];

        {
            let store: &dyn DataStore = self.backend.as_data_store();
            let ctx = CommandContext { store };
            for cmd in &commands {
                cmd.execute(&ctx)?;
            }
        }

        self.undo_stack.clear();
        self.dirty = true;

        Ok(board)
    }
}

impl KanbanContext {
    /// Reject edge mutations against unknown card ids before the
    /// command reaches the graph. Without this guard a stale or
    /// fabricated UUID would silently land in the graph as a dangling
    /// edge — the CLI's identifier-resolution layer parses raw UUIDs
    /// without looking them up, so service-level enforcement is the
    /// right boundary.
    fn require_card_exists(&self, id: Uuid) -> KanbanResult<()> {
        match self.backend.get_card(id)? {
            Some(_) => Ok(()),
            None => Err(KanbanError::not_found("Card", id)),
        }
    }
}

impl GraphOperations for KanbanContext {
    fn attach_children(&mut self, parent: Uuid, children: Vec<Uuid>) -> KanbanResult<()> {
        self.require_card_exists(parent)?;
        for child in &children {
            self.require_card_exists(*child)?;
        }
        let commands: Vec<Command> = children
            .into_iter()
            .map(|child| {
                Command::Dependency(DependencyCommand::AddSpawns(AddSpawns {
                    source: parent,
                    target: child,
                    as_archived: false,
                }))
            })
            .collect();
        self.execute(commands)
    }

    fn detach_children(&mut self, parent: Uuid, children: Vec<Uuid>) -> KanbanResult<()> {
        self.require_card_exists(parent)?;
        for child in &children {
            self.require_card_exists(*child)?;
        }
        let commands: Vec<Command> = children
            .into_iter()
            .map(|child| {
                Command::Dependency(DependencyCommand::RemoveSpawns(RemoveSpawns {
                    source: parent,
                    target: child,
                    tolerate_missing: false,
                }))
            })
            .collect();
        self.execute(commands)
    }

    fn list_children_of(&self, parent: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.require_card_exists(parent)?;
        Ok(self.backend.get_graph()?.children(parent))
    }

    fn list_parents_of(&self, child: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.require_card_exists(child)?;
        Ok(self.backend.get_graph()?.parents(child))
    }

    fn block(&mut self, blocker: Uuid, blocked: Uuid, severity: Severity) -> KanbanResult<()> {
        self.require_card_exists(blocker)?;
        self.require_card_exists(blocked)?;
        self.execute(vec![Command::Dependency(DependencyCommand::AddBlocks(
            AddBlocks {
                source: blocker,
                target: blocked,
                severity,
                as_archived: false,
            },
        ))])
    }

    fn unblock(&mut self, blocker: Uuid, blocked: Uuid) -> KanbanResult<()> {
        self.require_card_exists(blocker)?;
        self.require_card_exists(blocked)?;
        self.execute(vec![Command::Dependency(DependencyCommand::RemoveBlocks(
            RemoveBlocks {
                source: blocker,
                target: blocked,
                tolerate_missing: false,
            },
        ))])
    }

    fn list_blocked_by(&self, blocker: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.require_card_exists(blocker)?;
        Ok(self.backend.get_graph()?.blocked(blocker))
    }

    fn list_blockers_of(&self, blocked: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.require_card_exists(blocked)?;
        Ok(self.backend.get_graph()?.blockers(blocked))
    }

    fn relate(&mut self, a: Uuid, b: Uuid, kind: RelatesKind) -> KanbanResult<()> {
        self.require_card_exists(a)?;
        self.require_card_exists(b)?;
        self.execute(vec![Command::Dependency(DependencyCommand::AddRelates(
            AddRelates {
                source: a,
                target: b,
                kind,
                as_archived: false,
            },
        ))])
    }

    fn dissociate(&mut self, a: Uuid, b: Uuid) -> KanbanResult<()> {
        self.require_card_exists(a)?;
        self.require_card_exists(b)?;
        self.execute(vec![Command::Dependency(DependencyCommand::RemoveRelates(
            RemoveRelates {
                source: a,
                target: b,
                tolerate_missing: false,
            },
        ))])
    }

    fn list_related_to(&self, card: Uuid) -> KanbanResult<Vec<Uuid>> {
        self.require_card_exists(card)?;
        Ok(self.backend.get_graph()?.related(card))
    }
}
