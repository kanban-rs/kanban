use crate::data_store::DataStore;
use crate::{Card, DomainError, KanbanError, KanbanResult, Sprint};
use uuid::Uuid;

pub fn require_sprint_on_board(sprint: &Sprint, card_board: Uuid) -> KanbanResult<()> {
    if sprint.board_id == card_board {
        return Ok(());
    }
    Err(KanbanError::Domain(DomainError::SprintBoardMismatch {
        sprint_id: sprint.id,
        sprint_board: sprint.board_id,
        card_board,
    }))
}

/// Drops the card's sprint binding when moving it onto `target_board` would
/// leave it bound to a sprint on another board. A same-board move never
/// touches the binding, so an already-inconsistent binding is left as is.
pub fn detach_sprint_if_board_changes(
    store: &dyn DataStore,
    card: &mut Card,
    target_board: Uuid,
) -> KanbanResult<bool> {
    let Some(sprint_id) = card.sprint_id else {
        return Ok(false);
    };
    if card.board_id == target_board {
        return Ok(false);
    }
    if matches!(store.get_sprint(sprint_id)?, Some(s) if s.board_id == target_board) {
        return Ok(false);
    }
    card.end_current_sprint_log();
    card.sprint_id = None;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_require_sprint_on_board_accepts_the_sprints_own_board() {
        let board_id = Uuid::new_v4();
        let sprint = Sprint::new(board_id, 1, None, Some("Sprint"));
        assert!(require_sprint_on_board(&sprint, board_id).is_ok());
    }

    #[test]
    fn test_require_sprint_on_board_rejects_another_board_with_all_three_ids() {
        let sprint_board = Uuid::new_v4();
        let card_board = Uuid::new_v4();
        let sprint = Sprint::new(sprint_board, 1, None, Some("Sprint"));
        let sprint_id = sprint.id;
        let err = require_sprint_on_board(&sprint, card_board).unwrap_err();
        assert!(err.is_sprint_board_mismatch());
        let msg = err.to_string();
        assert!(msg.contains(&sprint_id.to_string()), "msg: {msg}");
        assert!(msg.contains(&sprint_board.to_string()), "msg: {msg}");
        assert!(msg.contains(&card_board.to_string()), "msg: {msg}");
    }
}
