use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;
use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::migration_v2_to_v3::open_seeded_pool;
use super::{completion_rows, make_rt};

/// Seed a schema_version-5 shaped DB: boards still carries the legacy
/// `completion_column_id` column (open_seeded_pool's boards shape), and the
/// `board_completion_columns` join table does not exist yet. The remaining
/// tables are created by SCHEMA on open. Returns (board_id, column_id).
async fn seed_v5_db(path: &Path) -> (Uuid, Uuid) {
    let (pool, board_id, column_id) = open_seeded_pool(path, 5).await;
    pool.close().await;
    (board_id, column_id)
}

async fn add_column(
    pool: &sqlx::SqlitePool,
    id: Uuid,
    board_id: Uuid,
    position: i64,
    created_at: &str,
) {
    sqlx::query(
        "INSERT INTO columns (id, board_id, name, position, created_at, updated_at)
         VALUES (?, ?, 'Col', ?, ?, ?)",
    )
    .bind(id.to_string())
    .bind(board_id.to_string())
    .bind(position)
    .bind(created_at)
    .bind(created_at)
    .execute(pool)
    .await
    .unwrap();
}

// These tests exercise `migrate_v5_to_v6_completion_columns` directly against
// a still-open pool, rather than through `SqliteStore::open`: by the time
// `open()` returns, the full migrate chain has already run
// `migrate_v8_to_v9_drop_completion_columns`, which drops the join table this
// step creates. Calling the step in isolation is the only way left to observe
// its output.

#[test]
fn test_sqlite_migration_v5_to_v6_creates_join_table() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v5.db");
    let rt = make_rt();
    rt.block_on(async {
        let (pool, _board_id, _column_id) = open_seeded_pool(&path, 5).await;

        SqliteStore::migrate_v5_to_v6_completion_columns(&pool)
            .await
            .unwrap();

        let has_table: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master
             WHERE type='table' AND name='board_completion_columns'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            has_table,
            "board_completion_columns must exist right after the v5 -> v6 step"
        );
    });
}

#[test]
fn test_sqlite_migration_v5_to_v6_backfills_from_existing_completion_column_id() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v5.db");
    let rt = make_rt();
    rt.block_on(async {
        let (pool, board_id, first_column) = open_seeded_pool(&path, 5).await;
        let last_column = Uuid::new_v4();
        add_column(&pool, last_column, board_id, 1, "2024-01-02T00:00:00Z").await;
        sqlx::query("UPDATE boards SET completion_column_id = ? WHERE id = ?")
            .bind(first_column.to_string())
            .bind(board_id.to_string())
            .execute(&pool)
            .await
            .unwrap();

        SqliteStore::migrate_v5_to_v6_completion_columns(&pool)
            .await
            .unwrap();

        assert_eq!(
            completion_rows(&pool, board_id).await,
            vec![first_column.to_string()],
            "a valid legacy completion_column_id must be carried forward, not the last column"
        );
    });
}

#[test]
fn test_sqlite_migration_v5_to_v6_backfills_last_column_when_null() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v5.db");
    let rt = make_rt();
    rt.block_on(async {
        let (pool, board_id, _first_column) = open_seeded_pool(&path, 5).await;
        let last_column = Uuid::new_v4();
        add_column(&pool, last_column, board_id, 7, "2024-01-02T00:00:00Z").await;

        SqliteStore::migrate_v5_to_v6_completion_columns(&pool)
            .await
            .unwrap();

        assert_eq!(
            completion_rows(&pool, board_id).await,
            vec![last_column.to_string()],
            "null legacy id: backfill with the board's last column by position"
        );
    });
}

#[test]
fn test_sqlite_migration_v5_to_v6_dangling_legacy_id_falls_back_to_last_column() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v5.db");
    let rt = make_rt();
    rt.block_on(async {
        let (pool, board_id, _first_column) = open_seeded_pool(&path, 5).await;
        let last_column = Uuid::new_v4();
        add_column(&pool, last_column, board_id, 3, "2024-01-02T00:00:00Z").await;
        sqlx::query("UPDATE boards SET completion_column_id = ? WHERE id = ?")
            .bind(Uuid::new_v4().to_string())
            .bind(board_id.to_string())
            .execute(&pool)
            .await
            .unwrap();

        SqliteStore::migrate_v5_to_v6_completion_columns(&pool)
            .await
            .unwrap();

        assert_eq!(
            completion_rows(&pool, board_id).await,
            vec![last_column.to_string()],
            "a dangling legacy id must fall back to the sorted-last column"
        );
    });
}

#[test]
fn test_sqlite_migration_v5_to_v6_tie_break_prefers_created_at_then_id() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v5.db");
    let rt = make_rt();
    rt.block_on(async {
        let (pool, board_id, _first_column) = open_seeded_pool(&path, 5).await;
        let earlier = Uuid::new_v4();
        let later = Uuid::new_v4();
        // Same position: the later created_at must sort last and win.
        add_column(&pool, earlier, board_id, 5, "2024-01-01T00:00:00Z").await;
        add_column(&pool, later, board_id, 5, "2024-06-01T00:00:00Z").await;

        SqliteStore::migrate_v5_to_v6_completion_columns(&pool)
            .await
            .unwrap();

        assert_eq!(
            completion_rows(&pool, board_id).await,
            vec![later.to_string()],
            "equal position: the later created_at sorts last and wins"
        );
    });
}

#[test]
fn test_sqlite_migration_v5_to_v6_board_without_columns_gets_no_row() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v5.db");
    let rt = make_rt();
    rt.block_on(async {
        let (pool, _board_id, _column_id) = open_seeded_pool(&path, 5).await;
        let empty_board = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO boards (id, name, created_at, updated_at)
             VALUES (?, 'Empty', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        )
        .bind(empty_board.to_string())
        .execute(&pool)
        .await
        .unwrap();

        SqliteStore::migrate_v5_to_v6_completion_columns(&pool)
            .await
            .unwrap();

        assert_eq!(
            completion_rows(&pool, empty_board).await,
            Vec::<String>::new(),
            "a board with no columns gets no completion row"
        );
    });
}

#[test]
fn test_sqlite_migration_v5_to_v6_drops_legacy_boards_column() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v5.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v5_db(&path).await;

        let store = SqliteStore::open(&path).await.unwrap();

        let has_legacy: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('boards')
             WHERE name = 'completion_column_id'",
        )
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert!(
            !has_legacy,
            "boards.completion_column_id must be dropped by the v5 -> v6 migration"
        );
    });
}

#[test]
fn test_sqlite_migration_v5_to_v6_is_idempotent_on_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v5.db");
    let rt = make_rt();
    rt.block_on(async {
        let (pool, board_id, column_id) = open_seeded_pool(&path, 5).await;

        SqliteStore::migrate_v5_to_v6_completion_columns(&pool)
            .await
            .unwrap();
        let after_first = completion_rows(&pool, board_id).await;

        SqliteStore::migrate_v5_to_v6_completion_columns(&pool)
            .await
            .unwrap();
        let after_second = completion_rows(&pool, board_id).await;

        assert_eq!(after_first, vec![column_id.to_string()]);
        assert_eq!(
            after_first, after_second,
            "reapplying must not duplicate or alter the backfilled rows"
        );
    });
}

#[test]
fn test_sqlite_migration_v5_to_v6_writes_v5_backup() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v5.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v5_db(&path).await;

        let store = SqliteStore::open(&path).await.unwrap();

        let backup = SqliteStore::backup_path_for(&path, 5);
        assert!(
            backup.exists(),
            "expected a <path>.v5.backup file after opening a schema-5 DB"
        );

        let backup_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(&backup))
            .await
            .unwrap();
        let backup_version: u32 =
            sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
                .fetch_one(&backup_pool)
                .await
                .unwrap();
        assert_eq!(backup_version, 5, "backup must be a pre-migration snapshot");
        let backup_has_legacy: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('boards')
             WHERE name = 'completion_column_id'",
        )
        .fetch_one(&backup_pool)
        .await
        .unwrap();
        assert!(
            backup_has_legacy,
            "backup must predate the completion-columns migration"
        );
        backup_pool.close().await;

        let live_version: u32 =
            sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(live_version, 10, "live store must be migrated to current");
    });
}
