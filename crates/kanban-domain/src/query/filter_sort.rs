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
use crate::sort::{resolve_sort, sort_cards_in_place};
use crate::{Board, Card, CardStatus, Column, SortField, SortOrder, Sprint};
use std::borrow::Borrow;
use std::collections::HashSet;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archived_filter_default_is_live_only() {
        assert_eq!(ArchivedFilter::default(), ArchivedFilter::LiveOnly);
    }

    #[test]
    fn test_card_list_filter_default_archived_is_live_only() {
        assert_eq!(CardListFilter::default().archived, ArchivedFilter::LiveOnly);
    }
}
