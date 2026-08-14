use crate::card_lifecycle::sorted_board_columns;
use crate::{CardStatus, Column};
use uuid::Uuid;

/// A column is a completion column iff its default status is the completion
/// status.
pub fn is_completion_column(column: &Column) -> bool {
    column.default_status == Some(CardStatus::Done)
}

/// All of a board's completion columns, in position order.
pub fn completion_columns(board_id: Uuid, columns: &[Column]) -> Vec<&Column> {
    sorted_board_columns(board_id, columns)
        .into_iter()
        .filter(|c| is_completion_column(c))
        .collect()
}

/// The board's first completion column by position.
pub fn primary_completion_column(board_id: Uuid, columns: &[Column]) -> Option<&Column> {
    completion_columns(board_id, columns).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn make_column(board_id: Uuid, position: i32, default_status: Option<CardStatus>) -> Column {
        let mut col = Column::new(board_id, format!("col-{position}"), position);
        col.default_status = default_status;
        col
    }

    #[test]
    fn test_column_with_done_default_status_is_a_completion_column() {
        let board_id = Uuid::new_v4();
        let col = make_column(board_id, 0, Some(CardStatus::Done));
        assert!(is_completion_column(&col));
    }

    #[test]
    fn test_column_with_other_default_status_is_not_a_completion_column() {
        let board_id = Uuid::new_v4();
        let col = make_column(board_id, 0, Some(CardStatus::Todo));
        assert!(!is_completion_column(&col));
    }

    #[test]
    fn test_column_without_default_status_is_not_a_completion_column() {
        let board_id = Uuid::new_v4();
        let col = make_column(board_id, 0, None);
        assert!(!is_completion_column(&col));
    }

    #[test]
    fn test_completion_columns_returns_matches_in_position_order() {
        let board_id = Uuid::new_v4();
        let col0 = make_column(board_id, 0, Some(CardStatus::Todo));
        let col1 = make_column(board_id, 1, Some(CardStatus::Done));
        let col2 = make_column(board_id, 2, Some(CardStatus::Done));
        let columns = vec![col2.clone(), col0, col1.clone()];

        let result = completion_columns(board_id, &columns);

        assert_eq!(result, vec![&col1, &col2]);
    }

    #[test]
    fn test_completion_columns_tie_breaks_by_created_at_then_id() {
        let board_id = Uuid::new_v4();
        let now = Utc::now();

        let mut earlier = make_column(board_id, 0, Some(CardStatus::Done));
        earlier.created_at = now;
        let mut later = make_column(board_id, 0, Some(CardStatus::Done));
        later.created_at = now + Duration::seconds(1);

        let columns = vec![later.clone(), earlier.clone()];
        let result = completion_columns(board_id, &columns);

        assert_eq!(result, vec![&earlier, &later]);
    }

    #[test]
    fn test_primary_completion_column_is_the_first_by_position() {
        let board_id = Uuid::new_v4();
        let col0 = make_column(board_id, 0, Some(CardStatus::Done));
        let col1 = make_column(board_id, 1, Some(CardStatus::Done));
        let columns = vec![col1, col0.clone()];

        assert_eq!(primary_completion_column(board_id, &columns), Some(&col0));
    }
}
