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

/// Write-side counterpart of [`resolve_sprint_name`] for a sprint the caller
/// has already committed. A sprint returned without a `name_index` is read
/// again by id first, because a backend that could not see the board's name
/// pool at write time returns it unnamed. Any failed read degrades the name
/// to `None` with a warning instead of failing the caller.
pub fn resolve_committed_sprint_name<O: KanbanOperations + ?Sized>(
    ops: &O,
    sprint: &Sprint,
) -> Option<String> {
    let reread;
    let sprint = if sprint.name_index.is_some() {
        sprint
    } else {
        match ops.get_sprint(sprint.id) {
            Ok(Some(fresh)) if fresh.name_index.is_some() => {
                reread = fresh;
                &reread
            }
            Ok(Some(_)) => return None,
            Ok(None) => {
                warn_committed_sprint_unnamed(sprint, &KanbanError::not_found("Sprint", sprint.id));
                return None;
            }
            Err(e) => {
                warn_committed_sprint_unnamed(sprint, &e);
                return None;
            }
        }
    };
    match resolve_sprint_name(ops, sprint) {
        Ok(name) => name,
        Err(e) => {
            warn_committed_sprint_unnamed(sprint, &e);
            None
        }
    }
}

fn warn_committed_sprint_unnamed(sprint: &Sprint, error: &KanbanError) {
    tracing::warn!(
        board_id = %sprint.board_id,
        sprint_id = %sprint.id,
        error = %error,
        "sprint write committed but its name lookup failed; reporting it unnamed"
    );
}
