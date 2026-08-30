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

fn entities(inv: Invalidation) -> EntityIds {
    match inv {
        Invalidation::Entities(ids) => ids,
        Invalidation::All => panic!("expected Entities, got All"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_forward_execution_records_the_derived_invalidation() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;

    let (_card, inv) = ctx.update_card_impl(
        card.id,
        CardUpdate {
            title: Some("Renamed".into()),
            ..Default::default()
        },
    )?;

    assert_eq!(entities(inv).cards, HashSet::from([card.id]));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_undo_records_the_invalidation_of_the_entry_it_reverses() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card_a = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;
    let card_b = ctx.create_card(board.id, col.id, "B".into(), Default::default())?;

    let _ = ctx.update_card_impl(
        card_a.id,
        CardUpdate {
            title: Some("A2".into()),
            ..Default::default()
        },
    )?;
    let _ = ctx.update_card_impl(
        card_b.id,
        CardUpdate {
            title: Some("B2".into()),
            ..Default::default()
        },
    )?;

    let inv = ctx.undo()?.expect("undo applied");
    assert_eq!(entities(inv).cards, HashSet::from([card_b.id]));

    let inv = ctx.undo()?.expect("undo applied");
    let ids = entities(inv);
    assert_eq!(ids.cards, HashSet::from([card_a.id]));
    assert!(!ids.cards.contains(&card_b.id));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_redo_records_the_invalidation_of_the_forward_batch() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let (card, create_inv) =
        ctx.create_card_impl(board.id, col.id, "A".into(), Default::default())?;

    assert_eq!(
        create_inv,
        Invalidation::All,
        "create's inverse is DeleteCard, which is unenumerable"
    );

    let _ = ctx.undo()?.expect("undo applied");
    let inv = ctx.redo()?.expect("redo applied");

    let ids = entities(inv);
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

    let (_card, update_inv) = ctx.update_card_impl(
        card_a.id,
        CardUpdate {
            title: Some("A2".into()),
            ..Default::default()
        },
    )?;
    assert_eq!(entities(update_inv).cards, HashSet::from([card_a.id]));

    let inv = ctx.undo()?.expect("undo applied");
    assert_eq!(entities(inv).cards, HashSet::from([card_a.id]));

    let inv = ctx.undo()?.expect("undo applied");
    assert_eq!(inv, Invalidation::All);

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_failed_undo_returns_an_error_and_leaves_the_entry_retryable() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board = ctx.create_board("B".into(), None)?;
    let col = ctx.create_column(board.id, "C".into(), None)?;
    let card_a = ctx.create_card(board.id, col.id, "A".into(), Default::default())?;
    let card_b = ctx.create_card(board.id, col.id, "B".into(), Default::default())?;

    let _ = ctx.update_card_impl(
        card_a.id,
        CardUpdate {
            title: Some("A2".into()),
            ..Default::default()
        },
    )?;
    let _ = ctx.update_card_impl(
        card_b.id,
        CardUpdate {
            title: Some("B2".into()),
            ..Default::default()
        },
    )?;

    let _ = ctx.undo()?.expect("undo applied");

    ctx.data_store().delete_card(card_a.id)?;

    let depth_before = ctx.undo_depth();
    assert!(ctx.undo().is_err());
    assert_eq!(ctx.undo_depth(), depth_before);
    assert!(ctx.can_undo());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_undo_with_an_empty_stack_records_nothing() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    assert!(ctx.undo()?.is_none());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_redo_with_an_empty_stack_records_nothing() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    assert!(ctx.redo()?.is_none());
    Ok(())
}
