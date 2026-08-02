//! Card query and filtering functionality.
//!
//! - [`filter_sort`] holds the request shape ([`CardListFilter`]) and the in-memory filter+sort engine
//!   ([`filter_and_sort_cards`], [`count_filtered_cards`]).
//! - [`CardQueryBuilder`] is the fluent typed wrapper the TUI uses over
//!   its model snapshot.
//! - [`sprint`] holds sprint-specific helpers (`get_sprint_cards`,
//!   `partition_sprint_cards`, `sort_card_ids`, points calculations).

pub mod filter_sort;
pub mod sprint;

pub use filter_sort::{
    count_filtered_cards, filter_and_sort_boards, filter_and_sort_cards, resolve_board_sort,
    ArchivedFilter, BoardListFilter, CardListFilter,
};

use crate::{Board, Card, Column, Sprint};
use std::collections::HashSet;
use uuid::Uuid;

/// Builder for constructing card queries with fluent API.
pub struct CardQueryBuilder<'a> {
    cards: &'a [Card],
    columns: &'a [Column],
    sprints: &'a [Sprint],
    board: &'a Board,
    column_id: Option<Uuid>,
    sprint_filter: Option<HashSet<Uuid>>,
    hide_assigned: bool,
    search_query: Option<String>,
}

impl<'a> CardQueryBuilder<'a> {
    /// Create a new query builder.
    pub fn new(
        cards: &'a [Card],
        columns: &'a [Column],
        sprints: &'a [Sprint],
        board: &'a Board,
    ) -> Self {
        Self {
            cards,
            columns,
            sprints,
            board,
            column_id: None,
            sprint_filter: None,
            hide_assigned: false,
            search_query: None,
        }
    }

    /// Filter to a specific column.
    pub fn in_column(mut self, column_id: Uuid) -> Self {
        self.column_id = Some(column_id);
        self
    }

    /// Filter to specific sprints.
    pub fn in_sprints(mut self, sprint_ids: impl IntoIterator<Item = Uuid>) -> Self {
        self.sprint_filter = Some(sprint_ids.into_iter().collect());
        self
    }

    /// Hide cards that are assigned to any sprint.
    pub fn hide_assigned(mut self) -> Self {
        self.hide_assigned = true;
        self
    }

    /// Filter by search query.
    pub fn search(mut self, query: impl Into<String>) -> Self {
        let q = query.into();
        if !q.is_empty() {
            self.search_query = Some(q);
        }
        self
    }

    /// Execute the query and return matching card IDs.
    ///
    /// Thin shim over `filter_and_sort_cards` — the actual filter/sort
    /// logic lives in the domain helper so the service, CLI and MCP
    /// share it. This builder is the TUI's typed API into that helper.
    pub fn execute(self) -> Vec<Uuid> {
        let filter = CardListFilter {
            board_id: Some(self.board.id),
            column_id: self.column_id,
            sprint_ids: self.sprint_filter,
            hide_assigned: self.hide_assigned,
            search: self.search_query,
            ..Default::default()
        };
        filter_and_sort_cards(
            self.cards,
            self.columns,
            self.sprints,
            Some(self.board),
            &filter,
        )
        .into_iter()
        .map(|c| c.id)
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SortField, SortOrder};

    fn create_test_board() -> Board {
        let mut board = Board::new("Test", None::<String>);
        board.update_task_sort(SortField::Default, SortOrder::Ascending);
        board
    }

    fn create_test_column(board: &Board, name: &str, position: i32) -> Column {
        Column::new(board.id, name.to_string(), position)
    }

    fn create_test_card(board: &mut Board, column: &Column, title: &str, position: i32) -> Card {
        Card::new(board, column.id, title.to_string(), position)
    }

    #[test]
    fn test_filter_and_sort_cards_basic() {
        let mut board = create_test_board();
        let column = create_test_column(&board, "Todo", 0);
        let card1 = create_test_card(&mut board, &column, "Task 1", 0);
        let card2 = create_test_card(&mut board, &column, "Task 2", 1);

        let columns = vec![column.clone()];
        let cards = vec![card1.clone(), card2.clone()];

        let result = CardQueryBuilder::new(&cards, &columns, &[], &board).execute();

        assert_eq!(result.len(), 2);
        assert!(result.contains(&card1.id));
        assert!(result.contains(&card2.id));
    }

    #[test]
    fn test_filter_by_column() {
        let mut board = create_test_board();
        let column1 = create_test_column(&board, "Todo", 0);
        let column2 = create_test_column(&board, "Done", 1);
        let card1 = create_test_card(&mut board, &column1, "Task 1", 0);
        let card2 = create_test_card(&mut board, &column2, "Task 2", 0);

        let columns = vec![column1.clone(), column2.clone()];
        let cards = vec![card1.clone(), card2.clone()];

        let result = CardQueryBuilder::new(&cards, &columns, &[], &board)
            .in_column(column1.id)
            .execute();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], card1.id);
    }

    #[test]
    fn test_filter_by_search() {
        let mut board = create_test_board();
        let column = create_test_column(&board, "Todo", 0);
        let card1 = create_test_card(&mut board, &column, "Fix bug", 0);
        let card2 = create_test_card(&mut board, &column, "Add feature", 1);

        let columns = vec![column.clone()];
        let cards = vec![card1.clone(), card2.clone()];

        let result = CardQueryBuilder::new(&cards, &columns, &[], &board)
            .search("bug")
            .execute();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], card1.id);
    }

    #[test]
    fn test_hide_assigned_cards() {
        let mut board = create_test_board();
        let column = create_test_column(&board, "Todo", 0);
        let mut card1 = create_test_card(&mut board, &column, "Assigned", 0);
        card1.sprint_id = Some(Uuid::new_v4());
        let card2 = create_test_card(&mut board, &column, "Unassigned", 1);

        let columns = vec![column.clone()];
        let cards = vec![card1.clone(), card2.clone()];

        let result = CardQueryBuilder::new(&cards, &columns, &[], &board)
            .hide_assigned()
            .execute();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], card2.id);
    }

    #[test]
    fn test_query_builder() {
        let mut board = create_test_board();
        let column = create_test_column(&board, "Todo", 0);
        let card1 = create_test_card(&mut board, &column, "Fix bug", 0);
        let card2 = create_test_card(&mut board, &column, "Add feature", 1);

        let columns = vec![column.clone()];
        let cards = vec![card1.clone(), card2.clone()];

        let result = CardQueryBuilder::new(&cards, &columns, &[], &board)
            .search("bug")
            .execute();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], card1.id);
    }

    #[test]
    fn test_query_builder_with_column() {
        let mut board = create_test_board();
        let column1 = create_test_column(&board, "Todo", 0);
        let column2 = create_test_column(&board, "Done", 1);
        let card1 = create_test_card(&mut board, &column1, "Task 1", 0);
        let card2 = create_test_card(&mut board, &column2, "Task 2", 0);

        let columns = vec![column1.clone(), column2.clone()];
        let cards = vec![card1.clone(), card2.clone()];

        let result = CardQueryBuilder::new(&cards, &columns, &[], &board)
            .in_column(column1.id)
            .execute();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], card1.id);
    }
}
