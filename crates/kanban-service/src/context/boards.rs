use super::KanbanContext;
use chrono::Utc;
use kanban_domain::commands::{BoardCommand, Command, ImportEntities};
use kanban_domain::{Board, BoardUpdate, KanbanError, KanbanResult, NewBoard};
use uuid::Uuid;

impl KanbanContext {
    /// Create a board from a full `NewBoard` spec plus an optional client-supplied
    /// id (idempotent PUT-create). Funnels through `Board::create`: resolves the
    /// id (client value or a fresh mint), enforces id uniqueness (duplicate →
    /// `AlreadyExists`/409), captures the clock once for undo/redo determinism,
    /// and applies the server-managed `position`. Inherent on `KanbanContext`
    /// (not a `KanbanOperations` trait method) — the trait is dual-impl by
    /// TUI+CLI and would force churn there.
    pub fn create_board_from_spec(
        &mut self,
        id: Option<Uuid>,
        spec: NewBoard,
    ) -> KanbanResult<Board> {
        let id = id.unwrap_or_else(Uuid::new_v4);
        if self.backend.get_board(id)?.is_some() {
            return Err(KanbanError::already_exists("Board", id));
        }
        let now = Utc::now();
        let position = self.backend.list_boards()?.len() as i32;
        let mut board = Board::create(spec, id, now)?;
        board.position = position;
        // Board has no parent FK — dispatch the single-board create through the
        // import command (correct inverse: DeleteBoard of this id).
        let cmd = Command::Board(BoardCommand::Import(ImportEntities {
            boards: vec![board],
            ..Default::default()
        }));
        self.execute(vec![cmd])?;
        self.get_board_impl(id)?.ok_or_else(|| {
            KanbanError::Internal("Board creation succeeded but board not found".into())
        })
    }

    /// Thin shim over [`create_board_from_spec`](Self::create_board_from_spec)
    /// taking just `name`/`card_prefix`, so the existing trait callers do not
    /// churn. The service mints the id; the remaining create fields default.
    pub(super) fn create_board_impl(
        &mut self,
        name: String,
        card_prefix: Option<String>,
    ) -> KanbanResult<Board> {
        let spec = NewBoard {
            name,
            description: None,
            sprint_prefix: None,
            card_prefix,
            task_sort_field: None,
            task_sort_order: None,
            sprint_duration_days: None,
            task_list_view: None,
            completion_column_id: None,
        };
        self.create_board_from_spec(None, spec)
    }

    pub(super) fn list_boards_impl(&self) -> KanbanResult<Vec<Board>> {
        self.backend.list_boards()
    }

    pub(super) fn get_board_impl(&self, id: Uuid) -> KanbanResult<Option<Board>> {
        self.backend.get_board(id)
    }

    pub(super) fn update_board_impl(
        &mut self,
        id: Uuid,
        updates: BoardUpdate,
    ) -> KanbanResult<Board> {
        use kanban_domain::commands::UpdateBoard;
        let cmd = Command::Board(BoardCommand::Update(UpdateBoard {
            board_id: id,
            updates,
        }));
        self.execute(vec![cmd])?;
        self.get_board_impl(id)?
            .ok_or_else(|| KanbanError::not_found("Board", id))
    }

    pub(super) fn delete_board_impl(&mut self, id: Uuid) -> KanbanResult<()> {
        let commands = crate::cascade::delete_board(self.backend.as_data_store(), id)?;
        self.execute(commands)
    }
}
