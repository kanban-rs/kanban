mod common;
use common::TestContext;
use kanban_domain::*;
use uuid::Uuid;

use kanban_domain::commands::sprint_commands::*;
use kanban_domain::DataStore;

#[test]
fn test_update_sprint_not_found_returns_error() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = UpdateSprint {
        sprint_id: Uuid::new_v4(),
        updates: SprintUpdate::default(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_update_sprint_name_with_nonexistent_board_returns_error() {
    let tc = TestContext::new();
    let nonexistent_board_id = Uuid::new_v4();
    let sprint = kanban_domain::Sprint::new(nonexistent_board_id, 1, None, None::<String>);
    let sprint_id = sprint.id;
    tc.store.upsert_sprint(sprint).unwrap();

    let context = tc.as_command_context();
    let cmd = UpdateSprint {
        sprint_id,
        updates: SprintUpdate {
            name: Some("New Name".to_string()),
            ..Default::default()
        },
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_activate_sprint_not_found_returns_error() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = ActivateSprint {
        sprint_id: Uuid::new_v4(),
        duration_days: 14,
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_complete_sprint_not_found_returns_error() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = CompleteSprint {
        sprint_id: Uuid::new_v4(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_cancel_sprint_not_found_returns_error() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = CancelSprint {
        sprint_id: Uuid::new_v4(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_create_sprint_command_funnels_through_factory_with_injected_id() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", None::<String>);
    let board_id = board.id;
    tc.store.upsert_board(board).unwrap();

    let context = tc.as_command_context();
    let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let cmd = CreateSprint {
        id,
        board_id,
        name: None,
        default_sprint_prefix: "Sprint".to_string(),
        explicit_prefix: Some("SPR".to_string()),
        auto_consume_name: false,
    };
    cmd.execute(&context).unwrap();

    let sprint = tc.store.get_sprint(id).unwrap().unwrap();
    // Injected id carried verbatim, server values minted from the board:
    assert_eq!(sprint.id, id);
    assert_eq!(sprint.sprint_number, 1);
    assert_eq!(sprint.prefix, Some("SPR".to_string()));
    // Factory-seeded lifecycle defaults (Sprint::create):
    assert_eq!(sprint.status, kanban_domain::SprintStatus::Planning);
    assert_eq!(sprint.start_date, None);
    assert_eq!(sprint.end_date, None);
    assert_eq!(
        sprint.created_at, sprint.updated_at,
        "no observable intermediate update — one Sprint::create call"
    );
}

#[test]
fn test_create_sprint_auto_consume_name_uses_name_pool() {
    let tc = TestContext::new();
    let mut board = kanban_domain::Board::new("Test", None::<String>);
    board.sprint_names = vec!["Alpha".to_string(), "Beta".to_string()];
    let board_id = board.id;
    tc.store.upsert_board(board).unwrap();

    let context = tc.as_command_context();
    let cmd = CreateSprint {
        id: Uuid::new_v4(),
        board_id,
        name: None,
        default_sprint_prefix: "Sprint".to_string(),
        explicit_prefix: None,
        auto_consume_name: true,
    };
    cmd.execute(&context).unwrap();

    let sprints = tc.store.list_all_sprints().unwrap();
    assert_eq!(sprints.len(), 1);
    let sprint = &sprints[0];
    let board = tc.store.get_board(board_id).unwrap().unwrap();
    assert_eq!(
        sprint.get_name(&board),
        Some("Alpha"),
        "auto_consume_name should consume the first available sprint name"
    );
}

#[test]
fn test_update_sprint_card_prefix_locked_after_card_assigned_returns_validation_error() {
    let tc = TestContext::new();
    let mut board = kanban_domain::Board::new("B", Some("KAN"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let sprint = kanban_domain::Sprint::new(board.id, 1, None, Some("SPR"));
    let sprint_id = sprint.id;
    let mut card = kanban_domain::Card::new(&mut board, col.id, "C", 0);
    card.sprint_id = Some(sprint_id);
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_sprint(sprint).unwrap();
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    let cmd = UpdateSprint {
        sprint_id,
        updates: kanban_domain::SprintUpdate {
            card_prefix: kanban_domain::FieldUpdate::Set("NEW".to_string()),
            ..Default::default()
        },
    };
    let err = cmd.execute(&context).unwrap_err();
    assert!(err.is_validation());
}

#[test]
fn test_update_sprint_card_prefix_locked_after_archived_card_assigned_returns_validation_error() {
    let tc = TestContext::new();
    let mut board = kanban_domain::Board::new("B", Some("KAN"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let sprint = kanban_domain::Sprint::new(board.id, 1, None, Some("SPR"));
    let sprint_id = sprint.id;
    let mut card = kanban_domain::Card::new(&mut board, col.id, "C", 0);
    card.sprint_id = Some(sprint_id);
    let card_id = card.id;
    let board_id = board.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_sprint(sprint).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store
        .insert_archived_card(kanban_domain::ArchivedCard::new(card_id, board_id))
        .unwrap();

    let context = tc.as_command_context();
    let cmd = UpdateSprint {
        sprint_id,
        updates: kanban_domain::SprintUpdate {
            card_prefix: kanban_domain::FieldUpdate::Set("NEW".to_string()),
            ..Default::default()
        },
    };
    let err = cmd.execute(&context).unwrap_err();
    assert!(err.is_validation());
}

#[test]
fn test_update_sprint_clear_card_prefix_locked_after_card_assigned_returns_validation_error() {
    let tc = TestContext::new();
    let mut board = kanban_domain::Board::new("B", Some("KAN"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let sprint = kanban_domain::Sprint::new(board.id, 1, None, Some("SPR"));
    let sprint_id = sprint.id;
    let mut card = kanban_domain::Card::new(&mut board, col.id, "C", 0);
    card.sprint_id = Some(sprint_id);
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_sprint(sprint).unwrap();
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    let cmd = UpdateSprint {
        sprint_id,
        updates: kanban_domain::SprintUpdate {
            card_prefix: kanban_domain::FieldUpdate::Clear,
            ..Default::default()
        },
    };
    let err = cmd.execute(&context).unwrap_err();
    assert!(err.is_validation());
}

#[test]
fn test_update_sprint_card_prefix_collides_with_board_prefix_returns_validation_error() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", Some("KAN"));
    let board_id = board.id;
    let sprint = kanban_domain::Sprint::new(board_id, 1, None, Some("SPR"));
    let sprint_id = sprint.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_sprint(sprint).unwrap();

    let context = tc.as_command_context();
    let cmd = UpdateSprint {
        sprint_id,
        updates: kanban_domain::SprintUpdate {
            card_prefix: kanban_domain::FieldUpdate::Set("KAN".to_string()),
            ..Default::default()
        },
    };
    let err = cmd.execute(&context).unwrap_err();
    assert!(err.is_validation());
}

#[test]
fn test_update_sprint_card_prefix_case_insensitive_collision_returns_validation_error() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", Some("KAN"));
    let board_id = board.id;
    let sprint = kanban_domain::Sprint::new(board_id, 1, None, Some("SPR"));
    let sprint_id = sprint.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_sprint(sprint).unwrap();

    let context = tc.as_command_context();
    let cmd = UpdateSprint {
        sprint_id,
        updates: kanban_domain::SprintUpdate {
            card_prefix: kanban_domain::FieldUpdate::Set("kan".to_string()),
            ..Default::default()
        },
    };
    let err = cmd.execute(&context).unwrap_err();
    assert!(err.is_validation());
}

#[test]
fn test_update_sprint_card_prefix_collides_with_sibling_sprint_returns_validation_error() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", Some("KAN"));
    let board_id = board.id;
    let mut sprint1 = kanban_domain::Sprint::new(board_id, 1, None, None::<String>);
    sprint1.card_prefix = Some("SPR".to_string());
    let sprint2 = kanban_domain::Sprint::new(board_id, 2, None, None::<String>);
    let sprint2_id = sprint2.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_sprint(sprint1).unwrap();
    tc.store.upsert_sprint(sprint2).unwrap();

    let context = tc.as_command_context();
    let cmd = UpdateSprint {
        sprint_id: sprint2_id,
        updates: kanban_domain::SprintUpdate {
            card_prefix: kanban_domain::FieldUpdate::Set("SPR".to_string()),
            ..Default::default()
        },
    };
    let err = cmd.execute(&context).unwrap_err();
    assert!(err.is_validation());
}

#[test]
fn test_delete_sprint_clears_sprint_from_cards_with_command_timestamp() {
    use chrono::{TimeZone, Utc};

    let tc = TestContext::new();
    let mut board = kanban_domain::Board::new("B", Some("KAN"));
    let board_id = board.id;
    let col = kanban_domain::Column::new(board_id, "Col", 0);
    let sprint = kanban_domain::Sprint::new(board_id, 1, None, None::<String>);
    let sprint_id = sprint.id;

    let mut card = kanban_domain::Card::new(&mut board, col.id, "C", 0);
    card.sprint_id = Some(sprint_id);
    let card_id = card.id;

    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col.clone()).unwrap();
    tc.store.upsert_sprint(sprint).unwrap();
    tc.store.upsert_card(card).unwrap();

    let fixed_time = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
    let context = tc.as_command_context();
    let cmd = DeleteSprint {
        sprint_id,
        timestamp: fixed_time,
    };
    cmd.execute(&context).unwrap();

    let card = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(
        card.updated_at, fixed_time,
        "clear_sprint_from_cards should use the command's timestamp, not Utc::now()"
    );
    assert_eq!(card.sprint_id, None);
}

#[test]
fn test_delete_sprint_uses_embedded_timestamp() {
    use chrono::{TimeZone, Utc};

    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", Some("KAN"));
    let board_id = board.id;
    let col = kanban_domain::Column::new(board_id, "Col", 0);
    let sprint = kanban_domain::Sprint::new(board_id, 1, None, None::<String>);
    let sprint_id = sprint.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col.clone()).unwrap();
    tc.store.upsert_sprint(sprint).unwrap();

    let card = kanban_domain::Card {
        id: Uuid::new_v4(),
        column_id: col.id,
        board_id,
        title: "C".to_string(),
        description: None,
        priority: kanban_domain::CardPriority::Medium,
        status: kanban_domain::CardStatus::Todo,
        position: 0,
        due_date: None,
        points: None,
        card_number: 1,
        sprint_id: Some(sprint_id),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        sprint_logs: Vec::new(),
    };
    let card_id = card.id;
    tc.store.upsert_card(card).unwrap();
    tc.store
        .insert_archived_card(kanban_domain::ArchivedCard::new(card_id, board_id))
        .unwrap();

    let fixed_time = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
    let context = tc.as_command_context();
    let cmd = DeleteSprint {
        sprint_id,
        timestamp: fixed_time,
    };
    cmd.execute(&context).unwrap();

    // Reference-marker model: the sprint binding cleared by DeleteSprint lives
    // on the LIVE card (fetched by the marker's entity_id), not the marker.
    let archived_cards = tc.store.list_archived_cards().unwrap();
    assert_eq!(archived_cards.len(), 1);
    let live = tc
        .store
        .get_card(archived_cards[0].entity_id)
        .unwrap()
        .unwrap();
    assert_eq!(live.updated_at, fixed_time);
    assert_eq!(live.sprint_id, None);
}

#[test]
fn test_update_sprint_card_prefix_unique_valid_succeeds() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", Some("KAN"));
    let board_id = board.id;
    let sprint = kanban_domain::Sprint::new(board_id, 1, None, Some("SPR"));
    let sprint_id = sprint.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_sprint(sprint).unwrap();

    let context = tc.as_command_context();
    let cmd = UpdateSprint {
        sprint_id,
        updates: kanban_domain::SprintUpdate {
            card_prefix: kanban_domain::FieldUpdate::Set("UNIQUE".to_string()),
            ..Default::default()
        },
    };
    assert!(cmd.execute(&context).is_ok());
    let sprint = tc.store.get_sprint(sprint_id).unwrap().unwrap();
    assert_eq!(sprint.card_prefix, Some("UNIQUE".to_string()));
}

#[test]
fn test_update_sprint_to_its_own_existing_card_prefix_succeeds() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", Some("KAN"));
    let board_id = board.id;
    let sprint = kanban_domain::Sprint::new(board_id, 1, None, Some("SPR"));
    let sprint_id = sprint.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_sprint(sprint).unwrap();

    let context = tc.as_command_context();
    let cmd = UpdateSprint {
        sprint_id,
        updates: kanban_domain::SprintUpdate {
            card_prefix: kanban_domain::FieldUpdate::Set("SPR".to_string()),
            ..Default::default()
        },
    };
    assert!(cmd.execute(&context).is_ok());
}

#[test]
fn test_update_sprint_name_allocates_from_board_name_pool() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", Some("KAN"));
    let board_id = board.id;
    let sprint = kanban_domain::Sprint::new(board_id, 1, None, None::<String>);
    let sprint_id = sprint.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_sprint(sprint).unwrap();

    let context = tc.as_command_context();
    let cmd = UpdateSprint {
        sprint_id,
        updates: kanban_domain::SprintUpdate {
            name: Some("My Sprint".to_string()),
            ..Default::default()
        },
    };
    cmd.execute(&context).unwrap();

    let board = tc.store.get_board(board_id).unwrap().unwrap();
    assert!(board.sprint_names.contains(&"My Sprint".to_string()));
    let sprint = tc.store.get_sprint(sprint_id).unwrap().unwrap();
    assert!(sprint.name_index.is_some());
}
