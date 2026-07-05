use crate::commands::card::RestoreCard;
use crate::commands::test_helpers::TestContext;
use crate::DataStore;
use chrono::Utc;
use uuid::Uuid;

#[test]
fn test_restore_card_to_deleted_column_returns_error() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("Test", Some("TST"));
    let col = crate::Column::new(board.id, "Col", 0);
    let col_id = col.id;
    let card = crate::Card::new(&mut board, col_id, "Card", 0);
    let card_id = card.id;
    let archived = crate::ArchivedCard::new(card, uuid::Uuid::nil(), col_id, 0);
    tc.store.upsert_board(board).unwrap();
    // Column intentionally NOT added — it has been deleted
    tc.store.insert_archived_card(archived).unwrap();

    let context = tc.as_command_context();
    let cmd = RestoreCard {
        card_id,
        column_id: col_id,
        position: 0,
        timestamp: Utc::now(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_restore_card_to_valid_column_succeeds() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("Test", Some("TST"));
    let col = crate::Column::new(board.id, "Col", 0);
    let col_id = col.id;
    let card = crate::Card::new(&mut board, col_id, "Card", 0);
    let card_id = card.id;
    let archived = crate::ArchivedCard::new(card, uuid::Uuid::nil(), col_id, 0);
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.insert_archived_card(archived).unwrap();

    let context = tc.as_command_context();
    let cmd = RestoreCard {
        card_id,
        column_id: col_id,
        position: 0,
        timestamp: Utc::now(),
    };
    assert!(cmd.execute(&context).is_ok());
    assert_eq!(tc.store.list_all_cards().unwrap().len(), 1);
    assert_eq!(tc.store.list_archived_cards().unwrap().len(), 0);
}

#[test]
fn test_restore_card_exceeding_wip_limit_returns_error() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("Test", Some("TST"));
    let mut col = crate::Column::new(board.id, "Col", 0);
    col.wip_limit = Some(1);
    let col_id = col.id;
    let existing = crate::Card::new(&mut board, col_id, "Existing", 0);
    let card = crate::Card::new(&mut board, col_id, "Card", 1);
    let card_id = card.id;
    let archived = crate::ArchivedCard::new(card, uuid::Uuid::nil(), col_id, 0);
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(existing).unwrap();
    tc.store.insert_archived_card(archived).unwrap();

    let context = tc.as_command_context();
    let cmd = RestoreCard {
        card_id,
        column_id: col_id,
        position: 1,
        timestamp: Utc::now(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_wip_limit_exceeded());
}

#[test]
fn test_restore_card_not_found_returns_error() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = RestoreCard {
        card_id: Uuid::new_v4(),
        column_id: Uuid::new_v4(),
        position: 0,
        timestamp: Utc::now(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_restore_card_uses_embedded_timestamp() {
    use chrono::{TimeZone, Utc};

    let tc = TestContext::new();
    let col = crate::Column::new(Uuid::new_v4(), "Col", 0);
    let column_id = col.id;
    tc.store.upsert_column(col).unwrap();

    let mut board = crate::Board::new("B", Some("TST"));
    let card = crate::Card::new(&mut board, column_id, "Card", 0);
    let card_id = card.id;
    let archived = crate::ArchivedCard::new(card, uuid::Uuid::nil(), column_id, 0);
    tc.store.insert_archived_card(archived).unwrap();

    let fixed_time = Utc.with_ymd_and_hms(2020, 6, 15, 12, 0, 0).unwrap();
    let context = tc.as_command_context();
    let cmd = RestoreCard {
        card_id,
        column_id,
        position: 0,
        timestamp: fixed_time,
    };
    cmd.execute(&context).unwrap();

    let card = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(card.updated_at, fixed_time);
}
