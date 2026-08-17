//! Card sorting functionality.
//!
//! Provides traits and implementations for sorting cards by various fields.
//! Used by both TUI and API for consistent sorting behavior.

pub mod boards;

pub use boards::sort_boards_in_place;

use crate::{Card, CardPriority, CardStatus, SortField, SortOrder};
use std::borrow::Borrow;
use std::cmp::Ordering;

/// Enum dispatch for sorting cards by a specific field.
///
/// All variants are stateless — the sort field is encoded in the
/// enum discriminant.
pub enum SortBy {
    Points,
    Priority,
    CreatedAt,
    UpdatedAt,
    DueDate,
    Status,
    CardNumber,
    Position,
}

impl SortBy {
    pub fn compare(&self, a: &Card, b: &Card) -> Ordering {
        match self {
            Self::Points => match (a.points, b.points) {
                (Some(ap), Some(bp)) => ap.cmp(&bp),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            },
            Self::Priority => priority_value(&a.priority).cmp(&priority_value(&b.priority)),
            Self::CreatedAt => a.created_at.cmp(&b.created_at),
            Self::UpdatedAt => a.updated_at.cmp(&b.updated_at),
            Self::DueDate => match (a.due_date, b.due_date) {
                (Some(ad), Some(bd)) => ad.cmp(&bd),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            },
            Self::Status => status_value(&a.status).cmp(&status_value(&b.status)),
            Self::CardNumber => a.card_number.cmp(&b.card_number),
            Self::Position => a.position.cmp(&b.position),
        }
    }
}

/// Sort a slice in place applying `order` to a `primary` comparator, then
/// breaking ties with a `tiebreak` comparator that stays ascending regardless
/// of `order`.
///
/// This is the shared reverse+tiebreak core behind both the card sorter
/// ([`OrderedSorter::sort_by`], tiebreak = `card_number`) and the board sorter
/// ([`sort_boards_in_place`], tiebreak = `position` then `created_at` then
/// `id`). Keeping the tiebreak direction fixed means toggling the primary
/// direction never reshuffles tied elements.
pub fn sort_by_with_order<T>(
    items: &mut [T],
    order: SortOrder,
    mut primary: impl FnMut(&T, &T) -> Ordering,
    mut tiebreak: impl FnMut(&T, &T) -> Ordering,
) {
    items.sort_by(|a, b| {
        let p = match order {
            SortOrder::Ascending => primary(a, b),
            SortOrder::Descending => primary(a, b).reverse(),
        };
        p.then_with(|| tiebreak(a, b))
    });
}

/// Wrapper that applies sort order (ascending/descending) to a sort field.
pub struct OrderedSorter {
    sorter: SortBy,
    order: SortOrder,
}

impl OrderedSorter {
    pub fn new(sorter: SortBy, order: SortOrder) -> Self {
        Self { sorter, order }
    }

    /// Sort a slice in place. Works with both `&Card` and `Card` elements.
    ///
    /// Ties on the primary key are broken by ascending `card_number` so that
    /// sort output is deterministic regardless of input order. Without this,
    /// backends that yield cards in HashMap iteration order (`InMemoryStore`)
    /// or unordered SQL result sets cause tied cards to jump on every render.
    /// The tiebreaker stays ascending even when `order` is descending so that
    /// toggling sort direction does not reshuffle tied cards.
    ///
    /// Note: `card_number` is unique only within a single board, so this
    /// stabiliser only holds when the slice contains cards from one board —
    /// which is currently always the case via `BoardFilter` in `query/mod.rs`
    /// and the sprint-scoped slices in `query/sprint.rs`. A future cross-board
    /// view would need a different tiebreaker (e.g. `(board_id, card_number)`
    /// or `card.id`).
    pub fn sort_by<T: Borrow<Card>>(&self, cards: &mut [T]) {
        sort_by_with_order(
            cards,
            self.order,
            |a, b| self.sorter.compare(a.borrow(), b.borrow()),
            |a, b| a.borrow().card_number.cmp(&b.borrow().card_number),
        );
    }
}

/// Resolve `(field, order)` from a caller override and an optional board.
/// Override wins; otherwise the board's defaults apply; `None` means
/// "leave storage order".
pub fn resolve_sort(
    sort: Option<SortField>,
    sort_order: Option<SortOrder>,
    board: Option<&crate::Board>,
) -> Option<(SortField, SortOrder)> {
    match (sort, sort_order, board) {
        (Some(f), Some(o), _) => Some((f, o)),
        (Some(f), None, Some(b)) => Some((f, b.task_sort_order)),
        (Some(f), None, None) => Some((f, SortOrder::Ascending)),
        (None, override_order, Some(b)) => Some((
            b.task_sort_field,
            override_order.unwrap_or(b.task_sort_order),
        )),
        (None, _, None) => None,
    }
}

pub fn sort_cards_in_place<T: Borrow<Card>>(cards: &mut [T], field: SortField, order: SortOrder) {
    let sorter = OrderedSorter::new(get_sorter_for_field(field), order);
    sorter.sort_by(cards);
}

/// Get the appropriate sorter for a sort field.
pub fn get_sorter_for_field(field: SortField) -> SortBy {
    match field {
        SortField::Points => SortBy::Points,
        SortField::Priority => SortBy::Priority,
        SortField::CreatedAt => SortBy::CreatedAt,
        SortField::UpdatedAt => SortBy::UpdatedAt,
        SortField::DueDate => SortBy::DueDate,
        SortField::Status => SortBy::Status,
        SortField::Position => SortBy::Position,
        SortField::Default => SortBy::CardNumber,
    }
}

/// Convert priority to numeric value for sorting.
fn priority_value(priority: &CardPriority) -> u8 {
    match priority {
        CardPriority::Critical => 3,
        CardPriority::High => 2,
        CardPriority::Medium => 1,
        CardPriority::Low => 0,
    }
}

/// Convert status to numeric value for sorting.
fn status_value(status: &CardStatus) -> u8 {
    match status {
        CardStatus::Done => 3,
        CardStatus::InProgress => 2,
        CardStatus::Blocked => 1,
        CardStatus::Todo => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Board, Column};

    fn create_test_cards() -> (Board, Column, Card, Card) {
        let board = Board::new("Test", None::<String>);
        let column = Column::new(board.id, "Todo", 0);

        let card1 = Card::new(board.id, column.id, "First", 0);
        let card2 = Card::new(board.id, column.id, "Second", 1);

        (board, column, card1, card2)
    }

    #[test]
    fn test_priority_sorter() {
        let (_, _, mut card1, mut card2) = create_test_cards();

        card1.update_priority(CardPriority::Low);
        card2.update_priority(CardPriority::High);

        assert_eq!(SortBy::Priority.compare(&card1, &card2), Ordering::Less);
        assert_eq!(SortBy::Priority.compare(&card2, &card1), Ordering::Greater);
    }

    #[test]
    fn test_ordered_sorter_ascending() {
        let (_, _, mut card1, mut card2) = create_test_cards();

        card1.update_priority(CardPriority::High);
        card2.update_priority(CardPriority::Low);

        let mut cards = vec![&card1, &card2];
        let sorter = OrderedSorter::new(SortBy::Priority, SortOrder::Ascending);
        sorter.sort_by(&mut cards);

        assert_eq!(cards[0].title, "Second"); // Low priority
        assert_eq!(cards[1].title, "First"); // High priority
    }

    #[test]
    fn test_ordered_sorter_descending() {
        let (_, _, mut card1, mut card2) = create_test_cards();

        card1.update_priority(CardPriority::Low);
        card2.update_priority(CardPriority::High);

        let mut cards = vec![&card1, &card2];
        let sorter = OrderedSorter::new(SortBy::Priority, SortOrder::Descending);
        sorter.sort_by(&mut cards);

        assert_eq!(cards[0].title, "Second"); // High priority
        assert_eq!(cards[1].title, "First"); // Low priority
    }

    #[test]
    fn test_position_sorter() {
        let board = Board::new("Test", None::<String>);
        let column = Column::new(board.id, "Todo", 0);

        let card1 = Card::new(board.id, column.id, "Third", 20);
        let card2 = Card::new(board.id, column.id, "First", 5);
        let card3 = Card::new(board.id, column.id, "Second", 10);

        assert_eq!(SortBy::Position.compare(&card2, &card3), Ordering::Less); // 5 < 10
        assert_eq!(SortBy::Position.compare(&card3, &card1), Ordering::Less); // 10 < 20
    }

    #[test]
    fn test_get_sorter_for_field() {
        let sorter = get_sorter_for_field(SortField::Position);

        let board = Board::new("Test", None::<String>);
        let column = Column::new(board.id, "Todo", 0);

        let card1 = Card::new(board.id, column.id, "A", 10);
        let card2 = Card::new(board.id, column.id, "B", 5);

        assert_eq!(sorter.compare(&card2, &card1), Ordering::Less);
    }

    #[test]
    fn test_due_date_sorter_orders_earlier_first() {
        let (_, _, mut card1, mut card2) = create_test_cards();

        let earlier = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let later = chrono::DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);

        card1.set_due_date(Some(earlier));
        card2.set_due_date(Some(later));

        assert_eq!(SortBy::DueDate.compare(&card1, &card2), Ordering::Less);
        assert_eq!(SortBy::DueDate.compare(&card2, &card1), Ordering::Greater);
    }

    #[test]
    fn test_due_date_sorter_places_none_last() {
        let (_, _, mut card1, mut card2) = create_test_cards();

        let some_date = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        card1.set_due_date(Some(some_date));
        card2.set_due_date(None);

        assert_eq!(SortBy::DueDate.compare(&card1, &card2), Ordering::Less);
        assert_eq!(SortBy::DueDate.compare(&card2, &card1), Ordering::Greater);

        card1.set_due_date(None);
        assert_eq!(SortBy::DueDate.compare(&card1, &card2), Ordering::Equal);
    }

    #[test]
    fn test_due_date_sorter_equal_dates_returns_equal() {
        let (_, _, mut card1, mut card2) = create_test_cards();

        let d = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        card1.set_due_date(Some(d));
        card2.set_due_date(Some(d));

        assert_eq!(SortBy::DueDate.compare(&card1, &card2), Ordering::Equal);
    }

    #[test]
    fn test_get_sorter_for_field_due_date_maps_to_due_date_sortby() {
        let sorter = get_sorter_for_field(SortField::DueDate);

        let (_, _, mut card1, mut card2) = create_test_cards();
        let earlier = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let later = chrono::DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        card1.set_due_date(Some(earlier));
        card2.set_due_date(Some(later));

        assert_eq!(sorter.compare(&card1, &card2), Ordering::Less);
    }

    #[test]
    fn test_points_sorter_none_handling() {
        let (_, _, mut card1, mut card2) = create_test_cards();

        card1.points = Some(3);
        card2.points = None;

        assert_eq!(SortBy::Points.compare(&card1, &card2), Ordering::Less);
        assert_eq!(SortBy::Points.compare(&card2, &card1), Ordering::Greater);

        card1.points = None;
        assert_eq!(SortBy::Points.compare(&card1, &card2), Ordering::Equal);
    }

    /// Cards with equal primary sort keys (e.g. all `points = None`) must
    /// always end up in the same final order regardless of how the slice
    /// arrived at the sort. Without a deterministic tiebreaker, backends
    /// that yield cards in HashMap iteration order (`InMemoryStore::snapshot`)
    /// or unordered SQL result sets cause tied cards to visibly jump on every
    /// re-render.
    #[test]
    fn test_ordered_sorter_is_deterministic_when_primary_keys_tie() {
        let board = Board::new("Test", None::<String>);
        let column = Column::new(board.id, "Todo", 0);

        // All three cards have points=None, so SortBy::Points reports them equal.
        let mut card1 = Card::new(board.id, column.id, "A", 0);
        let mut card2 = Card::new(board.id, column.id, "B", 1);
        let mut card3 = Card::new(board.id, column.id, "C", 2);
        // Set explicitly: `Card::new` does not number cards, and comparing
        // card numbers read back off these same cards would hold for any
        // ordering the sorter produced -- including none at all.
        card1.card_number = 1;
        card2.card_number = 2;
        card3.card_number = 3;

        let sorter = OrderedSorter::new(SortBy::Points, SortOrder::Ascending);

        let mut shuffled_a = vec![&card1, &card2, &card3];
        sorter.sort_by(&mut shuffled_a);
        let mut shuffled_b = vec![&card3, &card1, &card2];
        sorter.sort_by(&mut shuffled_b);
        let mut shuffled_c = vec![&card2, &card3, &card1];
        sorter.sort_by(&mut shuffled_c);

        let order = |cs: &[&Card]| (cs[0].card_number, cs[1].card_number, cs[2].card_number);
        let expected = (1, 2, 3);

        assert_eq!(
            order(&shuffled_a),
            expected,
            "tied cards must order by card_number regardless of input order"
        );
        assert_eq!(order(&shuffled_b), expected);
        assert_eq!(order(&shuffled_c), expected);
    }

    /// Tiebreaker must remain ascending even when the primary sort is
    /// descending — flipping the tiebreaker too would make tied cards swap
    /// when the user toggles direction, which is just as disorienting as
    /// the original instability.
    #[test]
    fn test_ordered_sorter_tiebreaker_is_ascending_under_descending_primary() {
        let board = Board::new("Test", None::<String>);
        let column = Column::new(board.id, "Todo", 0);

        let mut card1 = Card::new(board.id, column.id, "A", 0);
        let mut card2 = Card::new(board.id, column.id, "B", 1);
        card1.card_number = 1;
        card2.card_number = 2;
        let expected = (1, 2);

        let sorter = OrderedSorter::new(SortBy::Points, SortOrder::Descending);
        let mut shuffled = vec![&card2, &card1];
        sorter.sort_by(&mut shuffled);
        assert_eq!((shuffled[0].card_number, shuffled[1].card_number), expected);
    }

    fn board_with_sort(field: SortField, order: SortOrder) -> Board {
        let mut b = Board::new("Test", None::<String>);
        b.update_task_sort(field, order);
        b
    }

    #[test]
    fn test_resolve_sort_explicit_override_wins_over_board() {
        let board = board_with_sort(SortField::Priority, SortOrder::Ascending);
        let got = resolve_sort(
            Some(SortField::DueDate),
            Some(SortOrder::Descending),
            Some(&board),
        );
        assert_eq!(got, Some((SortField::DueDate, SortOrder::Descending)));
    }

    #[test]
    fn test_resolve_sort_field_override_takes_board_order_when_no_order_given() {
        let board = board_with_sort(SortField::Priority, SortOrder::Descending);
        let got = resolve_sort(Some(SortField::DueDate), None, Some(&board));
        assert_eq!(got, Some((SortField::DueDate, SortOrder::Descending)));
    }

    #[test]
    fn test_resolve_sort_field_override_without_board_defaults_to_ascending() {
        let got = resolve_sort(Some(SortField::DueDate), None, None);
        assert_eq!(got, Some((SortField::DueDate, SortOrder::Ascending)));
    }

    #[test]
    fn test_resolve_sort_no_field_falls_back_to_board_defaults() {
        let board = board_with_sort(SortField::Status, SortOrder::Descending);
        let got = resolve_sort(None, None, Some(&board));
        assert_eq!(got, Some((SortField::Status, SortOrder::Descending)));
    }

    #[test]
    fn test_resolve_sort_order_override_layers_on_board_field() {
        let board = board_with_sort(SortField::Status, SortOrder::Ascending);
        let got = resolve_sort(None, Some(SortOrder::Descending), Some(&board));
        assert_eq!(got, Some((SortField::Status, SortOrder::Descending)));
    }

    #[test]
    fn test_resolve_sort_returns_none_when_no_override_and_no_board() {
        assert_eq!(resolve_sort(None, None, None), None);
        assert_eq!(resolve_sort(None, Some(SortOrder::Descending), None), None);
    }

    #[test]
    fn test_sort_cards_in_place_uses_field_and_order() {
        let (_, _, mut card1, mut card2) = create_test_cards();
        let earlier = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let later = chrono::DateTime::parse_from_rfc3339("2026-06-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        card1.set_due_date(Some(later));
        card2.set_due_date(Some(earlier));

        let mut cards = vec![&card1, &card2];
        sort_cards_in_place(&mut cards, SortField::DueDate, SortOrder::Ascending);
        assert_eq!(cards[0].due_date, Some(earlier));
        assert_eq!(cards[1].due_date, Some(later));
    }

    /// The generic reverse+tiebreak core must apply `order` to the primary
    /// comparator (reversing it under `Descending`) while keeping the tiebreak
    /// comparator ascending regardless of `order`. Toggling direction flips the
    /// primary ranking but leaves tied elements in the same (ascending) order.
    #[test]
    fn test_sort_by_with_order_reverses_primary_but_keeps_tiebreak_ascending() {
        // Elements: (primary_key, tiebreak_key). Two share primary=0 to exercise
        // the tiebreak; one has primary=1 to exercise the reversal.
        let primary = |a: &(i32, i32), b: &(i32, i32)| a.0.cmp(&b.0);
        let tiebreak = |a: &(i32, i32), b: &(i32, i32)| a.1.cmp(&b.1);

        // Ascending: primary ascends (0s before 1), ties break ascending (10, 20).
        let mut asc = vec![(1, 5), (0, 20), (0, 10)];
        sort_by_with_order(&mut asc, SortOrder::Ascending, primary, tiebreak);
        assert_eq!(asc, vec![(0, 10), (0, 20), (1, 5)]);

        // Descending: primary reverses (1 before 0s), ties STAY ascending (10, 20).
        let mut desc = vec![(0, 20), (1, 5), (0, 10)];
        sort_by_with_order(&mut desc, SortOrder::Descending, primary, tiebreak);
        assert_eq!(desc, vec![(1, 5), (0, 10), (0, 20)]);
    }

    /// `CardNumber` and `Position` are excluded because their primaries
    /// are themselves unique per slice — there's nothing to tiebreak.
    #[test]
    fn test_ordered_sorter_tiebreaker_applies_to_every_sort_field_with_ties() {
        let variants = [
            SortBy::Points,
            SortBy::Priority,
            SortBy::Status,
            SortBy::CreatedAt,
            SortBy::UpdatedAt,
            SortBy::DueDate,
        ];

        for variant in variants {
            let board = Board::new("Test", None::<String>);
            let column = Column::new(board.id, "Todo", 0);
            let mut card1 = Card::new(board.id, column.id, "A", 0);
            let mut card2 = Card::new(board.id, column.id, "B", 1);
            let mut card3 = Card::new(board.id, column.id, "C", 2);
            card1.card_number = 1;
            card2.card_number = 2;
            card3.card_number = 3;
            let expected = (1, 2, 3);

            let sorter = OrderedSorter::new(variant, SortOrder::Ascending);
            let mut shuffled = vec![&card3, &card1, &card2];
            sorter.sort_by(&mut shuffled);

            assert_eq!(
                (
                    shuffled[0].card_number,
                    shuffled[1].card_number,
                    shuffled[2].card_number,
                ),
                expected,
                "tiebreaker must order tied cards by card_number for every sort variant"
            );
        }
    }
}
