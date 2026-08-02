//! Board archiving through the real SQLite backend (C5): `SqliteStore`
//! board_archival marker table + `SqliteBackend` forwards + the collection-move
//! commands. These are service-level (through the backend) on purpose — a
//! store-only test would miss the `SqliteBackend` forwards and the
//! `RestoreBoard` command ordering.

use kanban_domain::{KanbanOperations, KanbanResult};
use kanban_service::{AppConfig, KanbanContext};
use tempfile::TempDir;
use uuid::Uuid;

async fn open_context(locator: &str, config: AppConfig) -> KanbanResult<KanbanContext> {
    let mut config = config;
    let mut stores = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    stores.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    stores.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    let sm = kanban_service::StoreManager::new(stores, backends);
    sm.sync_backend_with_file(locator, &mut config);
    let backend = sm.make_backend(locator, &config).await?;
    KanbanContext::open(backend, config).await
}

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
        assert_eq!(archived[0].entity_id, board_id);
        assert!(
            ctx.list_all_columns()?.is_empty(),
            "subtree hidden from live view (C3b)"
        );
        assert_eq!(ctx.snapshot()?.columns.len(), 1, "subtree stays in place");
        ctx.save().await?;
        board_id
    };

    // Fresh open on the same file: the marker persisted.
    let ctx = open(&path).await;
    assert!(ctx.boards()?.is_empty(), "live boards empty after reload");
    let archived = ctx.list_archived_boards()?;
    assert_eq!(archived.len(), 1, "archived board persisted");
    assert_eq!(archived[0].entity_id, board_id);
    assert!(
        ctx.list_all_columns()?.is_empty(),
        "subtree hidden from live view"
    );
    assert_eq!(ctx.snapshot()?.columns.len(), 1, "subtree persisted");
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
    // The subtree must survive undo-of-archive, not just the head (KAN-863).
    assert_eq!(
        ctx.list_all_columns()?.len(),
        1,
        "undo preserved the subtree"
    );
    assert_eq!(ctx.list_all_cards()?.len(), 1);

    ctx.archive_board(board_id)?;
    ctx.restore_board(board_id)?;
    assert_eq!(ctx.boards()?.len(), 1, "restore returned the board to live");
    assert!(ctx.list_archived_boards()?.is_empty());
    // ...and survive an explicit restore too.
    assert_eq!(
        ctx.list_all_columns()?.len(),
        1,
        "restore preserved the subtree"
    );
    assert_eq!(ctx.list_all_cards()?.len(), 1);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_restore_preserves_board_subtree() -> KanbanResult<()> {
    // KAN-863: restoring an archived board on SQLite must NOT destroy its
    // subtree. `delete_archived_board` used to DELETE the shared board row,
    // and `columns/cards/sprints REFERENCES boards ON DELETE CASCADE` wiped the
    // subtree; `RestoreBoard` then re-inserted only the head.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("restore_subtree.sqlite3");
    let mut ctx = open(&path).await;

    let b = ctx.create_board("Proj".into(), None)?;
    let col = ctx.create_column(b.id, "Todo".into(), None)?;
    let card = ctx.create_card(b.id, col.id, "Task".into(), Default::default())?;
    let sprint = ctx.create_sprint(b.id, None, None)?;

    ctx.archive_board(b.id)?;
    ctx.restore_board(b.id)?;

    assert_eq!(ctx.boards()?.len(), 1, "board head restored");
    let cols = ctx.list_columns(b.id)?;
    assert_eq!(cols.len(), 1, "column survived restore");
    assert_eq!(cols[0].id, col.id);
    assert!(
        ctx.list_all_cards()?.iter().any(|c| c.id == card.id),
        "card survived restore"
    );
    assert!(
        ctx.list_sprints(b.id)?.iter().any(|s| s.id == sprint.id),
        "sprint survived restore"
    );
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
    assert_eq!(archived[0].entity_id, board_id);
    assert!(
        ctx.list_all_columns()?.is_empty(),
        "still archived: hidden from live"
    );
    assert_eq!(ctx.snapshot()?.columns.len(), 1, "subtree restored");
    Ok(())
}
