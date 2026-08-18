//! Resolves a sprint's display `name` against its owning board.

use kanban_domain::{KanbanError, KanbanOperations, KanbanResult, Sprint};
use uuid::Uuid;

pub fn resolve_sprint_name<O: KanbanOperations + ?Sized>(
    ops: &O,
    sprint: &Sprint,
) -> KanbanResult<Option<String>> {
    let board = ops
        .get_board(sprint.board_id)?
        .ok_or_else(|| KanbanError::not_found("Board", sprint.board_id))?;
    Ok(sprint.get_name(&board).map(str::to_string))
}

/// Resolves every sprint's `name` against a single read of `board_id`'s
/// board. Every sprint in `sprints` must belong to `board_id`; this does not
/// re-check each sprint's `board_id`.
pub fn resolve_sprint_names<O: KanbanOperations + ?Sized>(
    ops: &O,
    board_id: Uuid,
    sprints: &[Sprint],
) -> KanbanResult<Vec<Option<String>>> {
    let board = ops
        .get_board(board_id)?
        .ok_or_else(|| KanbanError::not_found("Board", board_id))?;
    Ok(sprints
        .iter()
        .map(|s| s.get_name(&board).map(str::to_string))
        .collect())
}
