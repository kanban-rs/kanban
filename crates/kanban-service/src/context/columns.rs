use super::KanbanContext;
use chrono::Utc;
use kanban_domain::commands::{BoardCommand, ColumnCommand, Command, ImportEntities};
use kanban_domain::{Column, ColumnUpdate, FieldUpdate, KanbanError, KanbanResult, NewColumn};
use uuid::Uuid;

/// Result of an idempotent PUT-create ([`KanbanContext::create_or_replace_column`]):
/// the resulting column plus whether this call created it (`true`, HTTP 201) or
/// replaced an existing one (`false`, HTTP 200). The HTTP binding lives in the
/// server seam; the service tier only reports which arm ran.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnCreateOutcome {
    pub column: Column,
    pub created: bool,
}

impl KanbanContext {
    /// Create a column from a full `NewColumn` spec plus an optional
    /// client-supplied id (idempotent PUT-create). Funnels through
    /// `Column::create`: validates the `board_id` FK (missing → `NotFound`),
    /// resolves the id (client value or a fresh mint), enforces id uniqueness
    /// (duplicate → `AlreadyExists`/409), captures the clock once for
    /// undo/redo determinism, and applies the server-assigned append
    /// `position`. Inherent on `KanbanContext` (not a `KanbanOperations` trait
    /// method) — the trait is dual-impl by TUI+CLI and would force churn there.
    pub fn create_column_from_spec(
        &mut self,
        id: Option<Uuid>,
        spec: NewColumn,
    ) -> KanbanResult<Column> {
        self.require_board(spec.board_id)?;
        let id = id.unwrap_or_else(Uuid::new_v4);
        if self.backend.get_column(id)?.is_some() {
            return Err(KanbanError::already_exists("Column", id));
        }
        let now = Utc::now();
        let position = self.backend.list_columns_by_board(spec.board_id)?.len() as i32;
        let column = Column::create(spec, id, position, now)?;
        // Dispatch the single-column create through the import command so the
        // full factory-built column (including `wip_limit`) is persisted
        // atomically; the inverse is a `DeleteColumn` of this id.
        let cmd = Command::Board(BoardCommand::Import(ImportEntities {
            columns: vec![column],
            ..Default::default()
        }));
        self.execute(vec![cmd])?;
        self.get_column_impl(id)?.ok_or_else(|| {
            KanbanError::Internal("Column creation succeeded but column not found".into())
        })
    }

    /// Idempotent PUT-create (create-or-replace) for a column keyed on a
    /// client-supplied `id`: create the column with that id when absent, or
    /// fully replace the content of an existing column with that id. The
    /// returned [`ColumnCreateOutcome::created`] distinguishes the two so the
    /// server seam can answer 201 vs 200. Server-managed `position` is preserved
    /// across the replace arm (only `name`/`wip_limit` are content) — an absent
    /// `wip_limit` clears (wholesale replace). The HTTP binding stays in the
    /// server seam.
    pub fn create_or_replace_column(
        &mut self,
        id: Uuid,
        spec: NewColumn,
    ) -> KanbanResult<ColumnCreateOutcome> {
        if self.backend.get_column(id)?.is_none() {
            let column = self.create_column_from_spec(Some(id), spec)?;
            return Ok(ColumnCreateOutcome {
                column,
                created: true,
            });
        }
        // FK (replace arm): the owning board must exist before we dispatch the
        // update, guarded via the canonical helper (KAN-248). The replace path
        // does not move a column across boards, but a board can be deleted
        // between reads, so the guard stays.
        self.require_board(spec.board_id)?;
        let column = self.update_column_impl(id, replace_update_from_spec(spec))?;
        Ok(ColumnCreateOutcome {
            column,
            created: false,
        })
    }

    pub(super) fn create_column_impl(
        &mut self,
        board_id: Uuid,
        name: String,
        position: Option<i32>,
    ) -> KanbanResult<Column> {
        // Explicit `position` callers (TUI/contract helpers seed columns at a
        // chosen index) keep the legacy command path; the `None` append case
        // routes through the rich spec funnel so it gains FK + id-uniqueness.
        match position {
            Some(position) => {
                use kanban_domain::commands::CreateColumn;
                let id = Uuid::new_v4();
                let cmd = Command::Column(ColumnCommand::Create(CreateColumn {
                    id,
                    board_id,
                    name,
                    position,
                }));
                self.execute(vec![cmd])?;
                self.get_column_impl(id)?.ok_or_else(|| {
                    KanbanError::Internal("Column creation succeeded but column not found".into())
                })
            }
            None => self.create_column_from_spec(
                None,
                NewColumn {
                    board_id,
                    name,
                    wip_limit: None,
                },
            ),
        }
    }

    pub(super) fn list_columns_impl(&self, board_id: Uuid) -> KanbanResult<Vec<Column>> {
        self.backend.list_columns_by_board(board_id)
    }

    pub(super) fn get_column_impl(&self, id: Uuid) -> KanbanResult<Option<Column>> {
        self.backend.get_column(id)
    }

    pub(super) fn update_column_impl(
        &mut self,
        id: Uuid,
        updates: ColumnUpdate,
    ) -> KanbanResult<Column> {
        use kanban_domain::commands::UpdateColumn;
        let cmd = Command::Column(ColumnCommand::Update(UpdateColumn {
            column_id: id,
            updates,
        }));
        self.execute(vec![cmd])?;
        self.get_column_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Column", id))
    }

    pub(super) fn delete_column_impl(&mut self, id: Uuid) -> KanbanResult<()> {
        use kanban_domain::commands::DeleteColumn;
        let cmd = Command::Column(ColumnCommand::Delete(DeleteColumn { column_id: id }));
        self.execute(vec![cmd])
    }

    pub(super) fn reorder_column_impl(
        &mut self,
        id: Uuid,
        new_position: i32,
    ) -> KanbanResult<Column> {
        let updates = ColumnUpdate {
            name: None,
            position: Some(new_position),
            wip_limit: FieldUpdate::NoChange,
        };
        self.update_column_impl(id, updates)
    }
}

/// Map a `NewColumn` create-spec onto a full-replace `ColumnUpdate` (the PUT
/// replace arm of [`KanbanContext::create_or_replace_column`]): `name` is set,
/// `wip_limit` maps `Option`→`FieldUpdate` (`Some`→`Set`, `None`→`Clear`, so an
/// absent field is wiped), and the server-managed `position` is left untouched.
/// `board_id` is the FK key, not editable content, so it is dropped here.
fn replace_update_from_spec(spec: NewColumn) -> ColumnUpdate {
    let NewColumn {
        board_id: _,
        name,
        wip_limit,
    } = spec;
    ColumnUpdate {
        name: Some(name),
        position: None,
        wip_limit: match wip_limit {
            Some(limit) => FieldUpdate::Set(limit),
            None => FieldUpdate::Clear,
        },
    }
}
