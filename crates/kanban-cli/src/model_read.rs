use kanban_domain::{KanbanError, KanbanOperations, KanbanResult, LoadState};
use uuid::Uuid;

pub(crate) fn require_loaded<'a, T>(state: &'a LoadState<T>, what: &str) -> KanbanResult<&'a T> {
    match state {
        LoadState::Loaded(value) => Ok(value),
        LoadState::NotLoaded => Err(KanbanError::Internal(format!(
            "{what} was not fetched for this command"
        ))),
        LoadState::Missing => Err(KanbanError::Internal(format!("{what} is unavailable"))),
        LoadState::Failed(e) => Err(KanbanError::Internal(format!("{what}: {e}"))),
    }
}

pub(crate) fn resolve_column_with_optional_board(
    ops: &impl KanbanOperations,
    raw: &str,
    board: Option<&str>,
) -> KanbanResult<Uuid> {
    if let Ok(uuid) = Uuid::parse_str(raw) {
        return Ok(uuid);
    }
    let Some(board_raw) = board else {
        return Err(board_required_for_column_name(raw));
    };
    let board_id = ops.resolve_board_id(board_raw)?;
    ops.resolve_column_id(raw, board_id)
}

fn board_required_for_column_name(raw: &str) -> KanbanError {
    KanbanError::validation(format!(
        "resolving column '{raw}' by name requires a board: columns belong to a board and column names are not unique across boards. Pass --board, or pass the column's UUID."
    ))
}

pub(crate) fn resolve_sprint_with_optional_board(
    ops: &impl KanbanOperations,
    raw: &str,
    board: Option<&str>,
) -> KanbanResult<Uuid> {
    if let Ok(uuid) = Uuid::parse_str(raw) {
        return Ok(uuid);
    }
    let Some(board_raw) = board else {
        return Err(board_required_for_sprint_name(raw));
    };
    let board_id = ops.resolve_board_id(board_raw)?;
    ops.resolve_sprint_id(raw, board_id)
}

fn board_required_for_sprint_name(raw: &str) -> KanbanError {
    KanbanError::validation(format!(
        "resolving sprint '{raw}' by name or number requires a board: a sprint's name is an index into its board's name pool and sprint numbers are not unique across boards. Pass --board, or pass the sprint's UUID."
    ))
}
