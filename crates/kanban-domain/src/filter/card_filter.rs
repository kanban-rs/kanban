//! Card filtering implementations.
//!
//! Provides the CardFilter trait and various filter implementations for
//! filtering cards by board, column, sprint, and other criteria.

use crate::{Card, Column};
use std::collections::HashSet;
use uuid::Uuid;

/// Trait for filtering cards by various criteria.
pub trait CardFilter {
    /// Returns true if the card matches the filter criteria.
    fn matches(&self, card: &Card) -> bool;
}

/// Filter cards by board membership.
///
/// A card belongs to a board if its column is in that board.
pub struct BoardFilter<'a> {
    board_id: Uuid,
    columns: &'a [Column],
}

impl<'a> BoardFilter<'a> {
    /// Create a new board filter.
    pub fn new(board_id: Uuid, columns: &'a [Column]) -> Self {
        Self { board_id, columns }
    }
}

impl CardFilter for BoardFilter<'_> {
    fn matches(&self, card: &Card) -> bool {
        self.columns
            .iter()
            .any(|col| col.id == card.column_id && col.board_id == self.board_id)
    }
}

/// Filter cards by column.
pub struct ColumnFilter {
    column_id: Uuid,
}

impl ColumnFilter {
    /// Create a new column filter.
    pub fn new(column_id: Uuid) -> Self {
        Self { column_id }
    }
}

impl CardFilter for ColumnFilter {
    fn matches(&self, card: &Card) -> bool {
        card.column_id == self.column_id
    }
}

/// Filter cards by sprint membership.
///
/// Matches cards that are assigned to any of the specified sprints.
pub struct SprintFilter {
    sprint_ids: HashSet<Uuid>,
}

impl SprintFilter {
    /// Create a filter for cards in specific sprints.
    pub fn in_sprints(ids: impl IntoIterator<Item = Uuid>) -> Self {
        Self {
            sprint_ids: ids.into_iter().collect(),
        }
    }

    /// Create a filter for cards in a single sprint.
    pub fn in_sprint(id: Uuid) -> Self {
        Self::in_sprints(std::iter::once(id))
    }
}

impl CardFilter for SprintFilter {
    fn matches(&self, card: &Card) -> bool {
        card.sprint_id
            .is_some_and(|id| self.sprint_ids.contains(&id))
    }
}

/// Filter for cards not assigned to any sprint.
pub struct UnassignedOnlyFilter;

impl CardFilter for UnassignedOnlyFilter {
    fn matches(&self, card: &Card) -> bool {
        card.sprint_id.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Board;

    fn create_test_card(board: &Board, column_id: Uuid) -> Card {
        Card::new(board.id, column_id, "Test Card", 0)
    }

    #[test]
    fn test_board_filter() {
        let board = Board::new("Test Board", None::<String>);
        let column = Column::new(board.id, "Todo", 0);
        let other_column = Column::new(Uuid::new_v4(), "Other", 0); // Different board

        let board_mut = board.clone();
        let card = create_test_card(&board_mut, column.id);

        let columns = vec![column.clone(), other_column];

        let filter = BoardFilter::new(board.id, &columns);
        assert!(filter.matches(&card));

        // Card in a column not belonging to the board
        let board_mut2 = board.clone();
        let other_card = create_test_card(&board_mut2, Uuid::new_v4());
        assert!(!filter.matches(&other_card));
    }

    #[test]
    fn test_column_filter() {
        let board = Board::new("Test Board", None::<String>);
        let column1 = Column::new(board.id, "Todo", 0);
        let column2 = Column::new(board.id, "Done", 1);

        let board_mut = board.clone();
        let card1 = create_test_card(&board_mut, column1.id);
        let card2 = create_test_card(&board_mut, column2.id);

        let filter = ColumnFilter::new(column1.id);
        assert!(filter.matches(&card1));
        assert!(!filter.matches(&card2));
    }

    #[test]
    fn test_sprint_filter() {
        let board = Board::new("Test Board", None::<String>);
        let column = Column::new(board.id, "Todo", 0);

        let board_mut = board.clone();
        let mut card = create_test_card(&board_mut, column.id);

        let sprint_id = Uuid::new_v4();
        card.sprint_id = Some(sprint_id);

        let filter = SprintFilter::in_sprint(sprint_id);
        assert!(filter.matches(&card));

        let other_sprint = Uuid::new_v4();
        let filter2 = SprintFilter::in_sprint(other_sprint);
        assert!(!filter2.matches(&card));

        // Multiple sprints
        let filter3 = SprintFilter::in_sprints(vec![sprint_id, other_sprint]);
        assert!(filter3.matches(&card));
    }

    #[test]
    fn test_unassigned_only_filter() {
        let board = Board::new("Test Board", None::<String>);
        let column = Column::new(board.id, "Todo", 0);

        let board_mut = board.clone();
        let mut assigned_card = create_test_card(&board_mut, column.id);
        assigned_card.sprint_id = Some(Uuid::new_v4());

        let unassigned_card = create_test_card(&board_mut, column.id);

        let filter = UnassignedOnlyFilter;
        assert!(!filter.matches(&assigned_card));
        assert!(filter.matches(&unassigned_card));
    }
}
