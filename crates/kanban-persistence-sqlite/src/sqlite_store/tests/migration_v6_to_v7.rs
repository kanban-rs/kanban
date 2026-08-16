use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;
use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;

/// Seed a schema_version-6 shaped DB directly: `boards` already lacks the
/// legacy `completion_column_id` column, `board_completion_columns` already
/// exists, and `columns` has no `default_status` column yet. Returns
/// (board_id, column_id).
async fn seed_v6_db(path: &Path) -> (Uuid, Uuid) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .foreign_keys(false),
        )
        .await
        .unwrap();

    sqlx::raw_sql(
        "CREATE TABLE metadata (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            instance_id TEXT NOT NULL,
            saved_at TEXT NOT NULL,
            schema_version INTEGER NOT NULL,
            writer_version TEXT,
            writer_commit TEXT
        );
        CREATE TABLE boards (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT,
            sprint_prefix TEXT, card_prefix TEXT,
            task_sort_field TEXT NOT NULL DEFAULT 'Default',
            task_sort_order TEXT NOT NULL DEFAULT 'Ascending',
            sprint_duration_days INTEGER,
            sprint_name_used_count INTEGER NOT NULL DEFAULT 0,
            next_sprint_number INTEGER NOT NULL DEFAULT 1,
            active_sprint_id TEXT,
            task_list_view TEXT NOT NULL DEFAULT 'Flat',
            card_counter INTEGER NOT NULL DEFAULT 1,
            position INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE columns (
            id TEXT PRIMARY KEY, board_id TEXT NOT NULL, name TEXT NOT NULL,
            position INTEGER NOT NULL, wip_limit INTEGER,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
            FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE
        );
        CREATE TABLE board_completion_columns (
            board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
            column_id TEXT NOT NULL REFERENCES columns(id) ON DELETE CASCADE,
            position INTEGER NOT NULL,
            PRIMARY KEY (board_id, column_id)
        );",
    )
    .execute(&pool)
    .await
    .unwrap();

    let board_id = Uuid::new_v4();
    let column_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO metadata (id, instance_id, saved_at, schema_version)
         VALUES (1, ?, '2024-01-01T00:00:00Z', 6)",
    )
    .bind(Uuid::new_v4().to_string())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO boards (id, name, created_at, updated_at)
         VALUES (?, 'B', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
    )
    .bind(board_id.to_string())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO columns (id, board_id, name, position, created_at, updated_at)
         VALUES (?, ?, 'Doing', 0, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
    )
    .bind(column_id.to_string())
    .bind(board_id.to_string())
    .execute(&pool)
    .await
    .unwrap();

    pool.close().await;
    (board_id, column_id)
}

#[test]
fn test_v6_database_upgrades_to_v8_with_todo_default_status_for_non_completion_columns() {
    // v7's terminal NULL is superseded by v8, which runs in the same
    // open() chain and derives a non-null value for every column; a column
    // named "Doing" that is not in board_completion_columns must never be
    // inferred as Done from its name, only from completion membership.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v6.db");
    let rt = make_rt();
    rt.block_on(async {
        let (_board_id, column_id) = seed_v6_db(&path).await;

        let store = SqliteStore::open(&path).await.unwrap();

        let default_status: Option<String> =
            sqlx::query_scalar("SELECT default_status FROM columns WHERE id = ?")
                .bind(column_id.to_string())
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(
            default_status,
            Some("Todo".to_string()),
            "a column named 'Doing' that is not a completion column must derive Todo, never an inferred status from its name"
        );
    });
}

#[test]
fn test_v6_to_v7_migration_leaves_a_v6_backup() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v6.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v6_db(&path).await;

        SqliteStore::open(&path).await.unwrap();

        let backup = SqliteStore::backup_path_for(&path, 6);
        assert!(
            backup.exists(),
            "expected exactly one <path>.v6.backup file after opening a schema-6 DB"
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
        assert_eq!(backup_version, 6, "backup must be a pre-migration snapshot");
        let backup_has_default_status: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('columns') WHERE name = 'default_status'",
        )
        .fetch_one(&backup_pool)
        .await
        .unwrap();
        assert!(
            !backup_has_default_status,
            "backup must predate the default_status migration"
        );
        backup_pool.close().await;
    });
}

#[test]
fn test_migrated_database_reports_schema_version_9() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v6.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v6_db(&path).await;

        let store = SqliteStore::open(&path).await.unwrap();

        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(version, 11, "schema_version must be bumped to 11");
    });
}
