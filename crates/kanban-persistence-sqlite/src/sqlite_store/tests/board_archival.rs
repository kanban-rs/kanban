use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;
use super::migration_v2_to_v3::seed_v2_db;
use kanban_domain::{Archived, Board, DataStore};

#[test]
fn test_archived_board_roundtrip_through_board_archival_table() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("archival.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("Proj", None::<String>);
        let id = board.id;
        store.upsert_board(board.clone()).unwrap();
        assert_eq!(store.list_boards().unwrap().len(), 1);

        // Archive: board stays in `boards`, a marker row appears; live reads
        // exclude it, archived reads reconstitute it.
        store.insert_archived_board(Archived::now(board)).unwrap();
        assert!(
            store.list_boards().unwrap().is_empty(),
            "NOT EXISTS filter hides the archived board from live reads"
        );
        assert!(store.get_board(id).unwrap().is_none());
        let archived = store.list_archived_boards().unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].entity.id, id);
        assert!(store.get_archived_board(id).unwrap().is_some());

        // delete_board is a NOT-EXISTS no-op on an archived board.
        store.delete_board(id).unwrap();
        assert_eq!(
            store.list_archived_boards().unwrap().len(),
            1,
            "delete_board no-op"
        );

        // delete_archived_board removes marker + row (permanent).
        store.delete_archived_board(id).unwrap();
        assert!(store.list_archived_boards().unwrap().is_empty());
        assert!(store.get_board(id).unwrap().is_none());
    });
}

#[test]
fn test_open_migrates_old_db_to_v4_with_board_archival_and_backup() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("old.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v2_db(&path, Uuid::nil()).await;

        let store = SqliteStore::open(&path).await.unwrap();

        // Bumped to the current version.
        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(version, 4, "old DB migrates to current schema v4");

        // board_archival table exists after the schema step.
        let has_table: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='board_archival'",
        )
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert!(has_table, "board_archival table created on open");

        // The pre-migration durable backup was written (from v2).
        assert!(
            SqliteStore::backup_path_for(&path, 2).exists(),
            "an upgrade from an older schema writes a pre-migration backup"
        );
    });
}

#[test]
fn test_delete_archived_board_is_noop_on_a_live_board() {
    // Parity with the in-memory store: delete_archived_board must only remove an
    // ARCHIVED board, never a live one.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("live.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("Live", None::<String>);
        let id = board.id;
        store.upsert_board(board).unwrap();

        store.delete_archived_board(id).unwrap();

        assert!(
            store.get_board(id).unwrap().is_some(),
            "a live board must survive delete_archived_board"
        );
        assert_eq!(store.list_boards().unwrap().len(), 1);
    });
}
