//! Sprint-specific query functions.
//!
//! Provides functions for filtering and partitioning cards by sprint.

use crate::sort::sort_cards_in_place;
use crate::{Card, SortField, SortOrder};
use uuid::Uuid;

/// Get all cards assigned to a sprint.
pub fn get_sprint_cards(sprint_id: Uuid, cards: &[Card]) -> Vec<&Card> {
    cards
        .iter()
        .filter(|card| card.sprint_id == Some(sprint_id))
        .collect()
}

/// Get completed cards assigned to a sprint.
pub fn get_sprint_completed_cards(sprint_id: Uuid, cards: &[Card]) -> Vec<&Card> {
    cards
        .iter()
        .filter(|card| card.sprint_id == Some(sprint_id) && card.is_completed())
        .collect()
}

/// Get uncompleted cards assigned to a sprint.
pub fn get_sprint_uncompleted_cards(sprint_id: Uuid, cards: &[Card]) -> Vec<&Card> {
    cards
        .iter()
        .filter(|card| card.sprint_id == Some(sprint_id) && !card.is_completed())
        .collect()
}

/// Partition sprint cards into completed and uncompleted lists.
///
/// Returns (uncompleted_ids, completed_ids).
pub fn partition_sprint_cards(sprint_id: Uuid, cards: &[Card]) -> (Vec<Uuid>, Vec<Uuid>) {
    let uncompleted_ids: Vec<Uuid> = cards
        .iter()
        .filter(|card| card.sprint_id == Some(sprint_id) && !card.is_completed())
        .map(|card| card.id)
        .collect();

    let completed_ids: Vec<Uuid> = cards
        .iter()
        .filter(|card| card.sprint_id == Some(sprint_id) && card.is_completed())
        .map(|card| card.id)
        .collect();

    (uncompleted_ids, completed_ids)
}

/// Sort card IDs based on the cards they reference.
///
/// Returns a new sorted vector of card IDs.
pub fn sort_card_ids(
    card_ids: &[Uuid],
    cards: &[Card],
    sort_field: SortField,
    sort_order: SortOrder,
) -> Vec<Uuid> {
    let mut card_refs: Vec<&Card> = card_ids
        .iter()
        .filter_map(|id| cards.iter().find(|c| c.id == *id))
        .collect();

    sort_cards_in_place(&mut card_refs, sort_field, sort_order);

    card_refs.iter().map(|c| c.id).collect()
}

/// Calculate total story points from a list of cards.
pub fn calculate_points(cards: &[&Card]) -> u32 {
    cards
        .iter()
        .filter_map(|card| card.points.map(|p| p as u32))
        .sum()
}

/// Calculate total story points from card IDs.
pub fn calculate_points_by_ids(card_ids: &[Uuid], cards: &[Card]) -> u32 {
    card_ids
        .iter()
        .filter_map(|id| cards.iter().find(|c| c.id == *id))
        .filter_map(|card| card.points.map(|p| p as u32))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Board, CardStatus, Column};

    fn create_test_board() -> Board {
        Board::new("Test", None::<String>)
    }

    fn create_test_column(board: &Board) -> Column {
        Column::new(board.id, "Todo", 0)
    }

    fn create_test_card(board: &Board, column: &Column, title: &str) -> Card {
        Card::new(board.id, column.id, title.to_string(), 0)
    }

    #[test]
    fn test_get_sprint_cards() {
        let board = create_test_board();
        let column = create_test_column(&board);
        let sprint_id = Uuid::new_v4();

        let mut card1 = create_test_card(&board, &column, "Task 1");
        card1.sprint_id = Some(sprint_id);

        let mut card2 = create_test_card(&board, &column, "Task 2");
        card2.sprint_id = Some(sprint_id);

        let card3 = create_test_card(&board, &column, "Task 3");

        let cards = vec![card1, card2, card3];
        let result = get_sprint_cards(sprint_id, &cards);

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_get_sprint_completed_cards() {
        let board = create_test_board();
        let column = create_test_column(&board);
        let sprint_id = Uuid::new_v4();

        let mut card1 = create_test_card(&board, &column, "Task 1");
        card1.sprint_id = Some(sprint_id);
        card1.status = CardStatus::Done;

        let mut card2 = create_test_card(&board, &column, "Task 2");
        card2.sprint_id = Some(sprint_id);
        card2.status = CardStatus::Todo;

        let cards = vec![card1, card2];
        let result = get_sprint_completed_cards(sprint_id, &cards);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].status, CardStatus::Done);
    }

    #[test]
    fn test_partition_sprint_cards() {
        let board = create_test_board();
        let column = create_test_column(&board);
        let sprint_id = Uuid::new_v4();

        let mut card1 = create_test_card(&board, &column, "Done");
        card1.sprint_id = Some(sprint_id);
        card1.status = CardStatus::Done;

        let mut card2 = create_test_card(&board, &column, "Todo");
        card2.sprint_id = Some(sprint_id);
        card2.status = CardStatus::Todo;

        let mut card3 = create_test_card(&board, &column, "InProgress");
        card3.sprint_id = Some(sprint_id);
        card3.status = CardStatus::InProgress;

        let cards = vec![card1.clone(), card2.clone(), card3.clone()];
        let (uncompleted, completed) = partition_sprint_cards(sprint_id, &cards);

        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0], card1.id);
        assert_eq!(uncompleted.len(), 2);
        assert!(uncompleted.contains(&card2.id));
        assert!(uncompleted.contains(&card3.id));
    }

    #[test]
    fn test_calculate_points() {
        let board = create_test_board();
        let column = create_test_column(&board);

        let mut card1 = create_test_card(&board, &column, "Task 1");
        card1.points = Some(3);

        let mut card2 = create_test_card(&board, &column, "Task 2");
        card2.points = Some(5);

        let card3 = create_test_card(&board, &column, "Task 3");

        let cards: Vec<&Card> = vec![&card1, &card2, &card3];
        let total = calculate_points(&cards);

        assert_eq!(total, 8);
    }

    #[test]
    fn get_sprint_uncompleted_cards_excludes_done() {
        let board = create_test_board();
        let column = create_test_column(&board);
        let sprint_id = Uuid::new_v4();

        let mut card_done = create_test_card(&board, &column, "Done Task");
        card_done.sprint_id = Some(sprint_id);
        card_done.status = CardStatus::Done;

        let mut card_todo = create_test_card(&board, &column, "Todo Task");
        card_todo.sprint_id = Some(sprint_id);

        let cards = vec![card_done, card_todo.clone()];
        let result = get_sprint_uncompleted_cards(sprint_id, &cards);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, card_todo.id);
    }

    #[test]
    fn get_sprint_uncompleted_cards_includes_all_non_done_statuses() {
        let board = create_test_board();
        let column = create_test_column(&board);
        let sprint_id = Uuid::new_v4();

        let mut card_todo = create_test_card(&board, &column, "Todo");
        card_todo.sprint_id = Some(sprint_id);
        card_todo.status = CardStatus::Todo;

        let mut card_in_progress = create_test_card(&board, &column, "InProgress");
        card_in_progress.sprint_id = Some(sprint_id);
        card_in_progress.status = CardStatus::InProgress;

        let mut card_blocked = create_test_card(&board, &column, "Blocked");
        card_blocked.sprint_id = Some(sprint_id);
        card_blocked.status = CardStatus::Blocked;

        let cards = vec![
            card_todo.clone(),
            card_in_progress.clone(),
            card_blocked.clone(),
        ];
        let result = get_sprint_uncompleted_cards(sprint_id, &cards);

        assert_eq!(result.len(), 3);
        let ids: Vec<_> = result.iter().map(|c| c.id).collect();
        assert!(ids.contains(&card_todo.id));
        assert!(ids.contains(&card_in_progress.id));
        assert!(ids.contains(&card_blocked.id));
    }

    #[test]
    fn get_sprint_uncompleted_cards_excludes_other_sprints() {
        let board = create_test_board();
        let column = create_test_column(&board);
        let sprint_id = Uuid::new_v4();
        let other_sprint_id = Uuid::new_v4();

        let mut card_this_sprint = create_test_card(&board, &column, "This Sprint");
        card_this_sprint.sprint_id = Some(sprint_id);

        let mut card_other_sprint = create_test_card(&board, &column, "Other Sprint");
        card_other_sprint.sprint_id = Some(other_sprint_id);

        let card_no_sprint = create_test_card(&board, &column, "No Sprint");

        let cards = vec![card_this_sprint.clone(), card_other_sprint, card_no_sprint];
        let result = get_sprint_uncompleted_cards(sprint_id, &cards);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, card_this_sprint.id);
    }

    #[test]
    fn get_sprint_uncompleted_cards_returns_empty_when_all_done() {
        let board = create_test_board();
        let column = create_test_column(&board);
        let sprint_id = Uuid::new_v4();

        let mut card1 = create_test_card(&board, &column, "Done 1");
        card1.sprint_id = Some(sprint_id);
        card1.status = CardStatus::Done;

        let mut card2 = create_test_card(&board, &column, "Done 2");
        card2.sprint_id = Some(sprint_id);
        card2.status = CardStatus::Done;

        let cards = vec![card1, card2];
        let result = get_sprint_uncompleted_cards(sprint_id, &cards);

        assert!(result.is_empty());
    }

    #[test]
    fn test_sort_card_ids() {
        let board = create_test_board();
        let column = create_test_column(&board);

        let mut card1 = create_test_card(&board, &column, "Task 1");
        card1.points = Some(5);

        let mut card2 = create_test_card(&board, &column, "Task 2");
        card2.points = Some(1);

        let mut card3 = create_test_card(&board, &column, "Task 3");
        card3.points = Some(3);

        let cards = vec![card1.clone(), card2.clone(), card3.clone()];
        let ids = vec![card1.id, card2.id, card3.id];

        let sorted = sort_card_ids(&ids, &cards, SortField::Points, SortOrder::Ascending);

        assert_eq!(sorted[0], card2.id); // 1 point
        assert_eq!(sorted[1], card3.id); // 3 points
        assert_eq!(sorted[2], card1.id); // 5 points
    }
}
