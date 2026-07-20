//! Board sorting — the board-side analogue of the card sort primitive.
//!
//! Uses the board-specific [`BoardSortField`] (NOT the card [`crate::SortField`]),
//! paired with the shared [`SortOrder`] and its toggle. Boards carry their own
//! `position`; `archived_at` is NOT on the board head (it lives on the archival
//! marker), so recency sorting takes an explicit id → timestamp map.
//!
//! Both [`BoardSortField`] variants are board-meaningful: `Position` (board
//! order) and `ArchivedAt` (recency). There is no card-only fallback path
//! because a card-only field cannot be passed here by construction.

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
    boards.sort_by(|a, b| {
        let primary = compare_boards(field, a, b, archived_at);
        let primary = match order {
            SortOrder::Ascending => primary,
            SortOrder::Descending => primary.reverse(),
        };
        primary.then_with(|| a.position.cmp(&b.position))
    });
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
}
