//! Board sorting — the board-side analogue of the card sort primitive.
//!
//! Uses the board-specific [`BoardSortField`] (NOT the card [`crate::SortField`]),
//! paired with the shared [`SortOrder`] and its toggle. Boards carry their own
//! `position`; `archived_at` is NOT on the board head (it lives on the archival
//! marker), so recency sorting takes an explicit id → timestamp map.
//!
//! Every [`BoardSortField`] variant is board-meaningful: `Position` (board
//! order), `Name` (case-insensitive), `CreatedAt`, and `ArchivedAt` (recency).
//! There is no card-only fallback path because a card-only field cannot be
//! passed here by construction.

use crate::sort::sort_by_with_order;
use crate::{Board, BoardSortField, SortOrder};
use chrono::{DateTime, Utc};
use std::cmp::Ordering;
use std::collections::HashMap;
use uuid::Uuid;

/// Compare two boards on a board sort field.
///
/// `ArchivedAt` resolves each board's id through `archived_at`; a board with
/// no entry sorts as the epoch minimum so untracked boards never displace
/// tracked ones under recency.
fn compare_boards(
    field: BoardSortField,
    a: &Board,
    b: &Board,
    archived_at: &HashMap<Uuid, DateTime<Utc>>,
) -> Ordering {
    match field {
        BoardSortField::ArchivedAt => {
            let at = |id: &Uuid| {
                archived_at
                    .get(id)
                    .copied()
                    .unwrap_or(DateTime::<Utc>::MIN_UTC)
            };
            at(&a.id).cmp(&at(&b.id))
        }
        BoardSortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        BoardSortField::CreatedAt => a.created_at.cmp(&b.created_at),
        BoardSortField::Position => a.position.cmp(&b.position),
    }
}

/// Sort a slice of boards in place by `field`/`order`, using the board-specific
/// [`BoardSortField`] and the shared [`SortOrder`]. Ties on the primary key are
/// broken by ascending `position` (kept ascending even under a descending
/// primary so toggling direction does not reshuffle tied boards), matching the
/// card sorter's stability guarantee.
pub fn sort_boards_in_place(
    boards: &mut [Board],
    field: BoardSortField,
    order: SortOrder,
    archived_at: &HashMap<Uuid, DateTime<Utc>>,
) {
    sort_by_with_order(
        boards,
        order,
        |a, b| compare_boards(field, a, b, archived_at),
        |a, b| a.position.cmp(&b.position),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board_at_position(name: &str, position: i32) -> Board {
        let mut b = Board::new(name, None::<String>);
        b.position = position;
        b
    }

    fn ts(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn test_sort_boards_by_position_matches_board_order() {
        let a = board_at_position("A", 2);
        let b = board_at_position("B", 0);
        let c = board_at_position("C", 1);
        let empty = HashMap::new();
        let mut boards = vec![a, b, c];
        sort_boards_in_place(
            &mut boards,
            BoardSortField::Position,
            SortOrder::Ascending,
            &empty,
        );
        assert_eq!(
            boards.iter().map(|x| x.name.clone()).collect::<Vec<_>>(),
            vec!["B", "C", "A"]
        );
    }

    #[test]
    fn test_sort_boards_by_archived_at_descending_is_recency_order() {
        let older = board_at_position("Older", 0);
        let newer = board_at_position("Newer", 1);
        let mut archived_at = HashMap::new();
        archived_at.insert(older.id, ts("2026-01-01T00:00:00Z"));
        archived_at.insert(newer.id, ts("2026-06-01T00:00:00Z"));

        // Input in position order (older first); recency-desc must flip it.
        let mut boards = vec![older, newer];
        sort_boards_in_place(
            &mut boards,
            BoardSortField::ArchivedAt,
            SortOrder::Descending,
            &archived_at,
        );
        assert_eq!(
            boards.iter().map(|x| x.name.clone()).collect::<Vec<_>>(),
            vec!["Newer", "Older"]
        );
    }

    #[test]
    fn test_sort_boards_by_archived_at_ascending_is_oldest_first() {
        let older = board_at_position("Older", 1);
        let newer = board_at_position("Newer", 0);
        let mut archived_at = HashMap::new();
        archived_at.insert(older.id, ts("2026-01-01T00:00:00Z"));
        archived_at.insert(newer.id, ts("2026-06-01T00:00:00Z"));

        let mut boards = vec![newer, older];
        sort_boards_in_place(
            &mut boards,
            BoardSortField::ArchivedAt,
            SortOrder::Ascending,
            &archived_at,
        );
        assert_eq!(
            boards.iter().map(|x| x.name.clone()).collect::<Vec<_>>(),
            vec!["Older", "Newer"]
        );
    }

    #[test]
    fn test_sort_boards_by_name_ascending_is_case_insensitive() {
        let a = board_at_position("Charlie", 0);
        let b = board_at_position("alpha", 1);
        let c = board_at_position("Bravo", 2);
        let empty = HashMap::new();
        let mut boards = vec![a, b, c];
        sort_boards_in_place(
            &mut boards,
            BoardSortField::Name,
            SortOrder::Ascending,
            &empty,
        );
        assert_eq!(
            boards.iter().map(|x| x.name.clone()).collect::<Vec<_>>(),
            vec!["alpha", "Bravo", "Charlie"]
        );
    }

    #[test]
    fn test_sort_boards_by_name_descending_reverses_case_insensitive_order() {
        let a = board_at_position("Charlie", 0);
        let b = board_at_position("alpha", 1);
        let c = board_at_position("Bravo", 2);
        let empty = HashMap::new();
        let mut boards = vec![a, b, c];
        sort_boards_in_place(
            &mut boards,
            BoardSortField::Name,
            SortOrder::Descending,
            &empty,
        );
        assert_eq!(
            boards.iter().map(|x| x.name.clone()).collect::<Vec<_>>(),
            vec!["Charlie", "Bravo", "alpha"]
        );
    }

    #[test]
    fn test_sort_boards_by_created_at_ascending_is_oldest_first() {
        let mut older = board_at_position("Older", 0);
        let mut newer = board_at_position("Newer", 1);
        older.created_at = ts("2026-01-01T00:00:00Z");
        newer.created_at = ts("2026-06-01T00:00:00Z");
        let empty = HashMap::new();
        let mut boards = vec![newer, older];
        sort_boards_in_place(
            &mut boards,
            BoardSortField::CreatedAt,
            SortOrder::Ascending,
            &empty,
        );
        assert_eq!(
            boards.iter().map(|x| x.name.clone()).collect::<Vec<_>>(),
            vec!["Older", "Newer"]
        );
    }

    #[test]
    fn test_sort_boards_ties_break_by_position_ascending() {
        // Equal archived_at → deterministic position order, even under a
        // descending primary (tiebreaker stays ascending).
        let at = Utc::now();
        let first = board_at_position("First", 0);
        let second = board_at_position("Second", 1);
        let mut archived_at = HashMap::new();
        archived_at.insert(first.id, at);
        archived_at.insert(second.id, at);

        let mut boards = vec![second, first];
        sort_boards_in_place(
            &mut boards,
            BoardSortField::ArchivedAt,
            SortOrder::Descending,
            &archived_at,
        );
        assert_eq!(
            boards.iter().map(|x| x.name.clone()).collect::<Vec<_>>(),
            vec!["First", "Second"]
        );
    }

    #[test]
    fn test_sort_boards_by_position_ties_break_by_created_at_then_id() {
        // field == Position is the degenerate case: the old tiebreak (also
        // `position`) was the same key as the primary comparator, so a real
        // tie previously resolved to nothing but input-slice order.
        let mut older = board_at_position("Older", 3);
        let mut newer = board_at_position("Newer", 3);
        older.created_at = ts("2026-01-01T00:00:00Z");
        newer.created_at = ts("2026-06-01T00:00:00Z");
        let empty = HashMap::new();

        for input in [
            vec![newer.clone(), older.clone()],
            vec![older.clone(), newer.clone()],
        ] {
            let mut boards = input;
            sort_boards_in_place(
                &mut boards,
                BoardSortField::Position,
                SortOrder::Ascending,
                &empty,
            );
            assert_eq!(
                boards.iter().map(|x| x.name.clone()).collect::<Vec<_>>(),
                vec!["Older", "Newer"]
            );
        }
    }

    #[test]
    fn test_sort_boards_by_position_ties_break_by_id_when_created_at_also_equal() {
        let same_time = ts("2026-01-01T00:00:00Z");
        let mut a = board_at_position("A", 3);
        let mut b = board_at_position("B", 3);
        a.created_at = same_time;
        b.created_at = same_time;
        let empty = HashMap::new();
        let expected = if a.id < b.id {
            vec![a.id, b.id]
        } else {
            vec![b.id, a.id]
        };

        let mut boards = vec![b, a];
        sort_boards_in_place(
            &mut boards,
            BoardSortField::Position,
            SortOrder::Ascending,
            &empty,
        );
        assert_eq!(boards.iter().map(|x| x.id).collect::<Vec<_>>(), expected);
    }
}
