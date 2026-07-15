//! Board archiving through the real SQLite backend (C5): `SqliteStore`
//! board_archival marker table + `SqliteBackend` forwards + the collection-move
//! commands. These are service-level (through the backend) on purpose — a
//! store-only test would miss the `SqliteBackend` forwards and the
//! `RestoreBoard` command ordering.

use kanban_domain::{KanbanOperations, KanbanResult};
use kanban_service::{open_context, AppConfig, KanbanContext};
use tempfile::TempDir;
use uuid::Uuid;

async fn open(path: &std::path::Path) -> KanbanContext {
    open_context(path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap()
}

fn seed(ctx: &mut KanbanContext) -> KanbanResult<Uuid> {
    let b = ctx.create_board("Proj".into(), None)?;
    let col = ctx.create_column(b.id, "Todo".into(), None)?;
    ctx.create_card(b.id, col.id, "Task".into(), Default::default())?;
    Ok(b.id)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_hides_from_live_lists_and_persists_across_reload() -> KanbanResult<()> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("archive.sqlite3");

    let board_id = {
        let mut ctx = open(&path).await;
        let board_id = seed(&mut ctx)?;
        ctx.archive_board(board_id)?;

        assert!(ctx.boards()?.is_empty(), "archived board left the live set");
        let archived = ctx.list_archived_boards()?;
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].entity.id, board_id);
        assert_eq!(ctx.list_all_columns()?.len(), 1, "subtree stays in place");
        assert_eq!(ctx.list_all_cards()?.len(), 1);
        ctx.save().await?;
        board_id
    };

    // Fresh open on the same file: the marker persisted.
    let ctx = open(&path).await;
    assert!(ctx.boards()?.is_empty(), "live boards empty after reload");
    let archived = ctx.list_archived_boards()?;
    assert_eq!(archived.len(), 1, "archived board persisted");
    assert_eq!(archived[0].entity.id, board_id);
    assert_eq!(ctx.list_all_columns()?.len(), 1, "subtree persisted");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archive_undo_and_restore_return_board_to_live() -> KanbanResult<()> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("undo.sqlite3");
    let mut ctx = open(&path).await;
    let board_id = seed(&mut ctx)?;

    ctx.archive_board(board_id)?;
    assert!(ctx.boards()?.is_empty());
    assert!(ctx.undo()?);
    assert_eq!(ctx.boards()?.len(), 1, "undo returned the board to live");
    assert!(ctx.list_archived_boards()?.is_empty());

    ctx.archive_board(board_id)?;
    ctx.restore_board(board_id)?;
    assert_eq!(ctx.boards()?.len(), 1, "restore returned the board to live");
    assert!(ctx.list_archived_boards()?.is_empty());
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_delete_works_on_archived_board_and_undo_restores_as_archived() -> KanbanResult<()> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("delete.sqlite3");
    let mut ctx = open(&path).await;
    let board_id = seed(&mut ctx)?;
    ctx.archive_board(board_id)?;

    ctx.delete_board(board_id)?;
    assert!(ctx.list_archived_boards()?.is_empty(), "board record gone");
    assert!(ctx.boards()?.is_empty());
    assert!(ctx.list_all_columns()?.is_empty(), "subtree cascaded");
    assert!(ctx.list_all_cards()?.is_empty());

    assert!(ctx.undo()?);
    assert!(ctx.boards()?.is_empty(), "not restored to the live set");
    let archived = ctx.list_archived_boards()?;
    assert_eq!(archived.len(), 1, "restored as archived");
    assert_eq!(archived[0].entity.id, board_id);
    assert_eq!(ctx.list_all_columns()?.len(), 1, "subtree restored");
    Ok(())
}
