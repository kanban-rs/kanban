use kanban_domain::{KanbanError, KanbanResult, LoadState};

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
