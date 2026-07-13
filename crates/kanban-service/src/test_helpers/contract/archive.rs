use super::super::BackendFactory;
use crate::KanbanContext;
use kanban_core::AppConfig;
use kanban_domain::card::CardPriority;
use kanban_domain::{CreateCardOptions, KanbanOperations};
use tempfile::TempDir;

pub async fn test_archive_card_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "To Archive".into(),
            CreateCardOptions {
                description: Some("archived desc".into()),
                priority: Some(CardPriority::High),
                points: Some(3),
                ..Default::default()
            },
        )
        .unwrap();

    ctx.archive_card(card.id).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    assert!(ctx.get_card(card.id).unwrap().is_none());

    let archived = ctx.list_archived_cards().unwrap();
    assert_eq!(archived.len(), 1);

    let ac = &archived[0];
    assert_eq!(ac.entity.id, card.id);
    assert_eq!(ac.entity.title, "To Archive");
    assert_eq!(ac.entity.description.as_deref(), Some("archived desc"));
    assert_eq!(ac.entity.priority, CardPriority::High);
    assert_eq!(ac.entity.points, Some(3));
    assert_eq!(ac.context.original_column_id, col.id);
}

pub async fn test_archive_card_with_sprint_logs_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();
    let sprint = ctx.create_sprint(board.id, None, None).unwrap();
    ctx.activate_sprint(sprint.id, Some(14)).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "Sprint Card".into(),
            CreateCardOptions::default(),
        )
        .unwrap();
    ctx.assign_card_to_sprint(card.id, sprint.id).unwrap();
    ctx.archive_card(card.id).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let archived = ctx.list_archived_cards().unwrap();
    assert_eq!(archived.len(), 1);
    assert!(!archived[0].entity.sprint_logs.is_empty());
}

pub async fn test_restore_archived_card_roundtrip(factory: &BackendFactory) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.store");
    let mut ctx = KanbanContext::open(factory(&path), AppConfig::default())
        .await
        .unwrap();

    let board = ctx.create_board("Board".into(), Some("B".into())).unwrap();
    let col = ctx.create_column(board.id, "Col".into(), None).unwrap();

    let card = ctx
        .create_card(
            board.id,
            col.id,
            "Will Restore".into(),
            CreateCardOptions::default(),
        )
        .unwrap();

    ctx.archive_card(card.id).unwrap();
    ctx.restore_card(card.id, None).unwrap();

    ctx.save().await.unwrap();
    let ctx = KanbanContext::open_deferred(factory(&path), AppConfig::default());

    let c = ctx.get_card(card.id).unwrap().unwrap();
    assert_eq!(c.title, "Will Restore");
    assert!(ctx.list_archived_cards().unwrap().is_empty());
}
