use chrono::Utc;
mod common;
use common::TestContext;
use uuid::Uuid;

use kanban_domain::commands::card::*;
use kanban_domain::*;

#[test]
fn test_assign_cards_to_sprint_validates_sprint_exists() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let card = kanban_domain::Card::new(board.id, Uuid::new_v4(), "Card", 0);
    let card_id = card.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    let cmd = AssignCardsToSprint {
        ids: vec![card_id],
        sprint_id: Uuid::new_v4(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_assign_cards_to_sprint_invalid_ids_skipped_valid_ids_assigned() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let card = kanban_domain::Card::new(board.id, Uuid::new_v4(), "Card", 0);
    let valid_id = card.id;
    let sprint = kanban_domain::Sprint::new(board.id, 1, None, Some("Sprint"));
    let sprint_id = sprint.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store.upsert_sprint(sprint).unwrap();

    let context = tc.as_command_context();
    let cmd = AssignCardsToSprint {
        ids: vec![valid_id, Uuid::new_v4()],
        sprint_id,
    };
    let result = cmd.execute(&context);
    assert!(result.is_ok());
    let card = tc.store.get_card(valid_id).unwrap().unwrap();
    assert_eq!(card.sprint_id, Some(sprint_id));
}

#[test]
fn test_assign_cards_to_sprint_on_another_board_returns_sprint_board_mismatch() {
    let tc = TestContext::new();
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let col_a = kanban_domain::Column::new(board_a.id, "Col", 0);
    let card = kanban_domain::Card::new(board_a.id, col_a.id, "Card", 0);
    let card_id = card.id;
    let sprint_b = kanban_domain::Sprint::new(board_b.id, 1, None, Some("Sprint"));
    let sprint_id = sprint_b.id;
    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(col_a).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store.upsert_sprint(sprint_b).unwrap();

    let context = tc.as_command_context();
    let cmd = AssignCardsToSprint {
        ids: vec![card_id],
        sprint_id,
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_sprint_board_mismatch());

    let card = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(card.sprint_id, None);
    assert!(card.sprint_logs.is_empty());
}

#[test]
fn test_assign_cards_to_sprint_with_one_foreign_card_writes_no_card() {
    let tc = TestContext::new();
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let col_a = kanban_domain::Column::new(board_a.id, "Col", 0);
    let same_board_card = kanban_domain::Card::new(board_a.id, col_a.id, "Same", 0);
    let foreign_card = kanban_domain::Card::new(board_b.id, Uuid::new_v4(), "Foreign", 0);
    let same_board_card_id = same_board_card.id;
    let foreign_card_id = foreign_card.id;
    let sprint_a = kanban_domain::Sprint::new(board_a.id, 1, None, Some("Sprint"));
    let sprint_id = sprint_a.id;
    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(col_a).unwrap();
    tc.store.upsert_card(same_board_card).unwrap();
    tc.store.upsert_card(foreign_card).unwrap();
    tc.store.upsert_sprint(sprint_a).unwrap();

    let context = tc.as_command_context();
    let cmd = AssignCardsToSprint {
        ids: vec![same_board_card_id, foreign_card_id],
        sprint_id,
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_sprint_board_mismatch());

    let same_board_card = tc.store.get_card(same_board_card_id).unwrap().unwrap();
    assert_eq!(same_board_card.sprint_id, None);
}

#[test]
fn test_assign_cards_to_sprint_already_on_a_cross_board_sprint_is_a_no_op() {
    let tc = TestContext::new();
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let col_a = kanban_domain::Column::new(board_a.id, "Col", 0);
    let mut card = kanban_domain::Card::new(board_a.id, col_a.id, "Card", 0);
    let sprint_b = kanban_domain::Sprint::new(board_b.id, 1, None, Some("Sprint"));
    let sprint_id = sprint_b.id;
    card.assign_to_sprint(
        sprint_id,
        sprint_b.sprint_number,
        None::<String>,
        "Planning",
        Utc::now(),
    );
    let card_id = card.id;
    let logs_before = card.sprint_logs.len();
    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(col_a).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store.upsert_sprint(sprint_b).unwrap();

    let context = tc.as_command_context();
    let cmd = AssignCardsToSprint {
        ids: vec![card_id],
        sprint_id,
    };
    let result = cmd.execute(&context);
    assert!(result.is_ok());

    let card = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(card.sprint_logs.len(), logs_before);
}

#[test]
fn test_unassign_card_from_sprint_not_found_returns_error() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = UnassignCardFromSprint {
        card_id: Uuid::new_v4(),
        timestamp: Utc::now(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_unassign_card_from_sprint_uses_embedded_timestamp() {
    use chrono::{TimeZone, Utc};

    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let mut card = kanban_domain::Card::new(board.id, col.id, "Card", 0);
    let card_id = card.id;
    card.sprint_id = Some(Uuid::new_v4());
    tc.store.upsert_card(card).unwrap();

    let fixed_time = Utc.with_ymd_and_hms(2020, 3, 10, 8, 0, 0).unwrap();
    let context = tc.as_command_context();
    let cmd = UnassignCardFromSprint {
        card_id,
        timestamp: fixed_time,
    };
    cmd.execute(&context).unwrap();

    let card = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(card.updated_at, fixed_time);
}
