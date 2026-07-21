use super::KanbanContext;
use chrono::{DateTime, Utc};
use kanban_domain::commands::{ArchiveBoards, BoardCommand, Command, ImportEntities, RestoreBoard};
use kanban_domain::{
    filter_and_sort_boards, ArchivedBoard, ArchivedFilter, Board, BoardListFilter, BoardSortField,
    BoardUpdate, FieldUpdate, KanbanError, KanbanResult, NewBoard, SortOrder,
};
use std::collections::HashMap;
use uuid::Uuid;

/// The built-in board sort applied when the AppConfig sets no default: Position
/// ascending, so the live board list stays in insertion/position order and is
/// byte-identical to `list_boards()` when nothing is configured.
const DEFAULT_BOARD_SORT: (BoardSortField, SortOrder) =
    (BoardSortField::Position, SortOrder::Ascending);

/// Parse an AppConfig `board_sort_field` string into a [`BoardSortField`].
/// Case-insensitive and tolerant of `-`/`_` separators. Unknown strings fall
/// back to `None` so the caller can apply the built-in default.
fn parse_board_sort_field(s: &str) -> Option<BoardSortField> {
    match s.to_lowercase().replace(['-', '_'], "").as_str() {
        "position" => Some(BoardSortField::Position),
        "name" => Some(BoardSortField::Name),
        "createdat" => Some(BoardSortField::CreatedAt),
        "archivedat" => Some(BoardSortField::ArchivedAt),
        _ => None,
    }
}

/// Parse an AppConfig `board_sort_order` string into a [`SortOrder`].
/// Case-insensitive; unknown strings fall back to `None`.
fn parse_sort_order(s: &str) -> Option<SortOrder> {
    match s.to_lowercase().as_str() {
        "asc" | "ascending" => Some(SortOrder::Ascending),
        "desc" | "descending" => Some(SortOrder::Descending),
        _ => None,
    }
}

/// Result of an idempotent PUT-create ([`KanbanContext::create_or_replace_board`]):
/// the resulting board plus whether this call created it (`true`, HTTP 201) or
/// replaced an existing one (`false`, HTTP 200). The HTTP binding lives in the
/// server seam; the service tier only reports which arm ran.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardCreateOutcome {
    pub board: Board,
    pub created: bool,
}

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

    /// Idempotent PUT-create (create-or-replace) for a board keyed on a
    /// client-supplied `id`: create the board with that id when absent, or
    /// fully replace the content of an existing board with that id. The
    /// returned [`BoardCreateOutcome::created`] distinguishes the two so the
    /// server seam can answer 201 vs 200. Server-managed state (`position`,
    /// counters, `active_sprint_id`) is preserved across the replace arm — only
    /// client-settable content is overwritten, wholesale (an absent nullable
    /// field clears). The HTTP binding stays in the server seam.
    pub fn create_or_replace_board(
        &mut self,
        id: Uuid,
        spec: NewBoard,
    ) -> KanbanResult<BoardCreateOutcome> {
        if self.backend.get_board(id)?.is_none() {
            let board = self.create_board_from_spec(Some(id), spec)?;
            return Ok(BoardCreateOutcome {
                board,
                created: true,
            });
        }
        let board = self.update_board_impl(id, replace_update_from_spec(spec))?;
        Ok(BoardCreateOutcome {
            board,
            created: false,
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

    /// Selector-aware board gather, mirroring `filter_cards`: gathers live
    /// heads via `list_boards` and/or archived heads via the archive markers
    /// (`get_board` is unfiltered, so it resolves an archived head). The
    /// `LiveOnly` path is byte-identical to `list_boards_impl`.
    pub(super) fn list_boards_filtered_impl(
        &self,
        filter: BoardListFilter,
    ) -> KanbanResult<Vec<Board>> {
        let mut out = Vec::new();
        if filter.archived != ArchivedFilter::ArchivedOnly {
            out.extend(self.backend.list_boards()?);
        }
        // Harvest archived_at per board id (needed both to resolve archived heads
        // and to sort by the ArchivedAt dimension), mirroring how the TUI does it.
        let markers = self.backend.list_archived_boards()?;
        let archived_at: HashMap<Uuid, DateTime<Utc>> = markers
            .iter()
            .map(|m| (m.entity_id, m.metadata.archived_at))
            .collect();
        if filter.archived != ArchivedFilter::LiveOnly {
            for m in &markers {
                if let Some(b) = self.backend.get_board(m.entity_id)? {
                    out.push(b);
                }
            }
        }
        // Request sort/sort_order override the AppConfig default via
        // `resolve_board_sort` (inside `filter_and_sort_boards`).
        let default = self.board_sort_default();
        Ok(filter_and_sort_boards(
            &out,
            &filter,
            &archived_at,
            Some(default),
        ))
    }

    /// Resolve the board sort default from the AppConfig `board_sort_field` /
    /// `board_sort_order`, falling back to [`DEFAULT_BOARD_SORT`] (Position ASC)
    /// for any unset or unrecognized value. An unset field with a set order (or
    /// vice versa) layers onto the built-in default's other half.
    fn board_sort_default(&self) -> (BoardSortField, SortOrder) {
        let field = self
            .app_config
            .board_sort_field
            .as_deref()
            .and_then(parse_board_sort_field)
            .unwrap_or(DEFAULT_BOARD_SORT.0);
        let order = self
            .app_config
            .board_sort_order
            .as_deref()
            .and_then(parse_sort_order)
            .unwrap_or(DEFAULT_BOARD_SORT.1);
        (field, order)
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

    /// Archive a board (collection move). Undoable via the command's symmetric
    /// inverse. NotFound if the board is not live.
    pub(super) fn archive_board_impl(&mut self, id: Uuid) -> KanbanResult<()> {
        if self.backend.get_board(id)?.is_none() {
            return Err(KanbanError::not_found("Board", id));
        }
        let cmd = Command::Board(BoardCommand::Archive(ArchiveBoards { ids: vec![id] }));
        self.execute(vec![cmd])
    }

    /// Restore an archived board back into the live set. NotFound if the board
    /// is not in the archived collection.
    pub(super) fn restore_board_impl(&mut self, id: Uuid) -> KanbanResult<()> {
        if self.backend.get_archived_board(id)?.is_none() {
            return Err(KanbanError::not_found("archived board", id));
        }
        let cmd = Command::Board(BoardCommand::Restore(RestoreBoard { board_id: id }));
        self.execute(vec![cmd])
    }

    pub(super) fn list_archived_boards_impl(&self) -> KanbanResult<Vec<ArchivedBoard>> {
        self.backend.list_archived_boards()
    }
}

/// Map a `NewBoard` create-spec onto a true full-replace `BoardUpdate` (the PUT
/// replace arm of [`KanbanContext::create_or_replace_board`]): nullable fields
/// map `Option`→`FieldUpdate` (`Some`→`Set`, `None`→`Clear`, so an absent field
/// is wiped), and the non-nullable sort/view fields fall back to the same
/// defaults `Board::create` applies. Server-managed fields are left untouched.
fn replace_update_from_spec(spec: NewBoard) -> BoardUpdate {
    use kanban_domain::{SortField, SortOrder};
    let NewBoard {
        name,
        description,
        sprint_prefix,
        card_prefix,
        task_sort_field,
        task_sort_order,
        sprint_duration_days,
        task_list_view,
        completion_column_id,
    } = spec;
    BoardUpdate {
        name: Some(name),
        description: description.into(),
        sprint_prefix: sprint_prefix.into(),
        card_prefix: card_prefix.into(),
        task_sort_field: Some(task_sort_field.unwrap_or(SortField::Default)),
        task_sort_order: Some(task_sort_order.unwrap_or(SortOrder::Ascending)),
        sprint_duration_days: sprint_duration_days.into(),
        task_list_view: Some(task_list_view.unwrap_or_default()),
        completion_column_id: completion_column_id.into(),
        // Server-managed — never overwritten by a content replace:
        active_sprint_id: FieldUpdate::NoChange,
        position: None,
    }
}
