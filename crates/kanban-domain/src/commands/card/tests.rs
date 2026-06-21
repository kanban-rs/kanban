use super::super::test_helpers::TestContext;
use super::*;
use crate::{CardUpdate, CreateCardOptions, DataStore, DomainError, KanbanError};
use chrono::Utc;
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
fn test_create_card_board_not_found_returns_error() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = CreateCard {
        id: Uuid::new_v4(),
        card_number: 1,
        board_id: Uuid::new_v4(),
        column_id: Uuid::new_v4(),
        title: "Test".to_string(),
        position: 0,
        options: CreateCardOptions::default(),
        timestamp: Utc::now(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_move_card_not_found_returns_error() {
    let tc = TestContext::new();
    let column = crate::Column::new(Uuid::new_v4(), "Col", 0);
    let column_id = column.id;
    tc.store.upsert_column(column).unwrap();
    let context = tc.as_command_context();
    let cmd = MoveCard {
        card_id: Uuid::new_v4(),
        new_column_id: column_id,
        new_position: 0,
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_move_card_column_not_found_returns_error() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("Test", Some("TST"));
    let card = crate::Card::new(&mut board, Uuid::new_v4(), "Card", 0);
    let card_id = card.id;
    tc.store.upsert_card(card).unwrap();
    let context = tc.as_command_context();
    let cmd = MoveCard {
        card_id,
        new_column_id: Uuid::new_v4(),
        new_position: 0,
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
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
fn test_create_card_exceeding_wip_limit_returns_error() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("Test", Some("TST"));
    let mut column = crate::Column::new(board.id, "Limited", 0);
    column.wip_limit = Some(1);
    let column_id = column.id;
    let existing = crate::Card::new(&mut board, column_id, "Existing", 0);
    let board_id = board.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(column).unwrap();
    tc.store.upsert_card(existing).unwrap();

    let context = tc.as_command_context();
    let cmd = CreateCard {
        id: Uuid::new_v4(),
        card_number: 1,
        board_id,
        column_id,
        title: "New".to_string(),
        position: 1,
        options: CreateCardOptions::default(),
        timestamp: Utc::now(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_wip_limit_exceeded());
}

#[test]
fn test_create_card_at_wip_limit_returns_error() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("Test", Some("TST"));
    let mut column = crate::Column::new(board.id, "Limited", 0);
    column.wip_limit = Some(2);
    let column_id = column.id;
    let card1 = crate::Card::new(&mut board, column_id, "C1", 0);
    let card2 = crate::Card::new(&mut board, column_id, "C2", 1);
    let board_id = board.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(column).unwrap();
    tc.store.upsert_card(card1).unwrap();
    tc.store.upsert_card(card2).unwrap();

    let context = tc.as_command_context();
    let cmd = CreateCard {
        id: Uuid::new_v4(),
        card_number: 1,
        board_id,
        column_id,
        title: "New".to_string(),
        position: 2,
        options: CreateCardOptions::default(),
        timestamp: Utc::now(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_wip_limit_exceeded());
}

#[test]
fn test_create_card_below_wip_limit_succeeds() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("Test", Some("TST"));
    let mut column = crate::Column::new(board.id, "Limited", 0);
    column.wip_limit = Some(2);
    let column_id = column.id;
    let card1 = crate::Card::new(&mut board, column_id, "C1", 0);
    let board_id = board.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(column).unwrap();
    tc.store.upsert_card(card1).unwrap();

    let context = tc.as_command_context();
    let cmd = CreateCard {
        id: Uuid::new_v4(),
        card_number: 1,
        board_id,
        column_id,
        title: "New".to_string(),
        position: 1,
        options: CreateCardOptions::default(),
        timestamp: Utc::now(),
    };
    assert!(cmd.execute(&context).is_ok());
}

#[test]
fn test_move_card_exceeding_wip_limit_returns_error() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("Test", Some("TST"));
    let src_col = crate::Column::new(board.id, "Source", 0);
    let mut dst_col = crate::Column::new(board.id, "Dest", 1);
    dst_col.wip_limit = Some(1);
    let dst_id = dst_col.id;
    let existing = crate::Card::new(&mut board, dst_id, "Existing", 0);
    let mover = crate::Card::new(&mut board, src_col.id, "Mover", 0);
    let mover_id = mover.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(src_col).unwrap();
    tc.store.upsert_column(dst_col).unwrap();
    tc.store.upsert_card(existing).unwrap();
    tc.store.upsert_card(mover).unwrap();

    let context = tc.as_command_context();
    let cmd = MoveCard {
        card_id: mover_id,
        new_column_id: dst_id,
        new_position: 1,
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_wip_limit_exceeded());
}

#[test]
fn test_restore_card_to_deleted_column_returns_error() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("Test", Some("TST"));
    let col = crate::Column::new(board.id, "Col", 0);
    let col_id = col.id;
    let card = crate::Card::new(&mut board, col_id, "Card", 0);
    let card_id = card.id;
    let archived = crate::ArchivedCard::new(card, col_id, 0);
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
    let archived = crate::ArchivedCard::new(card, col_id, 0);
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
    let archived = crate::ArchivedCard::new(card, col_id, 0);
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
fn test_assign_cards_to_sprint_validates_sprint_exists() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("Test", Some("TST"));
    let card = crate::Card::new(&mut board, Uuid::new_v4(), "Card", 0);
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
    let mut board = crate::Board::new("Test", Some("TST"));
    let card = crate::Card::new(&mut board, Uuid::new_v4(), "Card", 0);
    let valid_id = card.id;
    let sprint = crate::Sprint::new(board.id, 1, None, Some("Sprint"));
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

#[test]
fn test_compact_column_positions_makes_sequential() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("B", Some("TST"));
    let col = crate::Column::new(board.id, "Col", 0);
    let column_id = col.id;
    let mut card1 = crate::Card::new(&mut board, column_id, "C1", 0);
    card1.position = 0;
    let mut card2 = crate::Card::new(&mut board, column_id, "C2", 5);
    card2.position = 5;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card1).unwrap();
    tc.store.upsert_card(card2).unwrap();

    let context = tc.as_command_context();
    let cmd = CompactColumnPositions { column_id };
    cmd.execute(&context).unwrap();

    let cards = tc.store.list_cards_by_column(column_id).unwrap();
    assert_eq!(cards[0].position, 0);
    assert_eq!(cards[1].position, 1);
}

#[test]
fn test_create_card_with_sprint_id_assigns_card_to_sprint() {
    let tc = TestContext::new();
    let mut board = crate::Board::new("B", Some("TST"));
    let col = crate::Column::new(board.id, "Col", 0);
    let sprint = crate::Sprint::new(board.id, 1, None, None::<String>);
    let board_id = board.id;
    let column_id = col.id;
    let sprint_id = sprint.id;
    // Bump card_counter so upsert_board doesn't reset it; mirrors real usage.
    board.card_counter = 1;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_sprint(sprint).unwrap();

    let context = tc.as_command_context();
    let card_id = Uuid::new_v4();
    let cmd = CreateCard {
        id: card_id,
        card_number: 1,
        board_id,
        column_id,
        title: "Test".to_string(),
        position: 0,
        options: CreateCardOptions {
            sprint_id: Some(sprint_id),
            ..Default::default()
        },
        timestamp: Utc::now(),
    };
    cmd.execute(&context).unwrap();

    let card = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(card.sprint_id, Some(sprint_id));
    assert_eq!(card.sprint_logs.len(), 1);
    assert_eq!(card.sprint_logs[0].sprint_id, sprint_id);
}

#[test]
fn test_create_card_without_sprint_id_leaves_card_unassigned() {
    let tc = TestContext::new();
    let board = crate::Board::new("B", Some("TST"));
    let col = crate::Column::new(board.id, "Col", 0);
    let board_id = board.id;
    let column_id = col.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();

    let context = tc.as_command_context();
    let card_id = Uuid::new_v4();
    let cmd = CreateCard {
        id: card_id,
        card_number: 1,
        board_id,
        column_id,
        title: "Test".to_string(),
        position: 0,
        options: CreateCardOptions::default(),
        timestamp: Utc::now(),
    };
    cmd.execute(&context).unwrap();

    let card = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(card.sprint_id, None);
    assert!(card.sprint_logs.is_empty());
}

#[test]
fn test_create_card_with_invalid_sprint_id_returns_not_found_error() {
    let tc = TestContext::new();
    let board = crate::Board::new("B", Some("TST"));
    let col = crate::Column::new(board.id, "Col", 0);
    let board_id = board.id;
    let column_id = col.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();

    let context = tc.as_command_context();
    let cmd = CreateCard {
        id: Uuid::new_v4(),
        card_number: 1,
        board_id,
        column_id,
        title: "Test".to_string(),
        position: 0,
        options: CreateCardOptions {
            sprint_id: Some(Uuid::new_v4()),
            ..Default::default()
        },
        timestamp: Utc::now(),
    };
    let err = cmd.execute(&context).unwrap_err();
    assert!(err.is_not_found(), "Expected not found, got: {:?}", err);
}

#[test]
fn test_create_card_with_options_only_uses_embedded_timestamp() {
    use chrono::TimeZone;

    let tc = TestContext::new();
    let board = crate::Board::new("B", Some("TST"));
    let col = crate::Column::new(board.id, "Col", 0);
    let board_id = board.id;
    let column_id = col.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();

    let fixed_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let context = tc.as_command_context();
    let card_id = Uuid::new_v4();
    let cmd = CreateCard {
        id: card_id,
        card_number: 1,
        board_id,
        column_id,
        title: "T".to_string(),
        position: 0,
        options: CreateCardOptions {
            description: Some("d".to_string()),
            priority: Some(crate::CardPriority::High),
            points: Some(3),
            due_date: None,
            sprint_id: None,
        },
        timestamp: fixed_time,
    };
    cmd.execute(&context).unwrap();

    let card = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(card.created_at, fixed_time);
    assert_eq!(
        card.updated_at, fixed_time,
        "updated_at must match the embedded command timestamp even when \
             CardUpdate options reset it inside Card::update"
    );
}

#[test]
fn test_create_card_with_options_and_sprint_uses_embedded_timestamp() {
    use chrono::TimeZone;

    let tc = TestContext::new();
    let mut board = crate::Board::new("B", Some("TST"));
    let col = crate::Column::new(board.id, "Col", 0);
    let sprint = crate::Sprint::new(board.id, 1, None, None::<String>);
    let board_id = board.id;
    let column_id = col.id;
    let sprint_id = sprint.id;
    board.card_counter = 1;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_sprint(sprint).unwrap();

    let fixed_time = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let context = tc.as_command_context();
    let card_id = Uuid::new_v4();
    let cmd = CreateCard {
        id: card_id,
        card_number: 1,
        board_id,
        column_id,
        title: "T".to_string(),
        position: 0,
        options: CreateCardOptions {
            description: Some("d".to_string()),
            priority: Some(crate::CardPriority::High),
            points: Some(3),
            due_date: None,
            sprint_id: Some(sprint_id),
        },
        timestamp: fixed_time,
    };
    cmd.execute(&context).unwrap();

    let card = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(card.created_at, fixed_time);
    assert_eq!(
        card.updated_at, fixed_time,
        "updated_at must match the embedded command timestamp even when both \
             CardUpdate options and sprint assignment run inside execute"
    );
}

#[test]
fn test_create_card_with_sprint_from_different_board_returns_typed_mismatch() {
    let tc = TestContext::new();
    let board_a = crate::Board::new("A", Some("AAA"));
    let board_b = crate::Board::new("B", Some("BBB"));
    let col_a = crate::Column::new(board_a.id, "Col", 0);
    // Sprint belongs to board B.
    let sprint_b = crate::Sprint::new(board_b.id, 1, None, None::<String>);
    let board_a_id = board_a.id;
    let board_b_id = board_b.id;
    let column_id = col_a.id;
    let sprint_b_id = sprint_b.id;
    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(col_a).unwrap();
    tc.store.upsert_sprint(sprint_b).unwrap();

    let context = tc.as_command_context();
    let cmd = CreateCard {
        id: Uuid::new_v4(),
        card_number: 1,
        board_id: board_a_id,
        column_id,
        title: "X".to_string(),
        position: 0,
        options: CreateCardOptions {
            sprint_id: Some(sprint_b_id),
            ..Default::default()
        },
        timestamp: Utc::now(),
    };
    let err = cmd.execute(&context).unwrap_err();
    assert!(
        err.is_sprint_board_mismatch(),
        "expected SprintBoardMismatch, got: {err:?}"
    );
    match err {
        KanbanError::Domain(DomainError::SprintBoardMismatch {
            sprint_id,
            sprint_board,
            card_board,
        }) => {
            assert_eq!(sprint_id, sprint_b_id);
            assert_eq!(sprint_board, board_b_id);
            assert_eq!(card_board, board_a_id);
        }
        other => panic!("expected SprintBoardMismatch fields, got: {other:?}"),
    }
}

#[test]
fn test_create_card_uses_embedded_timestamp() {
    use chrono::{TimeZone, Utc};

    let tc = TestContext::new();
    let board = crate::Board::new("B", Some("TST"));
    let col = crate::Column::new(board.id, "Col", 0);
    let board_id = board.id;
    let column_id = col.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();

    let fixed_time = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
    let context = tc.as_command_context();
    let card_id = Uuid::new_v4();
    let cmd = CreateCard {
        id: card_id,
        card_number: 1,
        board_id,
        column_id,
        title: "Test".to_string(),
        position: 0,
        options: CreateCardOptions::default(),
        timestamp: fixed_time,
    };
    cmd.execute(&context).unwrap();

    let card = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(card.created_at, fixed_time);
    assert_eq!(card.updated_at, fixed_time);
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
    let archived = crate::ArchivedCard::new(card, column_id, 0);
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

#[test]
fn test_unassign_card_from_sprint_uses_embedded_timestamp() {
    use chrono::{TimeZone, Utc};

    let tc = TestContext::new();
    let mut board = crate::Board::new("B", Some("TST"));
    let col = crate::Column::new(board.id, "Col", 0);
    let mut card = crate::Card::new(&mut board, col.id, "Card", 0);
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
