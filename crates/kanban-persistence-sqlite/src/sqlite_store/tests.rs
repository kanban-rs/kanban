use chrono::Utc;
use kanban_domain::{KanbanError, Snapshot};
use kanban_persistence::{PersistenceMetadata, PersistenceStore, StoreSnapshot};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;

use super::{SqliteStore, SUPPORTED_SCHEMA_VERSION};

fn make_rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn test_sqlitestore_path_is_preserved() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let rt = make_rt();
    let store = rt.block_on(SqliteStore::open(&path)).unwrap();
    assert_eq!(store.path(), path.as_path());
}

#[test]
fn test_sqlitestore_instance_id_persists_across_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let rt = make_rt();
    let id1 = rt.block_on(SqliteStore::open(&path)).unwrap().instance_id();
    let id2 = rt.block_on(SqliteStore::open(&path)).unwrap().instance_id();
    assert_eq!(id1, id2, "instance_id must be stable across reopens");
}

#[test]
fn test_sqlitestore_persistence_save_load_roundtrip() {
    use kanban_domain::{Board, DependencyGraph};
    use kanban_persistence::snapshot_to_json_bytes;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let rt = make_rt();

    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("Test Board", None::<String>);
        let snapshot = Snapshot::from_data(
            vec![board],
            vec![],
            vec![],
            vec![],
            vec![],
            DependencyGraph::new(),
        );
        let data = snapshot_to_json_bytes(&snapshot).unwrap();
        let meta = PersistenceMetadata::new(store.instance_id());
        let store_snap = StoreSnapshot {
            data,
            metadata: meta,
        };

        PersistenceStore::save(&store, store_snap).await.unwrap();

        let (loaded_snap, _meta) = PersistenceStore::load(&store).await.unwrap();
        let loaded: Snapshot = serde_json::from_slice(&loaded_snap.data).unwrap();
        assert_eq!(loaded.boards.len(), 1);
        assert_eq!(loaded.boards[0].name, "Test Board");
    });
}

#[test]
fn test_sqlitestore_exists_returns_true_after_open() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        assert!(PersistenceStore::exists(&store).await);
    });
}

#[test]
fn test_checkpoint_executes_without_error() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        store.checkpoint().await.unwrap();
    });
}

#[test]
fn test_save_checkpoints_wal_file_stays_minimal() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let (snapshot, _) = PersistenceStore::load(&store).await.unwrap();
        PersistenceStore::save(&store, snapshot).await.unwrap();
        let wal_path = path.with_extension("sqlite3-wal");
        if wal_path.exists() {
            assert!(
                wal_path.metadata().unwrap().len() < 32 * 1024,
                "WAL file should be minimal after save+checkpoint"
            );
        }
    });
}

/// Per-kind tables hard-reject metadata outside their respective
/// CHECK constraints. Pin the constraint via a direct insert
/// attempt so any future schema relaxation has to choose
/// whether to drop or update this test.
#[test]
fn test_blocks_edges_rejects_unknown_severity() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("check.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let insert = sqlx::query(
            "INSERT INTO blocks_edges
                (source_id, target_id, severity, created_at, archived_at)
             VALUES (?, ?, 'Catastrophic', ?, NULL)",
        )
        .bind(uuid::Uuid::nil().to_string())
        .bind(uuid::Uuid::from_u128(0x42).to_string())
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(store.pool())
        .await;
        assert!(
            insert.is_err(),
            "CHECK on severity must reject 'Catastrophic'; got {:?}",
            insert
        );
    });
}

#[test]
fn test_relates_edges_rejects_unknown_kind() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("check_relates.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let insert = sqlx::query(
            "INSERT INTO relates_edges
                (source_id, target_id, kind, created_at, archived_at)
             VALUES (?, ?, 'Unknown', ?, NULL)",
        )
        .bind(uuid::Uuid::nil().to_string())
        .bind(uuid::Uuid::from_u128(0x42).to_string())
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(store.pool())
        .await;
        assert!(insert.is_err());
    });
}

/// SqliteStore::open drops the pre-KAN-504 `card_edges` table on
/// first encounter so the per-kind tables can take over.
/// Pre-KAN-504 graph work is not live anywhere, so the data on
/// such a table is dev-only and the drop is safe.
#[test]
fn test_open_drops_legacy_card_edges_table() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("legacy.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        // Pre-seed the legacy table by direct sqlx access without
        // going through SqliteStore::open (the open path would
        // drop it before we could inspect).
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE card_edges (
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                edge_type TEXT NOT NULL,
                direction TEXT NOT NULL,
                weight REAL,
                created_at TEXT NOT NULL,
                archived_at TEXT,
                PRIMARY KEY (source_id, target_id, edge_type)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        drop(pool);

        // Opening triggers the drop + per-kind table creation.
        let store = SqliteStore::open(&path).await.unwrap();
        let has_card_edges: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='card_edges'",
        )
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert!(!has_card_edges, "legacy card_edges table must be dropped");

        for table in ["spawns_edges", "blocks_edges", "relates_edges"] {
            let has: bool = sqlx::query_scalar(&format!(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='{}'",
                table
            ))
            .fetch_one(store.pool())
            .await
            .unwrap();
            assert!(has, "{table} must exist after open");
        }
    });
}

#[test]
fn test_delete_archived_card_orphaned_cards_row_is_still_cleaned_up() {
    use kanban_domain::data_store::DataStore;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let mut board = kanban_domain::Board::new("B", None::<String>);
        let column = kanban_domain::Column::new(board.id, "Col", 0);
        let card = kanban_domain::Card::new(&mut board, column.id, "Task", 0);
        let card_id = card.id;
        let column_id = column.id;
        store.upsert_board(board).unwrap();
        store.upsert_column(column).unwrap();
        store.upsert_card(card.clone()).unwrap();

        // Insert into archived_cards WITHOUT calling delete_card first,
        // leaving an orphaned row in the cards table.
        let archived = kanban_domain::ArchivedCard::new(card, column_id, 0);
        store.insert_archived_card(archived).unwrap();

        store.delete_archived_card(card_id).unwrap();

        assert!(
            store.list_archived_cards().unwrap().is_empty(),
            "card should be gone from archived_cards"
        );
        assert!(
            store.list_all_cards().unwrap().is_empty(),
            "orphaned cards row should also be removed by delete_archived_card"
        );
    });
}

#[test]
fn test_delete_archived_card_removes_from_cards_table() {
    use kanban_domain::data_store::DataStore;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let mut board = kanban_domain::Board::new("B", None::<String>);
        let column = kanban_domain::Column::new(board.id, "Col", 0);
        let card = kanban_domain::Card::new(&mut board, column.id, "Task", 0);
        let card_id = card.id;
        let column_id = column.id;
        store.upsert_board(board).unwrap();
        store.upsert_column(column).unwrap();
        store.upsert_card(card.clone()).unwrap();

        let archived = kanban_domain::ArchivedCard::new(card, column_id, 0);
        store.insert_archived_card(archived).unwrap();
        store.delete_card(card_id).unwrap();

        assert_eq!(store.list_archived_cards().unwrap().len(), 1);

        store.delete_archived_card(card_id).unwrap();

        assert!(
            store.list_archived_cards().unwrap().is_empty(),
            "card should be gone from archived_cards"
        );
        assert!(
            store.list_all_cards().unwrap().is_empty(),
            "card should also be gone from cards table, not restored as active"
        );
        assert!(
            store.get_card(card_id).unwrap().is_none(),
            "get_card should return None for permanently deleted card"
        );
    });
}

#[test]
fn test_read_metadata_sync_reflects_actual_schema_version_from_db_row() {
    // Contract: PersistenceMetadata.format_version is the format the DB
    // is currently at, not whatever the binary's SUPPORTED happens to be.
    // After open(), the two coincide (migrate normalises). We manually
    // UPDATE schema_version below to a value that differs from SUPPORTED
    // and confirm the read reflects the DB, not the const.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("drift.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        sqlx::query("UPDATE metadata SET schema_version = 1 WHERE id = 1")
            .execute(&store.pool)
            .await
            .unwrap();

        let meta = store
            .read_metadata_sync()
            .unwrap()
            .expect("metadata row should exist");
        assert_eq!(
            meta.format_version,
            Some(1),
            "format_version must reflect the DB row (1), not the binary's SUPPORTED ({SUPPORTED_SCHEMA_VERSION})"
        );
    });
}

#[test]
fn test_load_reports_actual_schema_version_in_metadata() {
    use kanban_domain::DependencyGraph;
    use kanban_persistence::snapshot_to_json_bytes;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("load_drift.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        // Seed the row so load() has something to read.
        let snapshot = Snapshot::from_data(
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            DependencyGraph::new(),
        );
        let data = snapshot_to_json_bytes(&snapshot).unwrap();
        PersistenceStore::save(
            &store,
            StoreSnapshot {
                data,
                metadata: PersistenceMetadata::new(store.instance_id()),
            },
        )
        .await
        .unwrap();
        sqlx::query("UPDATE metadata SET schema_version = 1 WHERE id = 1")
            .execute(&store.pool)
            .await
            .unwrap();

        let (_, meta) = PersistenceStore::load(&store).await.unwrap();
        assert_eq!(
            meta.format_version,
            Some(1),
            "load() must report the DB's actual schema_version, not the const"
        );
    });
}

#[test]
fn test_stamp_writer_updates_metadata_row_and_returns_timestamp() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("stamped.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        // Wipe what migrate-on-open already wrote so the assertion is clean.
        sqlx::query("UPDATE metadata SET writer_version = NULL, writer_commit = NULL WHERE id = 1")
            .execute(&store.pool)
            .await
            .unwrap();

        let before = Utc::now();
        let stamped_at = store.stamp_writer().await.unwrap();
        let after = Utc::now();

        assert!(
            stamped_at >= before && stamped_at <= after,
            "returned timestamp must be in [before, after]: {stamped_at:?}"
        );

        let (saved_at_str, wv, wc): (String, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT saved_at, writer_version, writer_commit FROM metadata WHERE id = 1",
        )
        .fetch_one(&store.pool)
        .await
        .unwrap();
        assert_eq!(wv.as_deref(), Some(kanban_core::KANBAN_VERSION));
        assert_eq!(wc.as_deref(), Some(kanban_core::KANBAN_COMMIT));
        assert_eq!(saved_at_str, stamped_at.to_rfc3339());
    });
}

#[test]
fn test_checkpoint_alone_does_not_stamp_writer_fields() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ckpt_only.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        sqlx::query("UPDATE metadata SET writer_version = NULL, writer_commit = NULL WHERE id = 1")
            .execute(&store.pool)
            .await
            .unwrap();

        store.checkpoint().await.unwrap();

        let (wv, wc): (Option<String>, Option<String>) =
            sqlx::query_as("SELECT writer_version, writer_commit FROM metadata WHERE id = 1")
                .fetch_one(&store.pool)
                .await
                .unwrap();
        assert!(
            wv.is_none() && wc.is_none(),
            "checkpoint() must be WAL-only post-split; got wv={wv:?} wc={wc:?}"
        );
    });
}

#[test]
fn test_fresh_db_records_schema_version_2() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert_eq!(version, SUPPORTED_SCHEMA_VERSION);
    });
}

#[test]
fn test_fresh_db_has_writer_version_and_writer_commit_columns() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        for col in ["writer_version", "writer_commit"] {
            let exists: bool = sqlx::query_scalar(&format!(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('metadata') WHERE name = '{col}'"
            ))
            .fetch_one(&store.pool)
            .await
            .unwrap();
            assert!(exists, "metadata.{col} column must exist on fresh DB");
        }
    });
}

#[test]
fn test_save_stamps_writer_version_and_commit_into_metadata_row() {
    use kanban_domain::DependencyGraph;
    use kanban_persistence::snapshot_to_json_bytes;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("stamped.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let snapshot = Snapshot::from_data(
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            DependencyGraph::new(),
        );
        let data = snapshot_to_json_bytes(&snapshot).unwrap();
        let store_snap = StoreSnapshot {
            data,
            metadata: PersistenceMetadata::new(store.instance_id()),
        };
        let returned = PersistenceStore::save(&store, store_snap).await.unwrap();

        assert_eq!(
            returned.writer_version.as_deref(),
            Some(kanban_core::KANBAN_VERSION),
        );
        assert_eq!(
            returned.writer_commit.as_deref(),
            Some(kanban_core::KANBAN_COMMIT),
        );

        let row: (Option<String>, Option<String>) =
            sqlx::query_as("SELECT writer_version, writer_commit FROM metadata WHERE id = 1")
                .fetch_one(&store.pool)
                .await
                .unwrap();
        assert_eq!(row.0.as_deref(), Some(kanban_core::KANBAN_VERSION));
        assert_eq!(row.1.as_deref(), Some(kanban_core::KANBAN_COMMIT));
    });
}

#[test]
fn test_load_returns_writer_stamp_from_metadata_row() {
    use kanban_domain::DependencyGraph;
    use kanban_persistence::snapshot_to_json_bytes;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("loaded.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let snapshot = Snapshot::from_data(
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            DependencyGraph::new(),
        );
        let data = snapshot_to_json_bytes(&snapshot).unwrap();
        PersistenceStore::save(
            &store,
            StoreSnapshot {
                data,
                metadata: PersistenceMetadata::new(store.instance_id()),
            },
        )
        .await
        .unwrap();

        let (_, meta) = PersistenceStore::load(&store).await.unwrap();
        assert_eq!(
            meta.writer_version.as_deref(),
            Some(kanban_core::KANBAN_VERSION),
        );
        assert_eq!(
            meta.writer_commit.as_deref(),
            Some(kanban_core::KANBAN_COMMIT),
        );
    });
}

#[test]
fn test_load_legacy_db_without_stamp_returns_none_writer_fields() {
    // Pre-KAN-522 DB: metadata row exists, schema_version was bumped to
    // current by migrate(), but writer_version/writer_commit are still
    // NULL because no save has happened since the ALTER.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("legacy_load.db");
    let rt = make_rt();
    rt.block_on(async {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE metadata (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                instance_id TEXT NOT NULL,
                saved_at TEXT NOT NULL,
                schema_version INTEGER NOT NULL DEFAULT 1
            );
            INSERT INTO metadata (id, instance_id, saved_at, schema_version)
            VALUES (1, '550e8400-e29b-41d4-a716-446655440000', '2024-01-01T00:00:00Z', 1);",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let store = SqliteStore::open(&path).await.unwrap();
        let (_, meta) = PersistenceStore::load(&store).await.unwrap();
        assert!(meta.writer_version.is_none());
        assert!(meta.writer_commit.is_none());
    });
}

#[test]
fn test_open_rejects_future_schema_version() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("future.db");
    let rt = make_rt();
    rt.block_on(async {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE metadata (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                instance_id TEXT NOT NULL,
                saved_at TEXT NOT NULL,
                schema_version INTEGER NOT NULL DEFAULT 2,
                writer_version TEXT,
                writer_commit TEXT
            );
            INSERT INTO metadata (id, instance_id, saved_at, schema_version)
            VALUES (1, '550e8400-e29b-41d4-a716-446655440000', '2030-01-01T00:00:00Z', 99);",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let err = SqliteStore::open(&path)
            .await
            .err()
            .expect("schema_version 99 must be refused");
        assert!(
            matches!(
                err,
                KanbanError::UnsupportedFutureVersion {
                    file_version: 99,
                    binary_max: SUPPORTED_SCHEMA_VERSION
                }
            ),
            "expected UnsupportedFutureVersion, got: {err:?}"
        );
    });
}

#[test]
fn test_open_alters_in_writer_columns_on_legacy_v1_db() {
    // Simulate a pre-KAN-522 SQLite file: metadata table without the
    // writer_* columns and schema_version = 1. SqliteStore::open must
    // ALTER in the new columns and bump schema_version to 2.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("legacy.db");
    let rt = make_rt();
    rt.block_on(async {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE metadata (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                instance_id TEXT NOT NULL,
                saved_at TEXT NOT NULL,
                schema_version INTEGER NOT NULL DEFAULT 1
            );
            INSERT INTO metadata (id, instance_id, saved_at, schema_version)
            VALUES (1, '550e8400-e29b-41d4-a716-446655440000', '2024-01-01T00:00:00Z', 1);",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let store = SqliteStore::open(&path).await.unwrap();

        for col in ["writer_version", "writer_commit"] {
            let exists: bool = sqlx::query_scalar(&format!(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('metadata') WHERE name = '{col}'"
            ))
            .fetch_one(&store.pool)
            .await
            .unwrap();
            assert!(exists, "metadata.{col} must be ALTERed in on legacy open");
        }

        let bumped: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert_eq!(
            bumped, SUPPORTED_SCHEMA_VERSION,
            "schema_version must be bumped to current on legacy open"
        );
    });
}

#[test]
fn test_empty_sprint_log_status_returns_error() {
    use kanban_domain::data_store::DataStore;
    use kanban_domain::{Board, Card, Column, SprintLog};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("validation.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let mut board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let mut card = Card::new(&mut board, column.id, "Task", 0);
        store.upsert_board(board).unwrap();
        store.upsert_column(column).unwrap();

        let log = SprintLog::new(uuid::Uuid::new_v4(), 1, None::<String>, "");
        card.sprint_logs.push(log);

        let result = store.upsert_card(card);
        assert!(
            result.is_err(),
            "upsert_card must reject a SprintLog with empty status"
        );
    });
}

#[test]
fn test_empty_board_name_returns_error() {
    use kanban_domain::data_store::DataStore;
    use kanban_domain::Board;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("validation.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("", None::<String>);
        let result = store.upsert_board(board);
        assert!(
            result.is_err(),
            "upsert_board must reject a Board with empty name"
        );
    });
}

#[test]
fn test_empty_column_name_returns_error() {
    use kanban_domain::data_store::DataStore;
    use kanban_domain::{Board, Column};
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("validation.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("B", None::<String>);
        let board_id = board.id;
        store.upsert_board(board).unwrap();
        let col = Column::new(board_id, "", 0);
        let result = store.upsert_column(col);
        assert!(
            result.is_err(),
            "upsert_column must reject a Column with empty name"
        );
    });
}

#[test]
fn test_empty_card_title_returns_error() {
    use kanban_domain::data_store::DataStore;
    use kanban_domain::{Board, Card, Column};
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("validation.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let mut board = Board::new("B", None::<String>);
        let col = Column::new(board.id, "Col", 0);
        let col_id = col.id;
        // Card::new borrows &mut board -- call it before upsert_board moves board
        let card = Card::new(&mut board, col_id, "", 0);
        store.upsert_board(board).unwrap();
        store.upsert_column(col).unwrap();
        let result = store.upsert_card(card);
        assert!(
            result.is_err(),
            "upsert_card must reject a Card with empty title"
        );
    });
}
