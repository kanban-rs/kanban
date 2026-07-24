//! Card list filter shapes and the in-memory filter+sort engine.
//!
//! Consumers see two layers:
//!
//! - **Filter shape** ([`CardListFilter`]) — the
//!   request a caller hands to the service or the engine.
//! - **Engine** ([`filter_and_sort_cards`], [`count_filtered_cards`]) — runs
//!   the request against an in-memory slice. Generic over `Borrow<Card>` so
//!   `Card` and `ArchivedCard` both flow through one predicate.
//!
//! `KanbanContext::list_cards` (kanban-service) delegates here, so the
//! three frontends (CLI, MCP, TUI) inherit one filter+sort path.

use crate::search::{CardSearcher, CompositeSearcher};
use crate::sort::{resolve_sort, sort_boards_in_place, sort_cards_in_place};
use crate::{Board, BoardSortField, Card, CardStatus, Column, SortField, SortOrder, Sprint};
use chrono::{DateTime, Utc};
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Three-state selector over a card's archival status for the unified card list.
/// The default is [`LiveOnly`](ArchivedFilter::LiveOnly); a
/// `CardListFilter::default()` caller sees the pre-selector card set, save for
/// the service-tier C3b exclusion of archived-BOARD descendants on unscoped
/// reads (a no-op when no board is archived).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ArchivedFilter {
    /// Only live (non-archived) cards. The default. Byte-identical to the
    /// pre-selector behavior EXCEPT that, at the service tier, an unscoped
    /// read also excludes archived-BOARD descendants (C3b); with no archived
    /// board the two are identical.
    #[default]
    LiveOnly,
    /// Only individually-archived cards.
    ArchivedOnly,
    /// Both live and archived cards (the union).
    Include,
}

#[derive(Default, Clone)]
pub struct CardListFilter {
    pub board_id: Option<Uuid>,
    pub column_id: Option<Uuid>,
    /// Any-of sprint membership. Pass `Some([sid].into())` for a single
    /// sprint, or a multi-element set for the TUI's sprint-chip filter.
    pub sprint_ids: Option<HashSet<Uuid>>,
    pub hide_assigned: bool,
    pub status: Option<CardStatus>,
    /// `CompositeSearcher::all` semantics; empty string is a no-op.
    pub search: Option<String>,
    pub sort: Option<SortField>,
    pub sort_order: Option<SortOrder>,
    /// Three-state archival selector. Defaults to `LiveOnly`, so callers that
    /// build the filter with `..Default::default()` are unaffected.
    pub archived: ArchivedFilter,
}

/// Board list request shape, mirroring [`CardListFilter`]. Carries the
/// three-state [`ArchivedFilter`] so board listing takes the same selector
/// cards do; the service tier (B2) gathers the live-vs-archived board set,
/// exactly as it does for cards. Also carries the optional sort override
/// (field + order) the picker sets; when absent, the service supplies the
/// AppConfig default to [`filter_and_sort_boards`].
#[derive(Default, Clone)]
pub struct BoardListFilter {
    /// Three-state archival selector. Defaults to `LiveOnly`, so callers that
    /// build the filter with `..Default::default()` see the pre-selector set.
    pub archived: ArchivedFilter,
    /// Optional board sort dimension override. `None` falls back to the
    /// `default` passed to [`filter_and_sort_boards`] (the AppConfig value).
    pub sort: Option<BoardSortField>,
    /// Optional sort direction override, resolved alongside `sort`.
    pub sort_order: Option<SortOrder>,
}

fn allowed_column_ids(columns: &[Column], board_id: Option<Uuid>) -> Option<HashSet<Uuid>> {
    board_id.map(|bid| {
        columns
            .iter()
            .filter(|c| c.board_id == bid)
            .map(|c| c.id)
            .collect()
    })
}

fn build_searcher(filter: &CardListFilter) -> Option<CompositeSearcher> {
    filter
        .search
        .as_deref()
        .filter(|q| !q.is_empty())
        .map(|q| CompositeSearcher::all(q.to_string()))
}

fn passes_filter(
    card: &Card,
    allowed_columns: Option<&HashSet<Uuid>>,
    searcher: Option<&CompositeSearcher>,
    board: Option<&Board>,
    sprints: &[Sprint],
    filter: &CardListFilter,
) -> bool {
    if let Some(allowed) = allowed_columns {
        if !allowed.contains(&card.column_id) {
            return false;
        }
    }
    if let Some(column_id) = filter.column_id {
        if card.column_id != column_id {
            return false;
        }
    }
    if let Some(ref ids) = filter.sprint_ids {
        if !ids.is_empty() {
            match card.sprint_id {
                Some(sid) if ids.contains(&sid) => {}
                _ => return false,
            }
        }
    }
    if filter.hide_assigned && card.sprint_id.is_some() {
        return false;
    }
    if let Some(status) = filter.status {
        if card.status != status {
            return false;
        }
    }
    if let Some(searcher) = searcher {
        let Some(board) = board else { return true };
        if !searcher.matches(card, board, sprints) {
            return false;
        }
    }
    true
}

/// Single filter + sort entry point for in-memory card slices, generic
/// over anything that borrows a `Card` (so archived cards flow through
/// the same predicate via their `Borrow<Card>` impl).
pub fn filter_and_sort_cards<T: Borrow<Card> + Clone>(
    cards: &[T],
    columns: &[Column],
    sprints: &[Sprint],
    board: Option<&Board>,
    filter: &CardListFilter,
) -> Vec<T> {
    let allowed = allowed_column_ids(columns, filter.board_id);
    let searcher = build_searcher(filter);
    let mut result: Vec<T> = cards
        .iter()
        .filter(|c| {
            passes_filter(
                (*c).borrow(),
                allowed.as_ref(),
                searcher.as_ref(),
                board,
                sprints,
                filter,
            )
        })
        .cloned()
        .collect();
    if let Some((field, order)) = resolve_sort(filter.sort, filter.sort_order, board) {
        sort_cards_in_place(&mut result, field, order);
    }
    result
}

/// Count-only variant that shares the predicate without allocating a
/// result vector or sorting. Used by the TUI badge/count render path.
pub fn count_filtered_cards<T: Borrow<Card>>(
    cards: &[T],
    columns: &[Column],
    sprints: &[Sprint],
    board: Option<&Board>,
    filter: &CardListFilter,
) -> usize {
    let allowed = allowed_column_ids(columns, filter.board_id);
    let searcher = build_searcher(filter);
    cards
        .iter()
        .filter(|c| {
            passes_filter(
                (*c).borrow(),
                allowed.as_ref(),
                searcher.as_ref(),
                board,
                sprints,
                filter,
            )
        })
        .count()
}

/// Resolve `(field, order)` for the board list from a caller override and an
/// optional default. Mirrors [`resolve_sort`], but the fallback is the passed
/// `default` (the AppConfig board-list value threaded by the service) rather
/// than a `Board` entity: override wins; else `default`; else `None`.
///
/// A field override with no explicit order borrows the default's order (or
/// ascending when there is no default); an order override with no field layers
/// onto the default's field.
pub fn resolve_board_sort(
    sort: Option<BoardSortField>,
    order: Option<SortOrder>,
    default: Option<(BoardSortField, SortOrder)>,
) -> Option<(BoardSortField, SortOrder)> {
    match (sort, order, default) {
        (Some(f), Some(o), _) => Some((f, o)),
        (Some(f), None, Some((_, o))) => Some((f, o)),
        (Some(f), None, None) => Some((f, SortOrder::Ascending)),
        (None, override_order, Some((f, o))) => Some((f, override_order.unwrap_or(o))),
        (None, _, None) => None,
    }
}

/// Single filter + sort entry point for in-memory board slices, mirroring
/// [`filter_and_sort_cards`]. Boards have no column/sprint predicates (their
/// archival split is handled upstream at the service tier), so this is
/// predominantly the sort: it resolves `(field, order)` via
/// [`resolve_board_sort`] and applies the shared board sort primitive.
pub fn filter_and_sort_boards(
    boards: &[Board],
    filter: &BoardListFilter,
    archived_at: &HashMap<Uuid, DateTime<Utc>>,
    default: Option<(BoardSortField, SortOrder)>,
) -> Vec<Board> {
    let mut result: Vec<Board> = boards.to_vec();
    if let Some((field, order)) = resolve_board_sort(filter.sort, filter.sort_order, default) {
        sort_boards_in_place(&mut result, field, order, archived_at);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_archived_filter_default_is_live_only() {
        assert_eq!(ArchivedFilter::default(), ArchivedFilter::LiveOnly);
    }

    #[test]
    fn test_card_list_filter_default_archived_is_live_only() {
        assert_eq!(CardListFilter::default().archived, ArchivedFilter::LiveOnly);
    }

    #[test]
    fn test_board_list_filter_defaults_to_liveonly() {
        assert_eq!(
            BoardListFilter::default().archived,
            ArchivedFilter::LiveOnly
        );
    }

    #[test]
    fn test_board_list_filter_default_sort_is_none() {
        let f = BoardListFilter::default();
        assert_eq!(f.sort, None);
        assert_eq!(f.sort_order, None);
    }

    fn board_named(name: &str, position: i32) -> Board {
        let mut b = Board::new(name, None::<String>);
        b.position = position;
        b
    }

    fn ts(s: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(s)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    fn names(boards: &[Board]) -> Vec<String> {
        boards.iter().map(|b| b.name.clone()).collect()
    }

    #[test]
    fn test_filter_and_sort_boards_by_name_asc() {
        let boards = vec![
            board_named("Charlie", 0),
            board_named("alpha", 1),
            board_named("Bravo", 2),
        ];
        let filter = BoardListFilter {
            sort: Some(BoardSortField::Name),
            sort_order: Some(SortOrder::Ascending),
            ..Default::default()
        };
        let out = filter_and_sort_boards(&boards, &filter, &HashMap::new(), None);
        assert_eq!(names(&out), vec!["alpha", "Bravo", "Charlie"]);
    }

    #[test]
    fn test_filter_and_sort_boards_by_name_desc() {
        let boards = vec![
            board_named("Charlie", 0),
            board_named("alpha", 1),
            board_named("Bravo", 2),
        ];
        let filter = BoardListFilter {
            sort: Some(BoardSortField::Name),
            sort_order: Some(SortOrder::Descending),
            ..Default::default()
        };
        let out = filter_and_sort_boards(&boards, &filter, &HashMap::new(), None);
        assert_eq!(names(&out), vec!["Charlie", "Bravo", "alpha"]);
    }

    #[test]
    fn test_filter_and_sort_boards_by_created_at() {
        let mut older = board_named("Older", 0);
        let mut newer = board_named("Newer", 1);
        older.created_at = ts("2026-01-01T00:00:00Z");
        newer.created_at = ts("2026-06-01T00:00:00Z");
        let boards = vec![newer, older];
        let filter = BoardListFilter {
            sort: Some(BoardSortField::CreatedAt),
            sort_order: Some(SortOrder::Ascending),
            ..Default::default()
        };
        let out = filter_and_sort_boards(&boards, &filter, &HashMap::new(), None);
        assert_eq!(names(&out), vec!["Older", "Newer"]);
    }

    #[test]
    fn test_filter_and_sort_boards_by_archived_at() {
        let older = board_named("Older", 0);
        let newer = board_named("Newer", 1);
        let mut archived_at = HashMap::new();
        archived_at.insert(older.id, ts("2026-01-01T00:00:00Z"));
        archived_at.insert(newer.id, ts("2026-06-01T00:00:00Z"));
        let boards = vec![older, newer];
        let filter = BoardListFilter {
            sort: Some(BoardSortField::ArchivedAt),
            sort_order: Some(SortOrder::Descending),
            ..Default::default()
        };
        let out = filter_and_sort_boards(&boards, &filter, &archived_at, None);
        assert_eq!(names(&out), vec!["Newer", "Older"]);
    }

    #[test]
    fn test_filter_and_sort_boards_by_position() {
        let boards = vec![
            board_named("A", 2),
            board_named("B", 0),
            board_named("C", 1),
        ];
        let filter = BoardListFilter {
            sort: Some(BoardSortField::Position),
            sort_order: Some(SortOrder::Ascending),
            ..Default::default()
        };
        let out = filter_and_sort_boards(&boards, &filter, &HashMap::new(), None);
        assert_eq!(names(&out), vec!["B", "C", "A"]);
    }

    #[test]
    fn test_filter_and_sort_boards_by_position_ties_break_deterministically() {
        let mut older = board_named("Older", 3);
        let mut newer = board_named("Newer", 3);
        older.created_at = ts("2026-01-01T00:00:00Z");
        newer.created_at = ts("2026-06-01T00:00:00Z");
        let boards = vec![newer, older];
        let filter = BoardListFilter {
            sort: Some(BoardSortField::Position),
            sort_order: Some(SortOrder::Ascending),
            ..Default::default()
        };
        let out = filter_and_sort_boards(&boards, &filter, &HashMap::new(), None);
        assert_eq!(names(&out), vec!["Older", "Newer"]);
    }

    #[test]
    fn test_filter_and_sort_boards_uses_default_when_filter_has_no_sort() {
        let boards = vec![
            board_named("A", 2),
            board_named("B", 0),
            board_named("C", 1),
        ];
        let filter = BoardListFilter::default();
        let out = filter_and_sort_boards(
            &boards,
            &filter,
            &HashMap::new(),
            Some((BoardSortField::Position, SortOrder::Ascending)),
        );
        assert_eq!(names(&out), vec!["B", "C", "A"]);
    }

    #[test]
    fn test_resolve_board_sort_override_beats_default() {
        let got = resolve_board_sort(
            Some(BoardSortField::Name),
            Some(SortOrder::Descending),
            Some((BoardSortField::Position, SortOrder::Ascending)),
        );
        assert_eq!(got, Some((BoardSortField::Name, SortOrder::Descending)));
    }

    #[test]
    fn test_resolve_board_sort_falls_back_to_default() {
        let got = resolve_board_sort(
            None,
            None,
            Some((BoardSortField::CreatedAt, SortOrder::Descending)),
        );
        assert_eq!(
            got,
            Some((BoardSortField::CreatedAt, SortOrder::Descending))
        );
    }

    #[test]
    fn test_resolve_board_sort_none_when_neither() {
        assert_eq!(resolve_board_sort(None, None, None), None);
    }

    #[test]
    fn test_resolve_board_sort_field_override_takes_default_order() {
        let got = resolve_board_sort(
            Some(BoardSortField::Name),
            None,
            Some((BoardSortField::Position, SortOrder::Descending)),
        );
        assert_eq!(got, Some((BoardSortField::Name, SortOrder::Descending)));
    }

    #[test]
    fn test_resolve_board_sort_field_override_without_default_is_ascending() {
        let got = resolve_board_sort(Some(BoardSortField::Name), None, None);
        assert_eq!(got, Some((BoardSortField::Name, SortOrder::Ascending)));
    }

    #[test]
    fn test_resolve_board_sort_order_override_layers_on_default_field() {
        let got = resolve_board_sort(
            None,
            Some(SortOrder::Descending),
            Some((BoardSortField::Position, SortOrder::Ascending)),
        );
        assert_eq!(got, Some((BoardSortField::Position, SortOrder::Descending)));
    }
}
