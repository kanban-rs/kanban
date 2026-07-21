use super::KanbanContext;
use kanban_domain::commands::{Command, SprintCommand};
use kanban_domain::{
    Board, DataStore, FieldUpdate, KanbanError, KanbanResult, Snapshot, Sprint, SprintUpdate,
};
use kanban_persistence::PersistenceError;
use uuid::Uuid;

/// Result of an idempotent PUT-create ([`KanbanContext::create_or_replace_sprint`]):
/// the resulting sprint plus whether this call created it (`true`, HTTP 201) or
/// replaced an existing one (`false`, HTTP 200). The HTTP binding lives in the
/// server seam; the service tier only reports which arm ran.
#[derive(Debug, Clone, PartialEq)]
pub struct SprintCreateOutcome {
    pub sprint: Sprint,
    pub created: bool,
}

impl KanbanContext {
    pub(super) fn carry_over_sprint_cards_impl(
        &mut self,
        from_sprint_id: Uuid,
        to_sprint_id: Uuid,
    ) -> KanbanResult<usize> {
        use kanban_domain::query::sprint::get_sprint_uncompleted_cards;

        let from_sprint = self
            .get_sprint_impl(from_sprint_id)?
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
            .get_sprint_impl(to_sprint_id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", to_sprint_id))?;
        if to_sprint.status != kanban_domain::SprintStatus::Planning {
            return Err(KanbanError::validation(format!(
                "Target sprint must be Planning, got {:?}",
                to_sprint.status
            )));
        }

        // C3b: live-scoped — never carry over an archived board's cards.
        let all_cards = self.list_live_cards_impl()?;
        let ids: Vec<Uuid> = get_sprint_uncompleted_cards(from_sprint_id, &all_cards)
            .iter()
            .map(|c| c.id)
            .collect();
        self.assign_cards_to_sprint_impl(ids, to_sprint_id)
    }

    /// Create a sprint from its create content plus an optional client-supplied
    /// `id` (idempotent PUT-create entry point). Validates the `board_id` FK
    /// (missing → `NotFound`) and the client-supplied id (already present →
    /// `AlreadyExists`/409) BEFORE the DUAL minting runs, so a rejected create
    /// leaves no side effect. The `sprint_number` (from the board's counters)
    /// and `name_index` (from the board's name pool) are minted inside the
    /// frozen `CreateSprint` command's execute, which now funnels construction
    /// through `Sprint::create` and persists the board (counters/pool) before
    /// the sprint. Inherent on `KanbanContext` (not a `KanbanOperations` trait
    /// method) — the trait is dual-impl by TUI+CLI and would force churn there.
    pub fn create_sprint_from_spec(
        &mut self,
        board_id: Uuid,
        id: Option<Uuid>,
        name: Option<String>,
        prefix: Option<String>,
        auto_consume_name: bool,
    ) -> KanbanResult<Sprint> {
        use kanban_domain::commands::CreateSprint;

        // FK: the owning board must exist before we mint anything.
        if self.backend.get_board(board_id)?.is_none() {
            return Err(KanbanError::not_found("Board", board_id));
        }

        // Client-supplied id uniqueness → conflict (idempotent PUT-create entry
        // point); validate before the mint so a collision has no side effect.
        let id = id.unwrap_or_else(Uuid::new_v4);
        if self.backend.get_sprint(id)?.is_some() {
            return Err(KanbanError::already_exists("Sprint", id));
        }

        let default_sprint_prefix = self
            .app_config
            .effective_default_sprint_prefix()
            .to_string();

        // Dispatch the frozen command (it mints sprint_number/name_index from
        // the board, funnels through `Sprint::create`, and keeps the
        // upsert_board-before-upsert_sprint ordering its capture_inverse relies
        // on). The service supplies the resolved id.
        let cmd = Command::Sprint(SprintCommand::Create(CreateSprint {
            id,
            board_id,
            name,
            default_sprint_prefix,
            explicit_prefix: prefix,
            auto_consume_name,
        }));
        self.execute(vec![cmd])?;
        self.get_sprint_impl(id)?.ok_or_else(|| {
            KanbanError::Internal("Sprint creation succeeded but sprint not found".into())
        })
    }

    /// Idempotent PUT-create (create-or-replace) for a sprint keyed on a
    /// client-supplied `id`: create the sprint with that id when absent, or
    /// replace the client-settable content (`name`/`prefix`) of an existing
    /// sprint with that id. The returned [`SprintCreateOutcome::created`]
    /// distinguishes the two so the server seam can answer 201 vs 200.
    /// Server-managed state (`sprint_number`, status, dates) is preserved across
    /// the replace arm. The HTTP binding stays in the server seam.
    pub fn create_or_replace_sprint(
        &mut self,
        board_id: Uuid,
        id: Uuid,
        name: Option<String>,
        prefix: Option<String>,
        auto_consume_name: bool,
    ) -> KanbanResult<SprintCreateOutcome> {
        if self.backend.get_sprint(id)?.is_none() {
            let sprint =
                self.create_sprint_from_spec(board_id, Some(id), name, prefix, auto_consume_name)?;
            return Ok(SprintCreateOutcome {
                sprint,
                created: true,
            });
        }
        let updates = SprintUpdate {
            name,
            prefix: match prefix {
                Some(p) => FieldUpdate::Set(p),
                None => FieldUpdate::Clear,
            },
            // Server-managed / lifecycle — never overwritten by a content
            // replace; name allocation is driven by `name` above.
            name_index: FieldUpdate::NoChange,
            card_prefix: FieldUpdate::NoChange,
            status: None,
            start_date: FieldUpdate::NoChange,
            end_date: FieldUpdate::NoChange,
        };
        let sprint = self.update_sprint_impl(id, updates)?;
        Ok(SprintCreateOutcome {
            sprint,
            created: false,
        })
    }

    /// Thin shim over [`create_sprint_from_spec`](Self::create_sprint_from_spec)
    /// for the legacy `(board_id, prefix, name)` create path, so the existing
    /// trait callers do not churn. The service mints the id; CLI/MCP semantics
    /// (no auto-consume of pooled names) are preserved.
    pub(super) fn create_sprint_impl(
        &mut self,
        board_id: Uuid,
        prefix: Option<String>,
        name: Option<String>,
    ) -> KanbanResult<Sprint> {
        self.create_sprint_from_spec(board_id, None, name, prefix, false)
    }

    pub(super) fn list_sprints_impl(&self, board_id: Uuid) -> KanbanResult<Vec<Sprint>> {
        self.backend.list_sprints_by_board(board_id)
    }

    pub(super) fn get_sprint_impl(&self, id: Uuid) -> KanbanResult<Option<Sprint>> {
        self.backend.get_sprint(id)
    }

    pub(super) fn update_sprint_impl(
        &mut self,
        id: Uuid,
        updates: SprintUpdate,
    ) -> KanbanResult<Sprint> {
        use kanban_domain::commands::UpdateSprint;
        let cmd = Command::Sprint(SprintCommand::Update(UpdateSprint {
            sprint_id: id,
            updates,
        }));
        self.execute(vec![cmd])?;
        self.get_sprint_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", id))
    }

    pub(super) fn activate_sprint_impl(
        &mut self,
        id: Uuid,
        duration_days: Option<i32>,
    ) -> KanbanResult<Sprint> {
        use kanban_domain::commands::ActivateSprint;
        let duration = duration_days.unwrap_or(14) as u32;
        let cmd = Command::Sprint(SprintCommand::Activate(ActivateSprint {
            sprint_id: id,
            duration_days: duration,
        }));
        self.execute(vec![cmd])?;
        self.get_sprint_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", id))
    }

    pub(super) fn complete_sprint_impl(&mut self, id: Uuid) -> KanbanResult<Sprint> {
        use kanban_domain::commands::CompleteSprint;
        let cmd = Command::Sprint(SprintCommand::Complete(CompleteSprint { sprint_id: id }));
        self.execute(vec![cmd])?;
        self.get_sprint_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", id))
    }

    pub(super) fn cancel_sprint_impl(&mut self, id: Uuid) -> KanbanResult<Sprint> {
        use kanban_domain::commands::CancelSprint;
        let cmd = Command::Sprint(SprintCommand::Cancel(CancelSprint { sprint_id: id }));
        self.execute(vec![cmd])?;
        self.get_sprint_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Sprint", id))
    }

    pub(super) fn delete_sprint_impl(&mut self, id: Uuid) -> KanbanResult<()> {
        use kanban_domain::commands::DeleteSprint;
        let cmd = Command::Sprint(SprintCommand::Delete(DeleteSprint {
            sprint_id: id,
            timestamp: chrono::Utc::now(),
        }));
        self.execute(vec![cmd])
    }

    pub(super) fn export_board_impl(&self, board_id: Option<Uuid>) -> KanbanResult<String> {
        let snapshot = if let Some(id) = board_id {
            // C3b FIDELITY: raw unfiltered board-head read — `list_boards` is
            // live-scoped and would drop an archived board, so a single-board export
            // of an archived board would carry no board head. `get_board` is
            // unfiltered.
            let boards: Vec<_> = self.backend.get_board(id)?.into_iter().collect();
            let columns = self.backend.list_columns_by_board(id)?;
            let column_ids: Vec<_> = columns.iter().map(|c| c.id).collect();
            // KAN-938: gather the board's archived cards by board_id (marker-based,
            // deleted-column-safe) at parity with the full-export path
            // (BoardExporter::export_board).
            let archived_cards = self.backend.list_archived_cards_by_board(id)?;
            let archived_card_ids: std::collections::HashSet<_> =
                archived_cards.iter().map(|ac| ac.entity_id).collect();
            // C3b FIDELITY: raw read — exporting a board (even an archived one)
            // must include its full subtree; do NOT live-scope here. `list_all_cards`
            // hides archived cards (F1 marker model), so carry their live rows too
            // (fetched unfiltered by id), even when their column was deleted after
            // archival (dangling column_id), so the markers are not orphaned on
            // import.
            let mut cards: Vec<_> = self
                .backend
                .list_all_cards()?
                .into_iter()
                .filter(|c| column_ids.contains(&c.column_id))
                .collect();
            let live_ids: std::collections::HashSet<_> = cards.iter().map(|c| c.id).collect();
            for ac_id in &archived_card_ids {
                if !live_ids.contains(ac_id) {
                    if let Some(card) = self.backend.get_card(*ac_id)? {
                        cards.push(card);
                    }
                }
            }
            // If the board itself is archived, carry its marker.
            let archived_boards = self.backend.get_archived_board(id)?.into_iter().collect();
            let sprints = self.backend.list_sprints_by_board(id)?;
            let graph = self.backend.get_graph()?;
            Snapshot {
                archived_boards,
                boards,
                columns,
                cards,
                archived_cards,
                sprints,
                graph,
            }
        } else {
            self.backend.snapshot()?
        };

        serde_json::to_string_pretty(&snapshot)
            .map_err(|e| PersistenceError::Serialization(e.to_string()).into())
    }

    pub(super) fn import_board_impl(&mut self, data: &str) -> KanbanResult<Board> {
        use kanban_domain::commands::BoardCommand;
        use kanban_domain::commands::{Command, CommandContext, ImportEntities};
        use std::collections::HashSet;

        let mut imported: Snapshot = serde_json::from_str(data)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let board = imported
            .boards
            .first()
            .cloned()
            .ok_or_else(|| KanbanError::validation("No board in import data"))?;

        // C3b FIDELITY: raw read — import dedup must see ALL columns
        // (live AND archived-board) to reject id collisions.
        let existing_columns = self.backend.list_all_columns()?;
        let imported_column_ids: HashSet<Uuid> = imported.columns.iter().map(|c| c.id).collect();
        let existing_column_ids: HashSet<Uuid> = existing_columns.iter().map(|c| c.id).collect();

        // Backfill board_id on archived-card markers that predate the first-class
        // field (they deserialize to nil when the `board_id` key is absent).
        // Reference-marker model: the marker references the live card by
        // `entity_id`; reconstruct board_id via that card's column -> board using
        // the imported cards + columns unioned with the existing store. Leave nil
        // only when it resolves nowhere.
        let col_to_board: std::collections::HashMap<Uuid, Uuid> = imported
            .columns
            .iter()
            .chain(existing_columns.iter())
            .map(|c| (c.id, c.board_id))
            .collect();
        let existing_cards = self.backend.list_all_cards()?;
        let card_to_column: std::collections::HashMap<Uuid, Uuid> = imported
            .cards
            .iter()
            .chain(existing_cards.iter())
            .map(|c| (c.id, c.column_id))
            .collect();
        for ac in &mut imported.archived_cards {
            if ac.context.board_id.is_nil() {
                if let Some(board_id) = card_to_column
                    .get(&ac.entity_id)
                    .and_then(|col_id| col_to_board.get(col_id))
                {
                    ac.context.board_id = *board_id;
                }
            }
        }

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
            archived_boards: imported.archived_boards,
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
