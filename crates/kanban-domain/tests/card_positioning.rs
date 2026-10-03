mod common;
use common::TestContext;
use uuid::Uuid;

use kanban_domain::commands::card::*;
use kanban_domain::commands::Command;
use kanban_domain::*;

#[test]
fn test_move_card_not_found_returns_error() {
    let tc = TestContext::new();
    let column = kanban_domain::Column::new(Uuid::new_v4(), "Col", 0);
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
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let card = kanban_domain::Card::new(board.id, Uuid::new_v4(), "Card", 0);
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
fn test_move_card_exceeding_wip_limit_returns_error() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("Test", Some("TST"));
    let src_col = kanban_domain::Column::new(board.id, "Source", 0);
    let mut dst_col = kanban_domain::Column::new(board.id, "Dest", 1);
    dst_col.wip_limit = Some(1);
    let dst_id = dst_col.id;
    let existing = kanban_domain::Card::new(board.id, dst_id, "Existing", 0);
    let mover = kanban_domain::Card::new(board.id, src_col.id, "Mover", 0);
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
fn test_move_card_updates_board_id_on_cross_board_move() {
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

    let context = tc.as_command_context();
    let cmd = MoveCard {
        card_id,
        new_column_id: col_b_id,
        new_position: 0,
    };
    cmd.execute(&context).unwrap();

    let moved = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(moved.column_id, col_b_id);
    assert_eq!(
        moved.board_id, board_b_id,
        "moving a card to a column on a different board keeps board_id in sync -- \
         cross-board moves stay possible and correct, not blocked"
    );
}

#[test]
fn test_move_card_cross_board_detaches_its_sprint_and_closes_the_log() {
    let tc = TestContext::new();
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let board_a_id = board_a.id;
    let col_a = kanban_domain::Column::new(board_a_id, "Col", 0);
    let sprint_a = kanban_domain::Sprint::new(board_a_id, 1, None, Some("Sprint A"));
    let sa = sprint_a.id;
    let mut card = kanban_domain::Card::new(board_a.id, col_a.id, "Card", 0);
    card.assign_to_sprint(sa, 1, None::<String>, "Planning", chrono::Utc::now());
    let card_id = card.id;

    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let board_b_id = board_b.id;
    let col_b = kanban_domain::Column::new(board_b_id, "Col", 0);
    let col_b_id = col_b.id;

    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_column(col_a).unwrap();
    tc.store.upsert_sprint(sprint_a).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(col_b).unwrap();

    let context = tc.as_command_context();
    let cmd = MoveCard {
        card_id,
        new_column_id: col_b_id,
        new_position: 0,
    };
    cmd.execute(&context).unwrap();

    let moved = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(moved.sprint_id, None);
    assert_eq!(moved.board_id, board_b_id);
    assert!(moved
        .sprint_logs
        .last()
        .expect("sprint log entry")
        .ended_at
        .is_some());
}

#[test]
fn test_move_card_cross_board_keeps_a_sprint_that_lives_on_the_target_board() {
    let tc = TestContext::new();
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let col_a = kanban_domain::Column::new(board_a.id, "Col", 0);

    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let board_b_id = board_b.id;
    let col_b = kanban_domain::Column::new(board_b_id, "Col", 0);
    let col_b_id = col_b.id;
    let sprint_b = kanban_domain::Sprint::new(board_b_id, 1, None, Some("Sprint B"));
    let sb = sprint_b.id;

    let mut card = kanban_domain::Card::new(board_a.id, col_a.id, "Card", 0);
    card.assign_to_sprint(sb, 1, None::<String>, "Planning", chrono::Utc::now());
    let card_id = card.id;

    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_column(col_a).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(col_b).unwrap();
    tc.store.upsert_sprint(sprint_b).unwrap();
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    let cmd = MoveCard {
        card_id,
        new_column_id: col_b_id,
        new_position: 0,
    };
    cmd.execute(&context).unwrap();

    let moved = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(moved.sprint_id, Some(sb));
}

#[test]
fn test_move_card_within_its_board_keeps_its_sprint() {
    let tc = TestContext::new();
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let board_a_id = board_a.id;
    let col_a = kanban_domain::Column::new(board_a_id, "Col A", 0);
    let col_a2 = kanban_domain::Column::new(board_a_id, "Col A2", 1);
    let col_a2_id = col_a2.id;

    // Cross-board binding predates this invariant (a same-board move must
    // never touch it, even though it is already inconsistent).
    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let board_b_id = board_b.id;
    let sprint_b = kanban_domain::Sprint::new(board_b_id, 1, None, Some("Sprint B"));
    let sb = sprint_b.id;

    let mut card = kanban_domain::Card::new(board_a.id, col_a.id, "Card", 0);
    card.assign_to_sprint(sb, 1, None::<String>, "Planning", chrono::Utc::now());
    let card_id = card.id;

    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_column(col_a).unwrap();
    tc.store.upsert_column(col_a2).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_sprint(sprint_b).unwrap();
    tc.store.upsert_card(card).unwrap();

    let context = tc.as_command_context();
    let cmd = MoveCard {
        card_id,
        new_column_id: col_a2_id,
        new_position: 0,
    };
    cmd.execute(&context).unwrap();

    let moved = tc.store.get_card(card_id).unwrap().unwrap();
    assert_eq!(moved.sprint_id, Some(sb));
    assert_eq!(moved.column_id, col_a2_id);
    assert_eq!(moved.board_id, board_a_id);
}

#[test]
fn test_move_card_cross_board_inverse_restores_the_sprint_attachment() {
    let tc = TestContext::new();
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let col_a = kanban_domain::Column::new(board_a.id, "Col", 0);
    let col_a_id = col_a.id;
    let sprint_a = kanban_domain::Sprint::new(board_a.id, 1, None, Some("Sprint A"));
    let sa = sprint_a.id;

    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let col_b = kanban_domain::Column::new(board_b.id, "Col", 0);
    let col_b_id = col_b.id;

    let mut card = kanban_domain::Card::new(board_a.id, col_a.id, "Card", 0);
    card.assign_to_sprint(sa, 1, None::<String>, "Planning", chrono::Utc::now());
    let card_id = card.id;
    let original_sprint_id = card.sprint_id;
    let original_sprint_logs = card.sprint_logs.clone();
    let original_updated_at = card.updated_at;

    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_column(col_a).unwrap();
    tc.store.upsert_sprint(sprint_a).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(col_b).unwrap();

    let cmd = MoveCard {
        card_id,
        new_column_id: col_b_id,
        new_position: 0,
    };
    let inverse = cmd.capture_inverse(&tc.store).unwrap();
    assert_eq!(inverse.len(), 2);
    match &inverse[0] {
        Command::Card(CardCommand::Move(m)) => {
            assert_eq!(m.card_id, card_id);
            assert_eq!(m.new_column_id, col_a_id);
            assert_eq!(m.new_position, 0);
        }
        other => panic!("expected Move, got {other:?}"),
    }
    match &inverse[1] {
        Command::Card(CardCommand::RestoreSprintAttachment(r)) => {
            assert_eq!(r.card_id, card_id);
            assert_eq!(r.sprint_id, original_sprint_id);
            assert_eq!(r.sprint_logs, original_sprint_logs);
            assert_eq!(r.updated_at, original_updated_at);
        }
        other => panic!("expected RestoreSprintAttachment, got {other:?}"),
    }
}

#[test]
fn test_move_card_cross_board_inverse_restores_a_kept_binding_to_the_target_boards_sprint() {
    let tc = TestContext::new();
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let col_a = kanban_domain::Column::new(board_a.id, "Col", 0);

    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let board_b_id = board_b.id;
    let col_b = kanban_domain::Column::new(board_b_id, "Col", 0);
    let col_b_id = col_b.id;
    let sprint_b = kanban_domain::Sprint::new(board_b_id, 1, None, Some("Sprint B"));
    let sb = sprint_b.id;

    let mut card = kanban_domain::Card::new(board_a.id, col_a.id, "Card", 0);
    card.assign_to_sprint(sb, 1, None::<String>, "Planning", chrono::Utc::now());
    let card_id = card.id;
    let original_sprint_id = card.sprint_id;

    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_column(col_a).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(col_b).unwrap();
    tc.store.upsert_sprint(sprint_b).unwrap();
    tc.store.upsert_card(card).unwrap();

    let cmd = MoveCard {
        card_id,
        new_column_id: col_b_id,
        new_position: 0,
    };
    let inverse = cmd.capture_inverse(&tc.store).unwrap();
    assert_eq!(inverse.len(), 2);
    match &inverse[1] {
        Command::Card(CardCommand::RestoreSprintAttachment(r)) => {
            assert_eq!(r.sprint_id, original_sprint_id);
        }
        other => panic!("expected RestoreSprintAttachment, got {other:?}"),
    }
}

#[test]
fn test_move_card_same_board_or_unbound_inverse_is_a_single_move() {
    let tc = TestContext::new();
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let board_a_id = board_a.id;
    let col_a = kanban_domain::Column::new(board_a_id, "Col A", 0);
    let col_a2 = kanban_domain::Column::new(board_a_id, "Col A2", 1);
    let col_a2_id = col_a2.id;
    let sprint_a = kanban_domain::Sprint::new(board_a_id, 1, None, Some("Sprint A"));
    let sa = sprint_a.id;

    let mut bound_card = kanban_domain::Card::new(board_a.id, col_a.id, "Bound", 0);
    bound_card.assign_to_sprint(sa, 1, None::<String>, "Planning", chrono::Utc::now());
    let bound_card_id = bound_card.id;

    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let col_b = kanban_domain::Column::new(board_b.id, "Col", 0);
    let col_b_id = col_b.id;
    let unbound_card = kanban_domain::Card::new(board_a.id, col_a.id, "Unbound", 1);
    let unbound_card_id = unbound_card.id;

    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_column(col_a).unwrap();
    tc.store.upsert_column(col_a2).unwrap();
    tc.store.upsert_sprint(sprint_a).unwrap();
    tc.store.upsert_card(bound_card).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(col_b).unwrap();
    tc.store.upsert_card(unbound_card).unwrap();

    let same_board_move = MoveCard {
        card_id: bound_card_id,
        new_column_id: col_a2_id,
        new_position: 0,
    };
    let inverse = same_board_move.capture_inverse(&tc.store).unwrap();
    assert_eq!(inverse.len(), 1);

    let cross_board_unbound_move = MoveCard {
        card_id: unbound_card_id,
        new_column_id: col_b_id,
        new_position: 0,
    };
    let inverse = cross_board_unbound_move.capture_inverse(&tc.store).unwrap();
    assert_eq!(inverse.len(), 1);
}

#[test]
fn test_move_card_cross_board_of_a_bound_card_invalidation_names_both_columns_and_no_sprint() {
    let tc = TestContext::new();
    let board_a = kanban_domain::Board::new("A", Some("AAA"));
    let a_col = kanban_domain::Column::new(board_a.id, "Col", 0);
    let a_col_id = a_col.id;
    let sprint_a = kanban_domain::Sprint::new(board_a.id, 1, None, Some("Sprint A"));
    let sa = sprint_a.id;

    let board_b = kanban_domain::Board::new("B", Some("BBB"));
    let b_col = kanban_domain::Column::new(board_b.id, "Col", 0);
    let b_col_id = b_col.id;

    let mut card = kanban_domain::Card::new(board_a.id, a_col.id, "Card", 0);
    card.assign_to_sprint(sa, 1, None::<String>, "Planning", chrono::Utc::now());
    let card_id = card.id;

    tc.store.upsert_board(board_a).unwrap();
    tc.store.upsert_column(a_col).unwrap();
    tc.store.upsert_sprint(sprint_a).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store.upsert_board(board_b).unwrap();
    tc.store.upsert_column(b_col).unwrap();

    let forward = vec![Command::Card(CardCommand::Move(MoveCard {
        card_id,
        new_column_id: b_col_id,
        new_position: 0,
    }))];
    let inverse = forward[0].capture_inverse(&tc.store).unwrap();
    let invalidation = kanban_domain::invalidation_from_batch(&forward, &inverse);

    match invalidation {
        kanban_domain::Invalidation::Entities(ids) => {
            assert_eq!(ids.cards, std::collections::HashSet::from([card_id]));
            assert_eq!(
                ids.card_columns,
                std::collections::HashMap::from([(
                    card_id,
                    std::collections::HashSet::from([a_col_id, b_col_id])
                )])
            );
            assert!(ids.sprints.is_empty());
        }
        other => panic!("expected Entities, got {other:?}"),
    }
}

#[test]
fn test_compact_column_positions_makes_sequential() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let column_id = col.id;
    let mut card1 = kanban_domain::Card::new(board.id, column_id, "C1", 0);
    card1.position = 0;
    let mut card2 = kanban_domain::Card::new(board.id, column_id, "C2", 5);
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
