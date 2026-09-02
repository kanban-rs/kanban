//! Shared control vocabulary: sort and filter types and functions used by
//! every application on top of `kanban-domain`.
//!
//! Not to be confused with `kanban_view::Controller`, one application's
//! presentation state derived from a `Model` plus session-scoped choices
//! (cached live/archived partitions of that application's cards and boards,
//! and its board sort field/order). It does not own `mode`, `selection` or
//! `focus`; those stay on `kanban-tui`'s `App`. This module is the
//! vocabulary a `kanban_view::Controller` filters and sorts with, not a
//! controller itself.
//!
//! The sort enums (`SortField`, `BoardSortField`, `SortOrder`) are
//! re-exported from `crate::board` rather than `crate::query` because they
//! live with `Board` for historical reasons.
//!
//! [`count_filtered_cards`](crate::query::count_filtered_cards) and
//! [`resolve_board_sort`](crate::query::resolve_board_sort) are part of the
//! same vocabulary and remain reachable as `kanban_domain::count_filtered_cards`
//! / `kanban_domain::resolve_board_sort` (or via `crate::query`); they are
//! omitted from this module's re-export list to keep the named surface to
//! the primary filter/sort entry points.

pub use crate::board::{BoardSortField, SortField, SortOrder};
pub use crate::query::{
    filter_and_sort_boards, filter_and_sort_cards, ArchivedFilter, BoardListFilter, CardListFilter,
    CardQueryBuilder,
};

#[cfg(test)]
mod tests {
    type BoardArchivedAt = std::collections::HashMap<uuid::Uuid, chrono::DateTime<chrono::Utc>>;
    type BoardSortOverride = Option<(super::BoardSortField, super::SortOrder)>;

    fn accepts_board_sort_vocabulary(
        _: super::SortField,
        _: super::BoardSortField,
        _: super::SortOrder,
    ) {
    }

    fn accepts_board_filter_vocabulary(
        _: super::ArchivedFilter,
        _: super::BoardListFilter,
        _: super::CardListFilter,
    ) {
    }

    fn identity_filter_and_sort_boards(
        boards: &[crate::Board],
        filter: &super::BoardListFilter,
        archived_at: &BoardArchivedAt,
        default: BoardSortOverride,
    ) -> Vec<crate::Board> {
        super::filter_and_sort_boards(boards, filter, archived_at, default)
    }

    fn identity_filter_and_sort_cards(
        cards: &[crate::Card],
        columns: &[crate::Column],
        sprints: &[crate::Sprint],
        board: Option<&crate::Board>,
        boards: &[crate::Board],
        filter: &super::CardListFilter,
    ) -> Vec<crate::Card> {
        super::filter_and_sort_cards(cards, columns, sprints, board, boards, filter)
    }

    fn identity_card_query_builder(
        builder: super::CardQueryBuilder<'_>,
    ) -> super::CardQueryBuilder<'_> {
        builder
    }

    #[test]
    fn test_controller_reexports_match_the_shared_control_vocabulary() {
        accepts_board_sort_vocabulary(
            crate::board::SortField::Priority,
            crate::board::BoardSortField::Name,
            crate::board::SortOrder::Ascending,
        );
        accepts_board_filter_vocabulary(
            crate::query::ArchivedFilter::default(),
            crate::query::BoardListFilter::default(),
            crate::query::CardListFilter::default(),
        );

        let _ = identity_filter_and_sort_boards;
        let _ = identity_filter_and_sort_cards;
        let _ = identity_card_query_builder;
    }
}
