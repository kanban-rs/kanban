mod common;
use common::TestContext;
use uuid::Uuid;

use kanban_domain::commands::*;
use kanban_domain::DataStore;

#[test]
fn test_check_wip_limit_column_not_found_returns_error() {
    let tc = TestContext::new();
    let ctx = tc.as_command_context();
    let result = ctx.check_wip_limit(Uuid::new_v4(), 1, &[]);
    assert!(result.unwrap_err().is_not_found());
}

#[test]
fn test_require_column_missing_returns_not_found() {
    let tc = TestContext::new();
    let ctx = tc.as_command_context();
    let err = ctx.require_column(Uuid::new_v4()).unwrap_err();
    assert!(err.is_not_found());
}

#[test]
fn test_require_column_present_returns_column() {
    let tc = TestContext::new();
    let col = kanban_domain::Column::new(Uuid::new_v4(), "Col", 0);
    let col_id = col.id;
    tc.store.upsert_column(col).unwrap();
    let ctx = tc.as_command_context();
    assert_eq!(ctx.require_column(col_id).unwrap().id, col_id);
}

#[test]
fn test_check_wip_limit_no_limit_always_ok() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", None::<String>);
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let col_id = col.id;
    let card = kanban_domain::Card::new(board.id, col_id, "C", 0);
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();
    let ctx = tc.as_command_context();
    assert!(ctx.check_wip_limit(col_id, 1, &[]).is_ok());
}

#[test]
fn test_check_wip_limit_below_limit_ok() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", None::<String>);
    let mut col = kanban_domain::Column::new(board.id, "Col", 0);
    col.wip_limit = Some(2);
    let col_id = col.id;
    let card = kanban_domain::Card::new(board.id, col_id, "C", 0);
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();
    let ctx = tc.as_command_context();
    assert!(ctx.check_wip_limit(col_id, 1, &[]).is_ok());
}

#[test]
fn test_check_wip_limit_at_limit_returns_error() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", None::<String>);
    let mut col = kanban_domain::Column::new(board.id, "Col", 0);
    col.wip_limit = Some(1);
    let col_id = col.id;
    let card = kanban_domain::Card::new(board.id, col_id, "C", 0);
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();
    let ctx = tc.as_command_context();
    let result = ctx.check_wip_limit(col_id, 1, &[]);
    assert!(result.unwrap_err().is_wip_limit_exceeded());
}

#[test]
fn test_check_wip_limit_exclude_reduces_count() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", None::<String>);
    let mut col = kanban_domain::Column::new(board.id, "Col", 0);
    col.wip_limit = Some(1);
    let col_id = col.id;
    let card = kanban_domain::Card::new(board.id, col_id, "C", 0);
    let card_id = card.id;
    tc.store.upsert_column(col).unwrap();
    tc.store.upsert_card(card).unwrap();
    let ctx = tc.as_command_context();
    assert!(ctx.check_wip_limit(col_id, 1, &[card_id]).is_ok());
}

#[test]
fn test_check_wip_limit_batch_exceeds_limit_returns_error() {
    let tc = TestContext::new();
    let board = kanban_domain::Board::new("B", None::<String>);
    let mut col = kanban_domain::Column::new(board.id, "Col", 0);
    col.wip_limit = Some(1);
    let col_id = col.id;
    tc.store.upsert_board(board).unwrap();
    tc.store.upsert_column(col).unwrap();
    let ctx = tc.as_command_context();
    let result = ctx.check_wip_limit(col_id, 2, &[]);
    assert!(result.unwrap_err().is_wip_limit_exceeded());
}

#[test]
fn test_command_serde_roundtrip_create_board() {
    let cmd = Command::Board(BoardCommand::Create(CreateBoard {
        id: Uuid::new_v4(),
        name: "B".into(),
        card_prefix: None,
        position: 0,
    }));
    let json = serde_json::to_string(&cmd).unwrap();
    let back: Command = serde_json::from_str(&json).unwrap();
    assert!(matches!(back, Command::Board(BoardCommand::Create(_))));
}

#[test]
fn test_command_serde_roundtrip_archive_restore_board() {
    let id = Uuid::new_v4();
    let archive = Command::Board(BoardCommand::Archive(ArchiveBoards { ids: vec![id] }));
    let value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&archive).unwrap()).unwrap();
    assert_eq!(value["domain"], "board");
    assert_eq!(value["action"], "archive");
    let back: Command = serde_json::from_value(value).unwrap();
    assert!(matches!(back, Command::Board(BoardCommand::Archive(_))));

    let restore = Command::Board(BoardCommand::Restore(RestoreBoard { board_id: id }));
    let value: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&restore).unwrap()).unwrap();
    assert_eq!(value["action"], "restore");
    let back: Command = serde_json::from_value(value).unwrap();
    assert!(matches!(back, Command::Board(BoardCommand::Restore(_))));
}

#[test]
fn test_command_serde_tagged_format() {
    let cmd = Command::Card(CardCommand::Move(MoveCard {
        card_id: Uuid::new_v4(),
        new_column_id: Uuid::new_v4(),
        new_position: 0,
    }));
    let json = serde_json::to_string(&cmd).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["domain"], "card");
    assert_eq!(value["action"], "move");
}

#[test]
fn test_command_execute_delegates_to_struct() {
    let tc = TestContext::new();
    let ctx = tc.as_command_context();
    let cmd = Command::Board(BoardCommand::Create(CreateBoard {
        id: Uuid::new_v4(),
        name: "B".into(),
        card_prefix: None,
        position: 0,
    }));
    cmd.execute(&ctx).unwrap();
    assert_eq!(tc.store.list_boards().unwrap().len(), 1);
}

#[test]
fn test_command_description_delegates() {
    let cmd = Command::Board(BoardCommand::Create(CreateBoard {
        id: Uuid::new_v4(),
        name: "My Board".into(),
        card_prefix: None,
        position: 0,
    }));
    assert!(cmd.description().contains("My Board"));
}

#[test]
fn test_command_serde_roundtrip_all_domains() {
    let commands = vec![
        Command::Board(BoardCommand::Delete(DeleteBoard {
            board_id: Uuid::new_v4(),
        })),
        Command::Column(ColumnCommand::Create(CreateColumn {
            id: Uuid::new_v4(),
            board_id: Uuid::new_v4(),
            name: "Col".into(),
            position: 0,
            default_status: None,
        })),
        Command::Card(CardCommand::Delete(DeleteCard {
            card_id: Uuid::new_v4(),
        })),
        Command::Sprint(SprintCommand::Delete(DeleteSprint {
            sprint_id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
        })),
        Command::Dependency(DependencyCommand::RemoveSpawns(RemoveSpawns {
            source: Uuid::new_v4(),
            target: Uuid::new_v4(),
            tolerate_missing: false,
        })),
    ];
    for cmd in commands {
        let json = serde_json::to_string(&cmd).unwrap();
        let _back: Command = serde_json::from_str(&json).unwrap();
    }
}

#[test]
fn test_command_serde_roundtrip_import_entities() {
    let board = kanban_domain::Board::new("Imported", Some("IMP"));
    let col = kanban_domain::Column::new(board.id, "Col", 0);
    let cmd = Command::Board(BoardCommand::Import(ImportEntities {
        boards: vec![board],
        columns: vec![col],
        cards: vec![],
        archived_cards: vec![],
        archived_boards: vec![],
        sprints: vec![],
        graph: Some(kanban_domain::DependencyGraph::new()),
        prefixes: vec![],
    }));
    let json = serde_json::to_string(&cmd).unwrap();
    let back: Command = serde_json::from_str(&json).unwrap();
    match back {
        Command::Board(BoardCommand::Import(ie)) => {
            assert_eq!(ie.boards.len(), 1);
            assert_eq!(ie.columns.len(), 1);
            assert!(ie.graph.is_some());
        }
        _ => panic!("expected ImportEntities"),
    }
}

#[test]
fn test_command_serde_roundtrip_complex_card_commands() {
    let commands = vec![
        Command::Card(CardCommand::Archive(ArchiveCards {
            ids: vec![Uuid::new_v4(), Uuid::new_v4()],
        })),
        Command::Card(CardCommand::AssignToSprint(AssignCardsToSprint {
            ids: vec![Uuid::new_v4()],
            sprint_id: Uuid::new_v4(),
        })),
        Command::Card(CardCommand::Restore(RestoreCard {
            card_id: Uuid::new_v4(),
            column_id: Uuid::new_v4(),
            position: 3,
            timestamp: chrono::Utc::now(),
        })),
        Command::Card(CardCommand::CompactPositions(CompactColumnPositions {
            column_id: Uuid::new_v4(),
        })),
    ];
    for cmd in commands {
        let json = serde_json::to_string(&cmd).unwrap();
        let back: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(std::mem::discriminant(&cmd), std::mem::discriminant(&back));
    }
}
