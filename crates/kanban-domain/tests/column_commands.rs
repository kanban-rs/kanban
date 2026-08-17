mod common;
use common::TestContext;
use uuid::Uuid;

use kanban_domain::commands::column_commands::*;
use kanban_domain::commands::Command;
use kanban_domain::*;

#[test]
fn test_update_column_not_found_returns_error() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = UpdateColumn {
        column_id: Uuid::new_v4(),
        updates: ColumnUpdate::default(),
    };
    let result = cmd.execute(&context);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_create_column_command_funnels_through_factory_with_injected_id() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let board_id = Uuid::new_v4();
    let cmd = CreateColumn {
        id,
        board_id,
        name: "Factory Funnel".to_string(),
        position: 3,
        default_status: None,
    };
    cmd.execute(&context).unwrap();

    let column = tc.store.get_column(id).unwrap().unwrap();
    assert_eq!(column.id, id);
    assert_eq!(column.board_id, board_id);
    assert_eq!(column.name, "Factory Funnel");
    // Server-managed position applied verbatim by the command.
    assert_eq!(column.position, 3);
    // The factory uses a single clock for both timestamps.
    assert_eq!(column.created_at, column.updated_at);
}

#[test]
fn test_delete_column_with_archived_cards_now_succeeds() {
    // Under the D2 first-class model, an archived card's `original_column_id`
    // is historical (not a live FK), so a column that only holds archived
    // cards can be deleted. Board-scoped cleanup handles the archived record.
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", Some("TST"));
    let board_id = board.id;
    let col = kanban_domain::Column::new(board_id, "C", 0);
    let col_id = col.id;
    let card = kanban_domain::Card::new(board.id, col_id, "archived", 0);
    let card_id = card.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();
    tc.store
        .insert_archived_card(kanban_domain::ArchivedCard::new(card_id, board_id))
        .unwrap();

    let context = tc.as_command_context();
    let cmd = DeleteColumn { column_id: col_id };
    cmd.execute(&context).unwrap();

    assert!(
        tc.store.get_column(col_id).unwrap().is_none(),
        "column with only archived cards must be deletable"
    );
    assert_eq!(
        tc.store.list_archived_cards().unwrap().len(),
        1,
        "the archived record survives the column deletion (dangling original_column_id)"
    );
}

#[test]
fn test_create_column_command_rejects_negative_position_via_factory_validation() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let cmd = CreateColumn {
        id: Uuid::new_v4(),
        board_id: Uuid::new_v4(),
        name: "Bad".to_string(),
        position: -1,
        default_status: None,
    };
    // The legacy `Column::new` + id-overwrite path silently accepts a
    // negative position; routing through `Column::create` enforces the
    // non-negativity invariant, so this must now be a validation error.
    let err = cmd.execute(&context).unwrap_err();
    assert!(err.is_validation());
}

#[test]
fn test_update_column_execute_rejects_negative_position() {
    let tc = TestContext::new();
    let context = tc.as_command_context();

    let board_id = Uuid::new_v4();
    let column = kanban_domain::Column::new(board_id, "Test Column", 0);
    let column_id = column.id;
    let original_position = column.position;
    tc.store.upsert_column(column).unwrap();

    let cmd = UpdateColumn {
        column_id,
        updates: ColumnUpdate {
            position: Some(-1),
            ..Default::default()
        },
    };

    let err = cmd.execute(&context).unwrap_err();
    assert!(err.is_validation());

    let column = tc.store.get_column(column_id).unwrap().unwrap();
    assert_eq!(
        column.position, original_position,
        "execute must reject before mutating"
    );
}

#[test]
fn test_deleting_a_completion_column_needs_no_board_update() {
    let tc = TestContext::new();
    let board_id = Uuid::new_v4();
    let mut column = Column::new(board_id, "Done", 0);
    column.default_status = Some(CardStatus::Done);
    let column_id = column.id;
    tc.store.upsert_column(column).unwrap();

    let inverse = DeleteColumn { column_id }
        .capture_inverse(&tc.store)
        .unwrap();

    assert!(
        !inverse.iter().any(|cmd| matches!(cmd, Command::Board(_))),
        "the undo of a completion column's delete must contain no board command, \
         since default_status alone carries completion state now"
    );
}

#[test]
fn test_undo_of_completion_column_delete_restores_its_default_status() {
    let tc = TestContext::new();
    let context = tc.as_command_context();
    let board_id = Uuid::new_v4();
    let mut column = Column::new(board_id, "Done", 0);
    column.default_status = Some(CardStatus::Done);
    let column_id = column.id;
    tc.store.upsert_column(column).unwrap();

    let inverse = DeleteColumn { column_id }
        .capture_inverse(&tc.store)
        .unwrap();
    DeleteColumn { column_id }.execute(&context).unwrap();
    assert!(tc.store.get_column(column_id).unwrap().is_none());

    for cmd in inverse {
        cmd.execute(&context).unwrap();
    }

    let restored = tc.store.get_column(column_id).unwrap().unwrap();
    assert_eq!(restored.default_status, Some(CardStatus::Done));
}

#[test]
fn test_no_dangling_completion_reference_after_column_delete() {
    // With completion carried solely by `column.default_status`, a deleted
    // column takes its completion membership with it: nothing else can
    // reference it, so there is nothing left to dangle.
    let tc = TestContext::new();
    let board_id = Uuid::new_v4();
    let mut column = Column::new(board_id, "Done", 0);
    column.default_status = Some(CardStatus::Done);
    let column_id = column.id;
    tc.store.upsert_column(column).unwrap();

    DeleteColumn { column_id }
        .execute(&tc.as_command_context())
        .unwrap();

    assert!(tc.store.get_column(column_id).unwrap().is_none());
}
