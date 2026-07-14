//! Board archiving through the real JSON file backend (C4). Verifies the
//! `JsonDataStore` archived-board forwards persist and reload correctly, and
//! that legacy files without the `archived_boards` key still load.

use kanban_domain::{DataStore, KanbanOperations, KanbanResult};
use kanban_persistence_json::JsonFileStore;
use kanban_service::{json_backend::JsonDataStore, AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;

fn make_json_backend(path: &std::path::Path) -> Arc<dyn KanbanBackend> {
    Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))))
}

#[tokio::test(flavor = "multi_thread")]
async fn test_archived_board_survives_json_file_roundtrip() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("archive.json");

    // Session 1: create a board + subtree, archive it, save to disk.
    let board_id = {
        let mut ctx = KanbanContext::open(make_json_backend(&path), AppConfig::default()).await?;
        let board = ctx.create_board("Proj".into(), None)?;
        ctx.create_column(board.id, "Todo".into(), None)?;
        ctx.archive_board(board.id)?;

        assert!(ctx.boards()?.is_empty(), "archived board left the live set");
        assert_eq!(ctx.list_archived_boards()?.len(), 1);
        ctx.save().await?;
        board.id
    };

    // The on-disk JSON envelope must actually carry archived_boards.
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(
        raw.contains("archived_boards"),
        "JSON envelope must carry archived_boards"
    );

    // Session 2: fresh independent store at the same path.
    let jds = JsonDataStore::new(Arc::new(JsonFileStore::new(&path)));
    assert!(
        jds.list_boards()?.is_empty(),
        "reloaded live boards must be empty"
    );
    let archived = jds.list_archived_boards()?;
    assert_eq!(
        archived.len(),
        1,
        "archived board must survive the JSON file round-trip"
    );
    assert_eq!(archived[0].entity.id, board_id);
    assert_eq!(jds.list_all_columns()?.len(), 1, "subtree column survived");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn test_legacy_json_without_archived_boards_loads_empty() -> KanbanResult<()> {
    let dir = tempdir().unwrap();
    let path = dir.path().join("legacy.json");

    {
        let mut ctx = KanbanContext::open(make_json_backend(&path), AppConfig::default()).await?;
        ctx.create_board("Legacy".into(), None)?;
        ctx.save().await?;
    }

    // Remove the archived_boards key from the persisted data to simulate a
    // pre-C4 file (robust to serde field order/formatting).
    let raw = std::fs::read_to_string(&path).unwrap();
    let mut envelope: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let removed = envelope["data"]
        .as_object_mut()
        .and_then(|data| data.remove("archived_boards"))
        .is_some();
    assert!(
        removed,
        "test setup expected an archived_boards key to remove"
    );
    std::fs::write(&path, serde_json::to_string(&envelope).unwrap()).unwrap();

    let jds = JsonDataStore::new(Arc::new(JsonFileStore::new(&path)));
    assert_eq!(jds.list_boards()?.len(), 1);
    assert!(
        jds.list_archived_boards()?.is_empty(),
        "missing archived_boards key must default to empty"
    );
    Ok(())
}
