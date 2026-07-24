//! Card lifecycle business rules.
//!
//! Pure functions that encode the relationship between card status,
//! column position, and completion state. Used by both TUI and API
//! to ensure consistent behavior.

use crate::{Board, Card, CardStatus, Column, Sprint, SprintLog};
use std::collections::HashSet;
use std::hash::Hash;
use uuid::Uuid;

/// Return the input slice with duplicates removed, preserving the order of
/// the first occurrence of each element. Useful for batch APIs that need
/// stable, dedup-tolerant input.
pub fn dedup_preserving_order<T: Hash + Eq + Copy>(items: &[T]) -> Vec<T> {
    let mut seen: HashSet<T> = HashSet::new();
    items
        .iter()
        .copied()
        .filter(|item| seen.insert(*item))
        .collect()
}

/// Compute the target positions for a batch move of cards into a column.
///
/// Cards whose IDs are in `moving_ids` are placed at the tail of the column,
/// in their input order, starting at the position right after the last
/// non-moving card. Returns `(card_id, target_position)` pairs in the order
/// of the first occurrence of each id in `moving_ids` (duplicates are
/// dropped via [`dedup_preserving_order`]).
///
/// This is a pure function: it does no I/O and takes the current column
/// contents as a snapshot input. The service layer is responsible for
/// reading `existing_cards` and persisting the resulting positions.
pub fn compute_move_positions(existing_cards: &[Card], moving_ids: &[Uuid]) -> Vec<(Uuid, i32)> {
    let deduped = dedup_preserving_order(moving_ids);
    let moving_set: HashSet<Uuid> = deduped.iter().copied().collect();
    let base = existing_cards
        .iter()
        .filter(|c| !moving_set.contains(&c.id))
        .count();
    deduped
        .into_iter()
        .enumerate()
        .map(|(i, id)| (id, (base + i) as i32))
        .collect()
}

/// Direction for moving a card between columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDirection {
    Left,
    Right,
}

/// Result of computing a completion toggle.
#[derive(Debug, Clone)]
pub struct CompletionToggleResult {
    pub new_status: CardStatus,
    pub target_column_id: Uuid,
    pub new_position: i32,
}

/// Result of computing a column move.
#[derive(Debug, Clone)]
pub struct CardMoveResult {
    pub target_column_id: Uuid,
    pub new_position: i32,
    /// If Some, the card's status should be changed to this value.
    pub new_status: Option<CardStatus>,
}

/// Get a board's columns sorted by position, then `created_at`, then `id`
/// (`position` alone is not unique across a board's columns).
pub fn sorted_board_columns(board_id: Uuid, columns: &[Column]) -> Vec<&Column> {
    let mut cols: Vec<_> = columns.iter().filter(|c| c.board_id == board_id).collect();
    cols.sort_by(|a, b| {
        a.position
            .cmp(&b.position)
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.id.cmp(&b.id))
    });
    cols
}

/// Count cards in a column and return the next append position.
pub fn next_position_in_column(cards: &[Card], column_id: Uuid) -> i32 {
    cards.iter().filter(|c| c.column_id == column_id).count() as i32
}

/// Compute what should happen when toggling a card's completion state.
///
/// - If the card is Done → move to second-to-last column, set Todo
/// - If the card is not Done → move to completion column, set Done
///
/// Returns `None` if the board has fewer than 2 columns.
pub fn compute_completion_toggle(
    card: &Card,
    board: &Board,
    columns: &[Column],
    cards: &[Card],
) -> Option<CompletionToggleResult> {
    let sorted = sorted_board_columns(board.id, columns);
    if sorted.len() < 2 {
        return None;
    }

    let completion_col_id = board.resolve_completion_column(columns)?;
    let current_idx = sorted.iter().position(|c| c.id == card.column_id)?;

    if card.status == CardStatus::Done {
        // Moving from Done → Todo: go to second-to-last column
        let is_in_completion_col =
            sorted.iter().position(|c| c.id == completion_col_id) == Some(current_idx);

        if is_in_completion_col && sorted.len() > 1 {
            let completion_idx = sorted.iter().position(|c| c.id == completion_col_id)?;
            // Target: one column before the completion column
            let target_idx = if completion_idx > 0 {
                completion_idx - 1
            } else {
                // Completion column is first (unusual) — stay put
                return None;
            };
            let target_col = sorted[target_idx];
            let new_position = next_position_in_column(cards, target_col.id);
            Some(CompletionToggleResult {
                new_status: CardStatus::Todo,
                target_column_id: target_col.id,
                new_position,
            })
        } else {
            // Card is Done but not in completion column — just toggle status, no move
            None
        }
    } else {
        // Moving to Done: go to completion column
        if completion_col_id == card.column_id {
            // Already in completion column — just toggle status, no column move needed
            return None;
        }
        let new_position = next_position_in_column(cards, completion_col_id);
        Some(CompletionToggleResult {
            new_status: CardStatus::Done,
            target_column_id: completion_col_id,
            new_position,
        })
    }
}

/// Compute the result of moving a card left or right between columns.
///
/// Returns `None` if there is no column in that direction.
/// Includes a status change if moving to/from the completion column.
pub fn compute_card_column_move(
    card: &Card,
    board: &Board,
    columns: &[Column],
    cards: &[Card],
    direction: MoveDirection,
) -> Option<CardMoveResult> {
    let sorted = sorted_board_columns(board.id, columns);
    let current_idx = sorted.iter().position(|c| c.id == card.column_id)?;

    let target_idx = match direction {
        MoveDirection::Left => {
            if current_idx == 0 {
                return None;
            }
            current_idx - 1
        }
        MoveDirection::Right => {
            if current_idx >= sorted.len() - 1 {
                return None;
            }
            current_idx + 1
        }
    };

    let target_col = sorted[target_idx];
    let new_position = next_position_in_column(cards, target_col.id);

    let completion_col_id = board.resolve_completion_column(columns);
    let new_status = if sorted.len() > 1 {
        if let Some(comp_id) = completion_col_id {
            let is_moving_to_completion = target_col.id == comp_id;
            let is_moving_from_completion =
                sorted.get(current_idx).is_some_and(|c| c.id == comp_id);

            if is_moving_to_completion && card.status != CardStatus::Done {
                Some(CardStatus::Done)
            } else if is_moving_from_completion && card.status == CardStatus::Done {
                Some(CardStatus::Todo)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    Some(CardMoveResult {
        target_column_id: target_col.id,
        new_position,
        new_status,
    })
}

/// Compute the column where a card should live given its new status,
/// to maintain the status ↔ completion column invariant.
///
/// Returns `Some(target_column_id)` if the card must move. Position within
/// the target column is the caller's responsibility (typically "append at end").
/// Returns `None` if no move is needed (already correctly placed, or board
/// has fewer than 2 columns, or no completion column is resolvable).
pub fn target_column_for_status(
    card: &Card,
    new_status: CardStatus,
    board: &Board,
    columns: &[Column],
) -> Option<Uuid> {
    let sorted = sorted_board_columns(board.id, columns);
    if sorted.len() < 2 {
        return None;
    }
    let completion_col_id = board.resolve_completion_column(columns)?;

    if new_status == CardStatus::Done {
        if card.column_id == completion_col_id {
            return None;
        }
        Some(completion_col_id)
    } else {
        if card.column_id != completion_col_id {
            return None;
        }
        let completion_idx = sorted.iter().position(|c| c.id == completion_col_id)?;
        if completion_idx == 0 {
            return None;
        }
        Some(sorted[completion_idx - 1].id)
    }
}

/// Compute the status a card should have after being moved to `new_column_id`,
/// to maintain the status ↔ completion column invariant.
///
/// Returns `Some(new_status)` if status must change, `None` otherwise.
pub fn target_status_for_column_move(
    card: &Card,
    new_column_id: Uuid,
    board: &Board,
    columns: &[Column],
) -> Option<CardStatus> {
    let completion_col_id = board.resolve_completion_column(columns)?;
    let moving_to_completion = new_column_id == completion_col_id;
    let was_in_completion = card.column_id == completion_col_id;

    if moving_to_completion && card.status != CardStatus::Done {
        Some(CardStatus::Done)
    } else if !moving_to_completion && was_in_completion && card.status == CardStatus::Done {
        Some(CardStatus::Todo)
    } else {
        None
    }
}

/// Compact card positions in a column to be sequential (0, 1, 2, ...).
pub fn compact_column_positions(cards: &mut [Card], column_id: Uuid) {
    let mut indices: Vec<usize> = cards
        .iter()
        .enumerate()
        .filter(|(_, c)| c.column_id == column_id)
        .map(|(i, _)| i)
        .collect();

    // Sort by current position to maintain order
    indices.sort_by_key(|&i| cards[i].position);

    for (new_pos, &idx) in indices.iter().enumerate() {
        cards[idx].position = new_pos as i32;
    }
}

/// Determine if a new card created in the given column should be auto-completed.
///
/// Returns true when the column is the completion column and the board has more than 2 columns.
pub fn should_auto_complete_new_card(column_id: Uuid, board: &Board, columns: &[Column]) -> bool {
    let board_cols = sorted_board_columns(board.id, columns);
    if board_cols.len() <= 2 {
        return false;
    }
    board.resolve_completion_column(columns) == Some(column_id)
}

/// Resolve which column to restore an archived card into.
///
/// If the original column still exists (for this board), use it.
/// Otherwise, fall back to the first column of the board.
/// Returns `None` if there are no columns for the board.
pub fn resolve_restore_column(
    original_column_id: Uuid,
    board_id: Uuid,
    columns: &[Column],
) -> Option<Uuid> {
    let board_cols = sorted_board_columns(board_id, columns);
    if board_cols.iter().any(|c| c.id == original_column_id) {
        Some(original_column_id)
    } else {
        board_cols.first().map(|c| c.id)
    }
}

/// Backfill sprint_logs for cards that have a sprint_id but empty logs.
///
/// Returns the count of cards that were migrated.
pub fn migrate_sprint_logs(cards: &mut [Card], sprints: &[Sprint], boards: &[Board]) -> usize {
    let mut migrated = 0;

    for card in cards.iter_mut() {
        if let Some(sprint_id) = card.sprint_id {
            if card.sprint_logs.is_empty() {
                if let Some(sprint) = sprints.iter().find(|s| s.id == sprint_id) {
                    let sprint_name = sprint.name_index.and_then(|idx| {
                        boards
                            .iter()
                            .find(|b| b.id == sprint.board_id)
                            .and_then(|board| board.sprint_names.get(idx).cloned())
                    });
                    let log = SprintLog::new(
                        sprint_id,
                        sprint.sprint_number,
                        sprint_name,
                        format!("{:?}", sprint.status),
                    );
                    card.sprint_logs.push(log);
                    migrated += 1;
                }
            }
        }
    }

    migrated
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Board;

    fn test_board() -> Board {
        Board::new("Test", None::<String>)
    }

    fn add_columns(board: &Board, names: &[&str]) -> Vec<Column> {
        names
            .iter()
            .enumerate()
            .map(|(i, name)| Column::new(board.id, name.to_string(), i as i32))
            .collect()
    }

    fn test_card(board: &mut Board, column: &Column, title: &str, position: i32) -> Card {
        Card::new(board, column.id, title.to_string(), position)
    }

    // --- sorted_board_columns ---

    #[test]
    fn sorted_board_columns_returns_sorted() {
        let board = test_board();
        let mut cols = add_columns(&board, &["C", "A", "B"]);
        cols[0].position = 2;
        cols[1].position = 0;
        cols[2].position = 1;

        let sorted = sorted_board_columns(board.id, &cols);
        assert_eq!(sorted[0].name, "A");
        assert_eq!(sorted[1].name, "B");
        assert_eq!(sorted[2].name, "C");
    }

    #[test]
    fn sorted_board_columns_filters_by_board() {
        let board = test_board();
        let other_board = test_board();
        let mut cols = add_columns(&board, &["Mine"]);
        cols.push(Column::new(other_board.id, "Other", 0));

        let sorted = sorted_board_columns(board.id, &cols);
        assert_eq!(sorted.len(), 1);
        assert_eq!(sorted[0].name, "Mine");
    }

    #[test]
    fn sorted_board_columns_ties_break_by_created_at_then_id() {
        let board = test_board();
        let mut older = Column::new(board.id, "Older".to_string(), 3);
        let mut newer = Column::new(board.id, "Newer".to_string(), 3);
        older.created_at = "2026-01-01T00:00:00Z".parse().unwrap();
        newer.created_at = "2026-06-01T00:00:00Z".parse().unwrap();

        for cols in [
            vec![newer.clone(), older.clone()],
            vec![older.clone(), newer.clone()],
        ] {
            let sorted = sorted_board_columns(board.id, &cols);
            assert_eq!(
                sorted.iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
                vec!["Older", "Newer"]
            );
        }
    }

    #[test]
    fn sorted_board_columns_ties_break_by_id_when_created_at_also_equal() {
        let board = test_board();
        let same_time: chrono::DateTime<chrono::Utc> = "2026-01-01T00:00:00Z".parse().unwrap();
        let mut a = Column::new(board.id, "A".to_string(), 3);
        let mut b = Column::new(board.id, "B".to_string(), 3);
        a.created_at = same_time;
        b.created_at = same_time;
        let expected = if a.id < b.id {
            vec![a.id, b.id]
        } else {
            vec![b.id, a.id]
        };

        let cols = vec![b, a];
        let sorted = sorted_board_columns(board.id, &cols);
        assert_eq!(sorted.iter().map(|c| c.id).collect::<Vec<_>>(), expected);
    }

    // --- compute_completion_toggle ---

    #[test]
    fn toggle_todo_to_done_moves_to_last_column() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Todo", "In Progress", "Done"]);
        let card = test_card(&mut board, &cols[0], "Task", 0);

        let result =
            compute_completion_toggle(&card, &board, &cols, std::slice::from_ref(&card)).unwrap();
        assert_eq!(result.new_status, CardStatus::Done);
        assert_eq!(result.target_column_id, cols[2].id);
    }

    #[test]
    fn toggle_done_to_todo_moves_to_second_to_last() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Todo", "In Progress", "Done"]);
        let mut card = test_card(&mut board, &cols[2], "Task", 0);
        card.status = CardStatus::Done;

        let result =
            compute_completion_toggle(&card, &board, &cols, std::slice::from_ref(&card)).unwrap();
        assert_eq!(result.new_status, CardStatus::Todo);
        assert_eq!(result.target_column_id, cols[1].id);
    }

    #[test]
    fn toggle_returns_none_for_single_column() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Only"]);
        let card = test_card(&mut board, &cols[0], "Task", 0);

        assert!(
            compute_completion_toggle(&card, &board, &cols, std::slice::from_ref(&card)).is_none()
        );
    }

    #[test]
    fn toggle_uses_explicit_completion_column() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Backlog", "Done", "Archive"]);
        // Set "Done" (middle column) as completion column
        board.update_completion_column_id(Some(cols[1].id));

        let card = test_card(&mut board, &cols[0], "Task", 0);

        let result =
            compute_completion_toggle(&card, &board, &cols, std::slice::from_ref(&card)).unwrap();
        assert_eq!(result.new_status, CardStatus::Done);
        assert_eq!(result.target_column_id, cols[1].id);
    }

    #[test]
    fn toggle_done_with_explicit_column_moves_to_previous() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Backlog", "Done", "Archive"]);
        board.update_completion_column_id(Some(cols[1].id));

        let mut card = test_card(&mut board, &cols[1], "Task", 0);
        card.status = CardStatus::Done;

        let result =
            compute_completion_toggle(&card, &board, &cols, std::slice::from_ref(&card)).unwrap();
        assert_eq!(result.new_status, CardStatus::Todo);
        assert_eq!(result.target_column_id, cols[0].id);
    }

    // --- compute_card_column_move ---

    #[test]
    fn move_right_to_last_column_marks_done() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Todo", "Done"]);
        let card = test_card(&mut board, &cols[0], "Task", 0);

        let result = compute_card_column_move(
            &card,
            &board,
            &cols,
            std::slice::from_ref(&card),
            MoveDirection::Right,
        )
        .unwrap();
        assert_eq!(result.target_column_id, cols[1].id);
        assert_eq!(result.new_status, Some(CardStatus::Done));
    }

    #[test]
    fn move_left_from_last_column_marks_todo() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Todo", "Done"]);
        let mut card = test_card(&mut board, &cols[1], "Task", 0);
        card.status = CardStatus::Done;

        let result = compute_card_column_move(
            &card,
            &board,
            &cols,
            std::slice::from_ref(&card),
            MoveDirection::Left,
        )
        .unwrap();
        assert_eq!(result.target_column_id, cols[0].id);
        assert_eq!(result.new_status, Some(CardStatus::Todo));
    }

    #[test]
    fn move_right_at_rightmost_returns_none() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Todo", "Done"]);
        let card = test_card(&mut board, &cols[1], "Task", 0);

        assert!(compute_card_column_move(
            &card,
            &board,
            &cols,
            std::slice::from_ref(&card),
            MoveDirection::Right
        )
        .is_none());
    }

    #[test]
    fn move_left_at_leftmost_returns_none() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Todo", "Done"]);
        let card = test_card(&mut board, &cols[0], "Task", 0);

        assert!(compute_card_column_move(
            &card,
            &board,
            &cols,
            std::slice::from_ref(&card),
            MoveDirection::Left
        )
        .is_none());
    }

    #[test]
    fn move_between_middle_columns_no_status_change() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Todo", "In Progress", "Done"]);
        let card = test_card(&mut board, &cols[0], "Task", 0);

        let result = compute_card_column_move(
            &card,
            &board,
            &cols,
            std::slice::from_ref(&card),
            MoveDirection::Right,
        )
        .unwrap();
        assert_eq!(result.target_column_id, cols[1].id);
        assert_eq!(result.new_status, None);
    }

    #[test]
    fn move_appends_to_end_of_target_column() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Todo", "Done"]);
        let existing = test_card(&mut board, &cols[1], "Existing", 0);
        let card = test_card(&mut board, &cols[0], "New", 0);

        let cards = vec![existing, card.clone()];
        let result =
            compute_card_column_move(&card, &board, &cols, &cards, MoveDirection::Right).unwrap();
        assert_eq!(result.new_position, 1); // appended after existing card
    }

    // --- compact_column_positions ---

    #[test]
    fn compact_resequences_positions() {
        let mut board = test_board();
        let col = Column::new(board.id, "Todo", 0);
        let mut cards = vec![
            test_card(&mut board, &col, "A", 0),
            test_card(&mut board, &col, "B", 5),
            test_card(&mut board, &col, "C", 10),
        ];

        compact_column_positions(&mut cards, col.id);
        assert_eq!(cards[0].position, 0);
        assert_eq!(cards[1].position, 1);
        assert_eq!(cards[2].position, 2);
    }

    #[test]
    fn compact_only_affects_target_column() {
        let mut board = test_board();
        let col1 = Column::new(board.id, "Todo", 0);
        let col2 = Column::new(board.id, "Done", 1);
        let mut cards = vec![
            test_card(&mut board, &col1, "A", 5),
            test_card(&mut board, &col2, "B", 99),
        ];

        compact_column_positions(&mut cards, col1.id);
        assert_eq!(cards[0].position, 0);
        assert_eq!(cards[1].position, 99); // untouched
    }

    // --- should_auto_complete_new_card ---

    #[test]
    fn auto_complete_true_when_in_completion_column() {
        let board = test_board();
        let cols = add_columns(&board, &["Todo", "In Progress", "Done"]);

        assert!(should_auto_complete_new_card(cols[2].id, &board, &cols));
    }

    #[test]
    fn auto_complete_false_when_not_completion_column() {
        let board = test_board();
        let cols = add_columns(&board, &["Todo", "In Progress", "Done"]);

        assert!(!should_auto_complete_new_card(cols[0].id, &board, &cols));
    }

    #[test]
    fn auto_complete_false_with_two_or_fewer_columns() {
        let board = test_board();
        let cols = add_columns(&board, &["Todo", "Done"]);

        assert!(!should_auto_complete_new_card(cols[1].id, &board, &cols));
    }

    // --- resolve_restore_column ---

    #[test]
    fn restore_to_original_column_when_exists() {
        let board = test_board();
        let cols = add_columns(&board, &["Todo", "Done"]);

        assert_eq!(
            resolve_restore_column(cols[1].id, board.id, &cols),
            Some(cols[1].id)
        );
    }

    #[test]
    fn restore_falls_back_to_first_column() {
        let board = test_board();
        let cols = add_columns(&board, &["Todo", "Done"]);
        let missing_id = Uuid::new_v4();

        assert_eq!(
            resolve_restore_column(missing_id, board.id, &cols),
            Some(cols[0].id)
        );
    }

    #[test]
    fn restore_returns_none_when_no_columns() {
        let board = test_board();

        assert_eq!(resolve_restore_column(Uuid::new_v4(), board.id, &[]), None);
    }

    // --- migrate_sprint_logs ---

    #[test]
    fn migrate_backfills_empty_sprint_logs() {
        let mut board = test_board();
        let col = Column::new(board.id, "Todo", 0);
        let sprint = Sprint::new(board.id, 1, None, None::<String>);
        let mut card = test_card(&mut board, &col, "Task", 0);
        card.sprint_id = Some(sprint.id);

        let mut cards = vec![card];
        let count = migrate_sprint_logs(&mut cards, &[sprint], &[board]);

        assert_eq!(count, 1);
        assert_eq!(cards[0].sprint_logs.len(), 1);
        assert_eq!(cards[0].sprint_logs[0].sprint_number, 1);
    }

    #[test]
    fn migrate_skips_cards_with_existing_logs() {
        let mut board = test_board();
        let col = Column::new(board.id, "Todo", 0);
        let sprint = Sprint::new(board.id, 1, None, None::<String>);
        let mut card = test_card(&mut board, &col, "Task", 0);
        card.sprint_id = Some(sprint.id);
        card.sprint_logs
            .push(SprintLog::new(sprint.id, 1, None::<String>, "Active"));

        let mut cards = vec![card];
        let count = migrate_sprint_logs(&mut cards, &[sprint], &[board]);

        assert_eq!(count, 0);
        assert_eq!(cards[0].sprint_logs.len(), 1);
    }

    #[test]
    fn migrate_skips_cards_without_sprint() {
        let mut board = test_board();
        let col = Column::new(board.id, "Todo", 0);
        let card = test_card(&mut board, &col, "Task", 0);

        let mut cards = vec![card];
        let count = migrate_sprint_logs(&mut cards, &[], &[board]);

        assert_eq!(count, 0);
    }

    #[test]
    fn migrate_with_mixed_cards_only_backfills_eligible() {
        let mut board = test_board();
        let col = Column::new(board.id, "Todo", 0);
        let sprint = Sprint::new(board.id, 1, None, None::<String>);

        let mut card_needs_backfill = test_card(&mut board, &col, "Needs Backfill", 0);
        card_needs_backfill.sprint_id = Some(sprint.id);

        let mut card_already_logged = test_card(&mut board, &col, "Already Logged", 1);
        card_already_logged.sprint_id = Some(sprint.id);
        card_already_logged.sprint_logs.push(SprintLog::new(
            sprint.id,
            1,
            None::<String>,
            "Active",
        ));
        let already_logged_before = card_already_logged.sprint_logs.clone();

        let card_no_sprint = test_card(&mut board, &col, "No Sprint", 2);
        let no_sprint_before = card_no_sprint.sprint_logs.clone();

        let mut cards = vec![card_needs_backfill, card_already_logged, card_no_sprint];
        let count = migrate_sprint_logs(&mut cards, &[sprint], &[board]);

        assert_eq!(count, 1, "only the eligible card should be migrated");
        assert_eq!(cards[0].sprint_logs.len(), 1);
        assert_eq!(cards[0].sprint_logs[0].sprint_number, 1);
        assert_eq!(
            cards[1].sprint_logs, already_logged_before,
            "card with existing logs should be untouched"
        );
        assert_eq!(
            cards[2].sprint_logs, no_sprint_before,
            "card with no sprint_id should be untouched"
        );
    }

    // --- compute_move_positions ---

    #[test]
    fn compute_move_positions_appends_after_existing_non_moving_cards() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Col"]);
        let existing1 = test_card(&mut board, &cols[0], "E1", 0);
        let existing2 = test_card(&mut board, &cols[0], "E2", 1);
        let existing = vec![existing1, existing2];

        let move_a = Uuid::new_v4();
        let move_b = Uuid::new_v4();

        let positions = compute_move_positions(&existing, &[move_a, move_b]);

        assert_eq!(positions, vec![(move_a, 2), (move_b, 3)]);
    }

    #[test]
    fn compute_move_positions_within_same_column_excludes_moving_from_base() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Col"]);
        let card1 = test_card(&mut board, &cols[0], "C1", 0);
        let card2 = test_card(&mut board, &cols[0], "C2", 1);
        let card3 = test_card(&mut board, &cols[0], "C3", 2);
        let c1 = card1.id;
        let c3 = card3.id;
        let existing = vec![card1, card2, card3];

        // Move c1 and c3; c2 stays — base = 1 (only c2 is non-moving)
        let positions = compute_move_positions(&existing, &[c1, c3]);

        assert_eq!(positions, vec![(c1, 1), (c3, 2)]);
    }

    #[test]
    fn compute_move_positions_into_empty_column_starts_at_zero() {
        let move_a = Uuid::new_v4();
        let move_b = Uuid::new_v4();

        let positions = compute_move_positions(&[], &[move_a, move_b]);

        assert_eq!(positions, vec![(move_a, 0), (move_b, 1)]);
    }

    #[test]
    fn compute_move_positions_with_empty_moving_returns_empty() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Col"]);
        let existing = vec![test_card(&mut board, &cols[0], "E", 0)];

        let positions = compute_move_positions(&existing, &[]);

        assert!(positions.is_empty());
    }

    #[test]
    fn compute_move_positions_preserves_input_order() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Col"]);
        let existing = vec![test_card(&mut board, &cols[0], "E", 0)];

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let positions = compute_move_positions(&existing, &[id3, id1, id2]);

        assert_eq!(positions, vec![(id3, 1), (id1, 2), (id2, 3)]);
    }

    #[test]
    fn compute_move_positions_dedupes_repeated_moving_ids_first_occurrence_wins() {
        let mut board = test_board();
        let cols = add_columns(&board, &["Col"]);
        let existing = vec![test_card(&mut board, &cols[0], "E", 0)];

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();

        // Duplicates of id_a and id_b: only first occurrence kept.
        let positions = compute_move_positions(&existing, &[id_a, id_b, id_a, id_b, id_a]);

        // base = 1 (one existing non-moving), only two unique moving ids → positions 1, 2.
        assert_eq!(positions, vec![(id_a, 1), (id_b, 2)]);
    }

    // --- dedup_preserving_order ---

    #[test]
    fn dedup_preserving_order_empty_input_returns_empty() {
        let out: Vec<u32> = dedup_preserving_order(&[]);
        assert!(out.is_empty());
    }

    #[test]
    fn dedup_preserving_order_no_duplicates_returns_input_in_order() {
        let out = dedup_preserving_order(&[3u32, 1, 2]);
        assert_eq!(out, vec![3, 1, 2]);
    }

    #[test]
    fn dedup_preserving_order_all_duplicates_returns_single_first() {
        let out = dedup_preserving_order(&[7u32, 7, 7, 7]);
        assert_eq!(out, vec![7]);
    }

    #[test]
    fn dedup_preserving_order_mixed_preserves_first_occurrence_order() {
        // First-occurrence positions: a=0, b=1, c=3 → output order [a, b, c].
        let out = dedup_preserving_order(&[1u32, 2, 1, 3, 2, 1, 3]);
        assert_eq!(out, vec![1, 2, 3]);
    }

    #[test]
    fn dedup_preserving_order_works_for_uuid() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();
        let out = dedup_preserving_order(&[a, b, a, c, b]);
        assert_eq!(out, vec![a, b, c]);
    }
}
