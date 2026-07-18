use kanban_domain::InMemoryStore;
use kanban_domain::{KanbanOperations, KanbanResult};
use kanban_service::KanbanContext;
use std::sync::Arc;
use uuid::Uuid;

async fn make_ctx() -> KanbanContext {
    KanbanContext::open(
        Arc::new(InMemoryStore::new()),
        kanban_core::AppConfig::default(),
    )
    .await
    .unwrap()
}

/// Create a board with one column and one card; return its id.
fn seed(ctx: &mut KanbanContext) -> KanbanResult<Uuid> {
    ctx.create_board("B".into(), None)?;
    let board_id = ctx.boards()?[0].id;
    ctx.create_column(board_id, "Todo".into(), None)?;
    let col_id = ctx.columns()?[0].id;
    ctx.create_card(board_id, col_id, "Task".into(), Default::default())?;
    Ok(board_id)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_board_hides_from_boards_and_lists_in_archived() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board_id = seed(&mut ctx)?;

    ctx.archive_board(board_id)?;

    assert!(
        ctx.boards()?.is_empty(),
        "archived board leaves the live set"
    );
    let archived = ctx.list_archived_boards()?;
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].entity.id, board_id);
    // Subtree stays in place — hidden from live cross-board views (C3b) but
    // preserved in the snapshot (fidelity).
    assert!(
        ctx.list_all_columns()?.is_empty(),
        "archived board's columns hidden from live view"
    );
    let snap = ctx.snapshot()?;
    assert_eq!(snap.columns.len(), 1, "subtree preserved in snapshot");
    assert_eq!(snap.cards.len(), 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_board_is_undoable() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board_id = seed(&mut ctx)?;
    ctx.archive_board(board_id)?;
    assert!(ctx.boards()?.is_empty());

    assert!(ctx.undo()?);
    assert_eq!(ctx.boards()?.len(), 1, "undo returns the board to live");
    assert!(ctx.list_archived_boards()?.is_empty());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_board_returns_it_to_live() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board_id = seed(&mut ctx)?;
    ctx.archive_board(board_id)?;

    ctx.restore_board(board_id)?;

    assert_eq!(ctx.boards()?.len(), 1);
    assert!(ctx.list_archived_boards()?.is_empty());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_works_on_archived_board_and_removes_subtree() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board_id = seed(&mut ctx)?;
    ctx.archive_board(board_id)?;

    // A single collection-agnostic delete works on the archived board.
    ctx.delete_board(board_id)?;

    assert!(ctx.list_archived_boards()?.is_empty(), "board record gone");
    assert!(ctx.boards()?.is_empty());
    assert!(ctx.list_all_columns()?.is_empty(), "subtree cascaded");
    assert!(ctx.list_all_cards()?.is_empty(), "subtree cascaded");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_archived_board_undo_restores_as_archived() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board_id = seed(&mut ctx)?;
    ctx.archive_board(board_id)?;
    ctx.delete_board(board_id)?;
    assert!(ctx.list_archived_boards()?.is_empty());

    assert!(ctx.undo()?);
    // Restored AS archived (not live), with its subtree.
    assert!(ctx.boards()?.is_empty(), "not restored to the live set");
    let archived = ctx.list_archived_boards()?;
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].entity.id, board_id);
    assert!(
        ctx.list_all_columns()?.is_empty(),
        "still archived: subtree hidden from live view"
    );
    let snap = ctx.snapshot()?;
    assert_eq!(snap.columns.len(), 1, "subtree preserved");
    assert_eq!(snap.cards.len(), 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_missing_board_returns_not_found() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let result = ctx.archive_board(Uuid::new_v4());
    assert!(result.is_err());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_non_archived_board_returns_not_found() -> KanbanResult<()> {
    let mut ctx = make_ctx().await;
    let board_id = seed(&mut ctx)?;
    // Board is live, not archived.
    let result = ctx.restore_board(board_id);
    assert!(result.is_err());
    Ok(())
}
