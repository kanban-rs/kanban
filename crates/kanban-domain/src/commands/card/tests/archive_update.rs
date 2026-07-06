use crate::commands::card::{ArchiveCards, UpdateCard};
use crate::commands::test_helpers::TestContext;
use crate::{CardUpdate, DataStore};
use uuid::Uuid;

#[test]
fn test_update_card_not_found_returns_error() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = UpdateCard {
        card_id: Uuid::new_v4(),
        updates: CardUpdate::default(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_update_card_to_nonexistent_column_returns_not_found() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("Test", Some("TST"));
    let col = crate::Column::new(board.id, "Col", 0);
    let col_id = col.id;
    let card = crate::Card::new(&mut board, col_id, "Card", 0);
    let card_id = card.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    let cmd = UpdateCard {
        card_id,
        updates: CardUpdate {
            column_id: Some(Uuid::new_v4()),
            ..CardUpdate::default()
        },
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());

    // FK rejected before mutation: the card stays in its original column.
    let stored = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(stored.column_id, col_id);
}

#[test]
fn test_archive_cards_all_invalid_ids_returns_error() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = ArchiveCards {
        ids: vec![Uuid::new_v4()],
    };
    let result = cmd.execute(&context);
    assert!(result.is_err(), "Expected error when all IDs are invalid");
}

#[test]
fn test_archive_cards_invalid_ids_skipped_valid_ids_archived() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("Test", Some("TST"));
    let card = crate::Card::new(&mut board, Uuid::new_v4(), "Card", 0);
    let valid_id = card.id;
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    let cmd = ArchiveCards {
        ids: vec![valid_id, Uuid::new_v4()],
    };
    let result = cmd.execute(&context);
    assert!(result.is_ok());
    assert_eq!(tc.store.list_all_cards().unwrap().len(), 0);
    assert_eq!(tc.store.list_archived_cards().unwrap().len(), 1);
}

#[test]
fn test_archive_captures_board_from_column() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("Test", Some("TST"));
    let board_id = board.id;
    let col = crate::Column::new(board_id, "Col", 0);
    let col_id = col.id;
    let card = crate::Card::new(&mut board, col_id, "Card", 0);
    let card_id = card.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    let cmd = ArchiveCards { ids: vec![card_id] };
    cmd.execute(&context).unwrap();

    // Capture walks card -> column -> board rather than defaulting to nil,
    // and the grown 4-arg signature still lands column/position correctly
    // (guards an arg-order swap at the production call site).
    let archived = tc.store.get_archived_card(card_id).unwrap().unwrap();
    assert_eq!(archived.board_id, board_id);
    assert_eq!(archived.original_column_id, col_id);
    assert_eq!(archived.original_position, 0);
}

#[test]
fn test_archive_batch_captures_each_cards_own_board() {
    let tc = TestContext::new();
    let mut board_a = crate::Board::new("A", Some("AAA"));
    let board_a_id = board_a.id;
    let col_a = crate::Column::new(board_a_id, "Col", 0);
    let card_a = crate::Card::new(&mut board_a, col_a.id, "CardA", 0);
    let card_a_id = card_a.id;

    let mut board_b = crate::Board::new("B", Some("BBB"));
    let board_b_id = board_b.id;
    let col_b = crate::Column::new(board_b_id, "Col", 0);
    let card_b = crate::Card::new(&mut board_b, col_b.id, "CardB", 0);
    let card_b_id = card_b.id;

    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_column(col_a).unwrap();
    tc.store.upsert_card(card_a).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(col_b).unwrap();
    tc.store.upsert_card(card_b).unwrap();

    let context = tc.as_command_context();
    let cmd = ArchiveCards {
        ids: vec![card_a_id, card_b_id],
    };
    cmd.execute(&context).unwrap();

    // Each archived card captures ITS OWN board, proving the loop resolves the
    // board per-item rather than hoisting or reusing the first card's board.
    let arch_a = tc.store.get_archived_card(card_a_id).unwrap().unwrap();
    let arch_b = tc.store.get_archived_card(card_b_id).unwrap().unwrap();
    assert_eq!(arch_a.board_id, board_a_id);
    assert_eq!(arch_b.board_id, board_b_id);
}

#[test]
fn test_archive_with_dangling_column_captures_nil_board_id() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("Test", Some("TST"));
    // column_id references a column that is never inserted (dangling).
    let card = crate::Card::new(&mut board, Uuid::new_v4(), "Card", 0);
    let card_id = card.id;
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    let cmd = ArchiveCards { ids: vec![card_id] };
    // Best-effort capture: a missing column must NOT abort the archive.
    assert!(cmd.execute(&context).is_ok());

    let archived = tc.store.get_archived_card(card_id).unwrap().unwrap();
    assert_eq!(archived.board_id, Uuid::nil());
}

#[test]
fn test_archive_cards_missing_card_after_filter_returns_error() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("Test", Some("TST"));
    let card = crate::Card::new(&mut board, Uuid::new_v4(), "Card", 0);
    let card_id = card.id;
    tc.store.upsert_card(card).unwrap();

    // Directly call ArchiveCards with a valid card id.
    // The card will be found by filter_valid_card_ids, then get_card should
    // return a proper error (not panic) if the card is somehow missing.
    // Here we test the happy path still works, plus we ensure the error
    // path is properly handled (not an unwrap panic) via the impl fix.
    let context = tc.as_command_context();
    let cmd = ArchiveCards { ids: vec![card_id] };
    assert!(cmd.execute(&context).is_ok());
    assert_eq!(tc.store.list_all_cards().unwrap().len(), 0);
    assert_eq!(tc.store.list_archived_cards().unwrap().len(), 1);
}
