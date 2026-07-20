//! Board sorting — the board-side analogue of the card sort primitive.
//!
//! Reuses the shared [`SortField`]/[`SortOrder`] enums (the same toggle the
//! task list uses) rather than a parallel sort mechanism. Boards carry their
//! own `position`; `archived_at` is NOT on the board head (it lives on the
//! archival marker), so recency sorting takes an explicit id → timestamp map.
//!
//! Only the board-meaningful fields are supported ([`SortField::Position`] and
//! [`SortField::ArchivedAt`]); any other field falls back to position order so
//! the projects panel never lands in an undefined ordering.

use crate::{Board, SortField, SortOrder};
use chrono::{DateTime, Utc};
use std::cmp::Ordering;
use std::collections::HashMap;
use uuid::Uuid;

/// Compare two boards on a board-meaningful sort field.
///
/// `ArchivedAt` resolves each board's id through `archived_at`; a board with
/// no entry sorts as the epoch minimum so untracked boards never displace
/// tracked ones under recency. Any non-board field (a card-only sort key)
/// falls back to position so ordering is always defined.
fn compare_boards(
    field: SortField,
    a: &Board,
    b: &Board,
    archived_at: &HashMap<Uuid, DateTime<Utc>>,
) -> Ordering {
    match field {
        SortField::ArchivedAt => {
            let at = |id: &Uuid| {
                archived_at
                    .get(id)
                    .copied()
                    .unwrap_or(DateTime::<Utc>::MIN_UTC)
            };
            at(&a.id).cmp(&at(&b.id))
        }
        _ => a.position.cmp(&b.position),
    }
}

/// Sort a slice of boards in place by `field`/`order`, reusing the shared
/// [`SortField`]/[`SortOrder`] enums. Ties on the primary key are broken by
/// ascending `position` (kept ascending even under a descending primary so
/// toggling direction does not reshuffle tied boards), matching the card
/// sorter's stability guarantee.
pub fn sort_boards_in_place(
    boards: &mut [Board],
    field: SortField,
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
