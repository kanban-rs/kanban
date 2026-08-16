use kanban_domain::KanbanError;
use kanban_persistence::PersistenceStore;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;

use super::super::{SqliteStore, SUPPORTED_SCHEMA_VERSION};
use super::make_rt;

#[test]
fn test_sqlitestore_instance_id_persists_across_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let rt = make_rt();
    let id1 = rt.block_on(SqliteStore::open(&path)).unwrap().instance_id();
    let id2 = rt.block_on(SqliteStore::open(&path)).unwrap().instance_id();
    assert_eq!(id1, id2, "instance_id must be stable across reopens");
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
fn test_fresh_db_records_current_schema_version() {
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
        assert_eq!(version, 11, "fresh DB stamps schema_version 11");
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
