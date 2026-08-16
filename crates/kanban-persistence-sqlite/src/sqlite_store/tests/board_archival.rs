use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;
use super::migration_v2_to_v3::seed_v2_db;
use kanban_domain::{Archived, Board, Column, DataStore};

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
        store.insert_archived_board(Archived::now(id)).unwrap();
        assert!(
            store.list_boards().unwrap().is_empty(),
            "NOT EXISTS filter hides the archived board from live reads"
        );
        assert!(
            store.get_board(id).unwrap().is_some(),
            "get_board is UNFILTERED: the head survives behind the marker"
        );
        let archived = store.list_archived_boards().unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].entity_id, id);
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
fn test_unarchive_board_drops_marker_keeps_row_and_subtree() {
    // KAN-863: RESTORE goes through unarchive_board, which must drop only the
    // marker. Deleting the board row (as delete_archived_board does, for
    // permanent delete) would CASCADE the subtree away.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("unarchive.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("Proj", None::<String>);
        let id = board.id;
        store.upsert_board(board.clone()).unwrap();
        let col = Column::new(id, "Todo", 0);
        let col_id = col.id;
        store.upsert_column(col).unwrap();
        store.insert_archived_board(Archived::now(id)).unwrap();
        assert!(store.list_boards().unwrap().is_empty(), "archived: hidden");

        store.unarchive_board(id).unwrap();

        assert!(
            store.list_archived_boards().unwrap().is_empty(),
            "marker removed"
        );
        assert!(
            store.get_board(id).unwrap().is_some(),
            "board row survived and is live again"
        );
        let cols = store.list_columns_by_board(id).unwrap();
        assert_eq!(cols.len(), 1, "subtree survived unarchive");
        assert_eq!(cols[0].id, col_id);
    });
}

#[test]
fn test_open_migrates_old_db_to_current_schema_with_board_archival_and_backup() {
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
        assert_eq!(version, 11, "old DB migrates to current schema v11");

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
fn test_delete_board_noop_on_archived_keeps_head_and_marker() {
    // KAN-899: pins the SQLite `AND NOT EXISTS board_archival` guard as the canonical
    // spec. A bare `delete_board` on an archived board must be a no-op — head and
    // marker both survive.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("noop.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("Archived", None::<String>);
        let id = board.id;
        store.upsert_board(board).unwrap();
        let col = Column::new(id, "Col", 0);
        let col_id = col.id;
        store.upsert_column(col).unwrap();
        store.insert_archived_board(Archived::now(id)).unwrap();

        // Bare delete_board must be a no-op on an archived board.
        store.delete_board(id).unwrap();

        assert!(
            store.get_board(id).unwrap().is_some(),
            "board head must survive bare delete_board on archived board"
        );
        assert!(
            store.get_archived_board(id).unwrap().is_some(),
            "archived marker must survive bare delete_board"
        );
        let cols = store.list_columns_by_board(id).unwrap();
        assert_eq!(
            cols.len(),
            1,
            "subtree must survive bare delete_board on archived board"
        );
        assert_eq!(cols[0].id, col_id);
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

#[test]
fn test_snapshot_and_apply_round_trip_archived_boards() {
    // Data-loss regression (KAN-860): a SQLite snapshot must carry archived
    // boards, and apply_snapshot must restore them.
    let rt = make_rt();
    rt.block_on(async {
        let dir1 = TempDir::new().unwrap();
        let path1 = dir1.path().join("src.sqlite3");
        let src = SqliteStore::open(&path1).await.unwrap();

        let live = Board::new("Live", None::<String>);
        let archived = Board::new("Archived", None::<String>);
        let archived_id = archived.id;
        src.upsert_board(live).unwrap();
        src.upsert_board(archived.clone()).unwrap();
        src.insert_archived_board(Archived::now(archived_id))
            .unwrap();

        let snap = src.snapshot().unwrap();
        assert_eq!(
            snap.boards.len(),
            2,
            "reference-marker model: .boards carries ALL heads (live + archived)"
        );
        assert_eq!(
            snap.archived_boards.len(),
            1,
            "snapshot must carry the archived-board marker (no data loss)"
        );
        assert_eq!(snap.archived_boards[0].entity_id, archived_id);

        // Apply into a FRESH store — the archived board must round-trip.
        let dir2 = TempDir::new().unwrap();
        let path2 = dir2.path().join("dst.sqlite3");
        let dst = SqliteStore::open(&path2).await.unwrap();
        dst.apply_snapshot(snap).unwrap();

        assert_eq!(dst.list_boards().unwrap().len(), 1);
        let restored = dst.list_archived_boards().unwrap();
        assert_eq!(
            restored.len(),
            1,
            "archived board restored by apply_snapshot"
        );
        assert_eq!(restored[0].entity_id, archived_id);
        assert!(
            dst.get_board(archived_id).unwrap().is_some(),
            "head round-trips (get_board is unfiltered)"
        );
        assert!(
            !dst.list_boards()
                .unwrap()
                .iter()
                .any(|b| b.id == archived_id),
            "archived, so excluded from the live list"
        );
    });
}
