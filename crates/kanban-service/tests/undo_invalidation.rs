use kanban_backend_memory::InMemoryStore;
use kanban_domain::{CardUpdate, EntityIds, Invalidation, KanbanOperations, KanbanResult};
use kanban_service::KanbanContext;
use std::collections::HashSet;
use std::sync::Arc;

async fn make_ctx() -> KanbanContext {
    KanbanContext::open(
        Arc::new(InMemoryStore::new()),
        kanban_core::AppConfig::default(),
    )
    .await
    .unwrap()
}

fn entities(ctx: &KanbanContext) -> &EntityIds {
    match ctx
        .last_invalidation()
        .expect("expected a recorded invalidation, found None")
    {
        Invalidation::Entities(ids) => ids,
        Invalidation::All => panic!("expected Entities, got All"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_fresh_context_has_no_recorded_invalidation() {
    let ctx = make_ctx().await;
    assert!(ctx.last_invalidation().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_forward_execution_records_the_derived_invalidation() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;

    ctx.update_card(
        card.id,
        CardUpdate {
            title: Some("Renamed".into()),
            ..Default::default()
        },
    )?;

    assert_eq!(entities(&ctx).cards, HashSet::from([card.id]));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_undo_records_the_invalidation_of_the_entry_it_reverses() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card_a = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;
    let card_b = ctx.create_card(board.id, col.id, "B".into(), Default::default())?;

    ctx.update_card(
        card_a.id,
        CardUpdate {
            title: Some("A2".into()),
            ..Default::default()
        },
    )?;
    ctx.update_card(
        card_b.id,
        CardUpdate {
            title: Some("B2".into()),
            ..Default::default()
        },
    )?;

    assert!(ctx.undo()?);
    assert_eq!(entities(&ctx).cards, HashSet::from([card_b.id]));

    assert!(ctx.undo()?);
    let ids = entities(&ctx);
    assert_eq!(ids.cards, HashSet::from([card_a.id]));
    assert!(!ids.cards.contains(&card_b.id));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_redo_records_the_invalidation_of_the_forward_batch() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;

    assert_eq!(
        ctx.last_invalidation(),
        Some(&Invalidation::All),
        "create's inverse is DeleteCard, which is unenumerable"
    );

    assert!(ctx.undo()?);
    assert!(ctx.redo()?);

    let ids = entities(&ctx);
    assert_eq!(ids.cards, HashSet::from([card.id]));
    assert_eq!(ids.boards, HashSet::from([board.id]));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_undo_of_a_batch_whose_inverse_is_unenumerable_records_all() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card_a = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;
    let _card_c = ctx.create_card(board.id, col.id, "C".into(), Default::default())?;

    ctx.update_card(
        card_a.id,
        CardUpdate {
            title: Some("A2".into()),
            ..Default::default()
        },
    )?;
    assert_eq!(entities(&ctx).cards, HashSet::from([card_a.id]));

    assert!(ctx.undo()?);
    assert_eq!(entities(&ctx).cards, HashSet::from([card_a.id]));

    assert!(ctx.undo()?);
    assert_eq!(ctx.last_invalidation(), Some(&Invalidation::All));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_failed_undo_leaves_the_previously_recorded_invalidation_intact() -> KanbanResult<()>
{
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card_a = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;
    let card_b = ctx.create_card(board.id, col.id, "B".into(), Default::default())?;

    ctx.update_card(
        card_a.id,
        CardUpdate {
            title: Some("A2".into()),
            ..Default::default()
        },
    )?;
    ctx.update_card(
        card_b.id,
        CardUpdate {
            title: Some("B2".into()),
            ..Default::default()
        },
    )?;

    assert!(ctx.undo()?);
    assert_eq!(entities(&ctx).cards, HashSet::from([card_b.id]));

    ctx.data_store().delete_card(card_a.id)?;

    assert!(ctx.undo().is_err());
    let ids = entities(&ctx);
    assert_eq!(ids.cards, HashSet::from([card_b.id]));
    assert!(!ids.cards.contains(&card_a.id));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_undo_with_an_empty_stack_records_nothing() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    assert!(!ctx.undo()?);
    assert!(ctx.last_invalidation().is_none());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_redo_with_an_empty_stack_records_nothing() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    assert!(!ctx.redo()?);
    assert!(ctx.last_invalidation().is_none());
    Ok(())
}
