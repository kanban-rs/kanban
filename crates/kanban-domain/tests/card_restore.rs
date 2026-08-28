mod common;
use common::TestContext;

use kanban_domain::commands::card::RestoreCard;

use chrono::Utc;
use kanban_domain::DataStore;
use uuid::Uuid;

#[test]
fn test_restore_card_to_deleted_column_returns_error() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let col_id = col.id;
    let card = kanban_domain::Card::new(board.id, col_id, "Card", 0);
    let card_id = card.id;
    let board_id = board.id;
    tc.store.upsert_board(board).unwrap();
    // Column intentionally NOT added — it has been deleted
    tc.store.upsert_card(card).unwrap();
    tc.store
        .insert_archived_card(kanban_domain::ArchivedCard::new(card_id, board_id))
        .unwrap();

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
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let col_id = col.id;
    let card = kanban_domain::Card::new(board.id, col_id, "Card", 0);
    let card_id = card.id;
    let board_id = board.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store
        .insert_archived_card(kanban_domain::ArchivedCard::new(card_id, board_id))
        .unwrap();

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
fn test_restore_card_preserves_board_id() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let board_id = board.id;
    let col = kanban_domain::Column::new(board_id, "Col", 0);
    let col_id = col.id;
    let card = kanban_domain::Card::new(board.id, col_id, "Card", 0);
    let card_id = card.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store
        .insert_archived_card(kanban_domain::ArchivedCard::new(card_id, board_id))
        .unwrap();

    let context = tc.as_command_context();
    let cmd = RestoreCard {
        card_id,
        column_id: col_id,
        position: 0,
        timestamp: Utc::now(),
    };
    cmd.execute(&context).unwrap();

    let restored = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(
        restored.board_id, board_id,
        "restoring to the same board's column leaves board_id unchanged"
    );
}

#[test]
fn test_restore_card_to_column_on_different_board_updates_board_id() {
    // Not the normal restore flow (capture_inverse always targets the card's
    // own current column, never a different board's), but RestoreCard's
    // column_id isn't otherwise validated to belong to the card's original
    // board, so board_id must stay in sync with wherever it actually lands,
    // mirroring MoveCard.
    let tc = TestContext::new();
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let board_a_id = board_a.id;
    let col_a = kanban_domain::Column::new(board_a_id, "Col", 0);
    let card = kanban_domain::Card::new(board_a.id, col_a.id, "Card", 0);
    let card_id = card.id;

    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let board_b_id = board_b.id;
    let col_b = kanban_domain::Column::new(board_b_id, "Col", 0);
    let col_b_id = col_b.id;

    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_column(col_a).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(col_b).unwrap();
    tc.store
        .insert_archived_card(kanban_domain::ArchivedCard::new(card_id, board_a_id))
        .unwrap();

    let context = tc.as_command_context();
    let cmd = RestoreCard {
        card_id,
        column_id: col_b_id,
        position: 0,
        timestamp: Utc::now(),
    };
    cmd.execute(&context).unwrap();

    let restored = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(restored.column_id, col_b_id);
    assert_eq!(
        restored.board_id, board_b_id,
        "board_id syncs to the column actually restored into"
    );
}

#[test]
fn test_restore_card_exceeding_wip_limit_returns_error() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let mut col = kanban_domain::Column::new(board.id, "Col", 0);
    col.wip_limit = Some(1);
    let col_id = col.id;
    let existing = kanban_domain::Card::new(board.id, col_id, "Existing", 0);
    let card = kanban_domain::Card::new(board.id, col_id, "Card", 1);
    let card_id = card.id;
    let board_id = board.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(existing).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store
        .insert_archived_card(kanban_domain::ArchivedCard::new(card_id, board_id))
        .unwrap();

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
    let col = kanban_domain::Column::new(Uuid::new_v4(), "Col", 0);
    let column_id = col.id;
    tc.store.upsert_column(col).unwrap();

    let board = kanban_domain::Board::new("B", Some("TST"));
    let card = kanban_domain::Card::new(board.id, column_id, "Card", 0);
    let card_id = card.id;
    let board_id = board.id;
    tc.store.upsert_card(card).unwrap();
    tc.store
        .insert_archived_card(kanban_domain::ArchivedCard::new(card_id, board_id))
        .unwrap();

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
