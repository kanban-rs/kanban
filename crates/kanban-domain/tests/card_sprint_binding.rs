use chrono::Utc;
mod common;
use common::TestContext;
use uuid::Uuid;

use kanban_domain::commands::card::*;
use kanban_domain::*;

#[test]
fn test_assign_cards_to_sprint_validates_sprint_exists() {
    let tc = TestContext::new();
    let mut board = kanban_domain::Board::new("Test", Some("TST"));
    let card = kanban_domain::Card::new(&mut board, Uuid::new_v4(), "Card", 0);
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
    let mut board = kanban_domain::Board::new("Test", Some("TST"));
    let card = kanban_domain::Card::new(&mut board, Uuid::new_v4(), "Card", 0);
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
    let mut board = kanban_domain::Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let mut card = kanban_domain::Card::new(&mut board, col.id, "Card", 0);
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
