use crate::{DomainError, KanbanError, KanbanResult, Sprint};
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
