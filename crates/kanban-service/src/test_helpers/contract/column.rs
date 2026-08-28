use super::super::BackendFactory;
use crate::KanbanContext;
use kanban_core::AppConfig;
use kanban_domain::{CardStatus, ColumnUpdate, FieldUpdate, KanbanOperations};
use tempfile::TempDir;

pub async fn test_column_all_fields_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Backlog".into(), None).unwrap();

    ctx.update_column(
        col.id,
        ColumnUpdate {
            name: Some("In Progress".into()),
            position: Some(3),
            wip_limit: FieldUpdate::Set(5),
            default_status: None,
        },
    )
    .unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let c = ctx.get_column(col.id).unwrap().unwrap();
    assert_eq!(c.name, "In Progress");
    assert_eq!(c.board_id, board.id);
    assert_eq!(c.position, 3);
    assert_eq!(c.wip_limit, Some(5));
    assert!(c.created_at <= c.updated_at);
}

pub async fn test_column_without_wip_limit_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Open".into(), None).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let c = ctx.get_column(col.id).unwrap().unwrap();
    assert_eq!(c.name, "Open");
    assert!(c.wip_limit.is_none());
}

pub async fn test_column_default_status_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col = ctx.create_column(board.id, "Doing".into(), None).unwrap();
    assert_eq!(col.default_status, None);

    ctx.update_column(
        col.id,
        ColumnUpdate {
            name: None,
            position: None,
            wip_limit: FieldUpdate::NoChange,
            default_status: Some(Some(CardStatus::InProgress)),
        },
    )
    .unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let c = ctx.get_column(col.id).unwrap().unwrap();
    assert_eq!(c.default_status, Some(CardStatus::InProgress));
}

pub async fn test_multiple_columns_preserve_positions(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), None).unwrap();
    let col1 = ctx.create_column(board.id, "Todo".into(), Some(0)).unwrap();
    let col2 = ctx
        .create_column(board.id, "In Progress".into(), Some(1))
        .unwrap();
    let col3 = ctx.create_column(board.id, "Done".into(), Some(2)).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let cols = ctx.list_columns(board.id).unwrap();
    assert_eq!(cols.len(), 3);
    assert_eq!(cols.iter().find(|c| c.id == col1.id).unwrap().position, 0);
    assert_eq!(cols.iter().find(|c| c.id == col2.id).unwrap().position, 1);
    assert_eq!(cols.iter().find(|c| c.id == col3.id).unwrap().position, 2);
}
