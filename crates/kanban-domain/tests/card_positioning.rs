mod common;
use common::TestContext;
use uuid::Uuid;

use kanban_domain::commands::card::*;
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
    let mut board = kanban_domain::Board::new("Test", Some("TST"));
    let card = kanban_domain::Card::new(&mut board, Uuid::new_v4(), "Card", 0);
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
    let mut board = kanban_domain::Board::new("Test", Some("TST"));
    let src_col = kanban_domain::Column::new(board.id, "Source", 0);
    let mut dst_col = kanban_domain::Column::new(board.id, "Dest", 1);
    dst_col.wip_limit = Some(1);
    let dst_id = dst_col.id;
    let existing = kanban_domain::Card::new(&mut board, dst_id, "Existing", 0);
    let mover = kanban_domain::Card::new(&mut board, src_col.id, "Mover", 0);
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
    let mut board_a = kanban_domain::Board::new("A", Some("AAA"));
    let board_a_id = board_a.id;
    let col_a = kanban_domain::Column::new(board_a_id, "Col", 0);
    let card = kanban_domain::Card::new(&mut board_a, col_a.id, "Card", 0);
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
fn test_compact_column_positions_makes_sequential() {
    let tc = TestContext::new();
    let mut board = kanban_domain::Board::new("B", Some("TST"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let column_id = col.id;
    let mut card1 = kanban_domain::Card::new(&mut board, column_id, "C1", 0);
    card1.position = 0;
    let mut card2 = kanban_domain::Card::new(&mut board, column_id, "C2", 5);
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
