//! Shared control vocabulary: sort and filter types and functions used by
//! every application on top of `kanban-domain`.
//!
//! Not to be confused with [`kanban_view::Controller`], the per-application
//! render-loop struct that owns view state (mode, selection, focus) for
//! `kanban-tui`; this module is the vocabulary those controllers filter and
//! sort with, not a controller itself.

#[cfg(test)]
mod tests {
    #[test]
    fn test_controller_reexports_match_the_shared_control_vocabulary() {
        let _: fn() -> crate::controller::SortField = || crate::board::SortField::Priority;
        let _: fn() -> crate::controller::BoardSortField =
            || crate::board::BoardSortField::Name;
        let _: fn() -> crate::controller::SortOrder = || crate::board::SortOrder::Ascending;

        let _: fn() -> crate::controller::ArchivedFilter =
            || crate::query::ArchivedFilter::default();
        let _: fn() -> crate::controller::BoardListFilter =
            || crate::query::BoardListFilter::default();
        let _: fn() -> crate::controller::CardListFilter =
            || crate::query::CardListFilter::default();

        let _: fn(
            crate::controller::CardQueryBuilder,
        ) -> crate::controller::CardQueryBuilder = |b| b;

        let _: fn(
            &[crate::Board],
            &crate::controller::BoardListFilter,
            &std::collections::HashMap<uuid::Uuid, chrono::DateTime<chrono::Utc>>,
            Option<(crate::controller::BoardSortField, crate::controller::SortOrder)>,
        ) -> Vec<crate::Board> = crate::controller::filter_and_sort_boards;

        let _: fn(
            &[crate::Card],
            &[crate::Column],
            &[crate::Sprint],
            Option<&crate::Board>,
            &[crate::Board],
            &crate::controller::CardListFilter,
        ) -> Vec<crate::Card> = crate::controller::filter_and_sort_cards::<crate::Card>;
    }
}
