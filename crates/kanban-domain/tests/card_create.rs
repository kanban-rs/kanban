mod common;
use common::TestContext;

use kanban_domain::commands::card::CreateCard;

use chrono::Utc;
use kanban_domain::{CreateCardOptions, DataStore, DomainError, KanbanError};
use uuid::Uuid;

#[test]
fn test_create_card_command_funnels_through_factory_seeds_defaults() {
    let tc = TestContext::new();
    let mut board = kanban_domain::Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let board_id = board.id;
    let column_id = col.id;
    board.card_counter = 1;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();

    let context = tc.as_command_context();
    let card_id = Uuid::new_v4();
    let cmd = CreateCard {
        id: card_id,
        card_number: 1,
        board_id,
        column_id,
        title: "Funnelled".to_string(),
        position: 0,
        options: CreateCardOptions {
            description: Some("d".to_string()),
            priority: Some(kanban_domain::CardPriority::High),
            ..Default::default()
        },
        timestamp: Utc::now(),
    };
    cmd.execute(&context).unwrap();

    let card = tc.store.get_card(card_id).unwrap().unwrap();
    // Factory-seeded server-managed defaults (Card::create), even with options:
    assert_eq!(card.status, kanban_domain::CardStatus::Todo);
    assert_eq!(card.completed_at, None);
    assert!(card.sprint_logs.is_empty());
    // Create fields applied in the single create (no follow-up patch):
    assert_eq!(card.description, Some("d".to_string()));
    assert_eq!(card.priority, kanban_domain::CardPriority::High);
    assert_eq!(
        card.updated_at, card.created_at,
        "no observable intermediate update — one Card::create call"
    );
    // Board counter bumped past the minted number (sibling-entity write):
    let bumped = tc.store.get_board(board_id).unwrap().unwrap();
    assert_eq!(bumped.card_counter, 2);
}

#[test]
fn test_create_card_sets_board_id() {
    let tc = TestContext::new();
    let mut board = kanban_domain::Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let board_id = board.id;
    let column_id = col.id;
    board.card_counter = 1;
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
    assert_eq!(
        card.board_id, board_id,
        "the created card carries its own durable board_id, not just column_id"
    );
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
fn test_create_card_exceeding_wip_limit_returns_error() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let mut column = kanban_domain::Column::new(board.id, "Limited", 0);
    column.wip_limit = Some(1);
    let column_id = column.id;
    let existing = kanban_domain::Card::new(board.id, column_id, "Existing", 0);
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
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let mut column = kanban_domain::Column::new(board.id, "Limited", 0);
    column.wip_limit = Some(2);
    let column_id = column.id;
    let card1 = kanban_domain::Card::new(board.id, column_id, "C1", 0);
    let card2 = kanban_domain::Card::new(board.id, column_id, "C2", 1);
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
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let mut column = kanban_domain::Column::new(board.id, "Limited", 0);
    column.wip_limit = Some(2);
    let column_id = column.id;
    let card1 = kanban_domain::Card::new(board.id, column_id, "C1", 0);
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
fn test_create_card_with_sprint_id_assigns_card_to_sprint() {
    let tc = TestContext::new();
    let mut board = kanban_domain::Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let sprint = kanban_domain::Sprint::new(board.id, 1, None, None::<String>);
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
    let board = kanban_domain::Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
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
    let board = kanban_domain::Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
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
    let board = kanban_domain::Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
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
            priority: Some(kanban_domain::CardPriority::High),
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
    let mut board = kanban_domain::Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let sprint = kanban_domain::Sprint::new(board.id, 1, None, None::<String>);
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
            priority: Some(kanban_domain::CardPriority::High),
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
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let col_a = kanban_domain::Column::new(board_a.id, "Col", 0);
    // Sprint belongs to board B.
    let sprint_b = kanban_domain::Sprint::new(board_b.id, 1, None, None::<String>);
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
    let board = kanban_domain::Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
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
