use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::Pool;
use sqlx::Sqlite;
use std::path::Path;
use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;

/// Seed a schema_version-8 shaped DB directly, with `board_completion_columns`
/// already populated and every `columns.default_status` already derived (as
/// the v7 -> v8 step would have left it). Returns (pool, board_id, done_id,
/// other_id, card_id, sprint_id).
async fn seed_v8_db(path: &Path) -> (Pool<Sqlite>, Uuid, Uuid, Uuid, Uuid, Uuid) {
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
            position INTEGER NOT NULL, wip_limit INTEGER, default_status TEXT,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
            FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE
        );
        CREATE TABLE board_completion_columns (
            board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
            column_id TEXT NOT NULL REFERENCES columns(id) ON DELETE CASCADE,
            position INTEGER NOT NULL,
            PRIMARY KEY (board_id, column_id)
        );
        CREATE TABLE cards (
            id TEXT PRIMARY KEY, column_id TEXT NOT NULL, board_id TEXT NOT NULL,
            title TEXT NOT NULL, description TEXT,
            priority TEXT NOT NULL DEFAULT 'Medium', status TEXT NOT NULL DEFAULT 'Todo',
            position INTEGER NOT NULL, due_date TEXT,
            points INTEGER CHECK (points >= 0 AND points <= 255),
            card_number INTEGER NOT NULL DEFAULT 0, sprint_id TEXT,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL, completed_at TEXT
        );
        CREATE TABLE sprints (
            id TEXT PRIMARY KEY, board_id TEXT NOT NULL, name TEXT NOT NULL,
            number INTEGER NOT NULL, status TEXT NOT NULL DEFAULT 'Planned',
            start_date TEXT, end_date TEXT,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE spawns_edges (
            source_id TEXT NOT NULL, target_id TEXT NOT NULL,
            PRIMARY KEY (source_id, target_id)
        );",
    )
    .execute(&pool)
    .await
    .unwrap();

    let board_id = Uuid::new_v4();
    let done_id = Uuid::new_v4();
    let other_id = Uuid::new_v4();
    let card_id = Uuid::new_v4();
    let sprint_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO metadata (id, instance_id, saved_at, schema_version)
         VALUES (1, ?, '2024-01-01T00:00:00Z', 8)",
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
    for (cid, name, pos, status) in [
        (done_id, "Complete", 1, "Done"),
        (other_id, "Doing", 0, "Todo"),
    ] {
        sqlx::query(
            "INSERT INTO columns (id, board_id, name, position, default_status, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        )
        .bind(cid.to_string())
        .bind(board_id.to_string())
        .bind(name)
        .bind(pos)
        .bind(status)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO board_completion_columns (board_id, column_id, position)
         VALUES (?, ?, 0)",
    )
    .bind(board_id.to_string())
    .bind(done_id.to_string())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO cards (id, column_id, board_id, title, position, card_number,
            created_at, updated_at)
         VALUES (?, ?, ?, 'Card', 0, 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
    )
    .bind(card_id.to_string())
    .bind(other_id.to_string())
    .bind(board_id.to_string())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sprints (id, board_id, name, number, created_at, updated_at)
         VALUES (?, ?, 'Sprint 1', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
    )
    .bind(sprint_id.to_string())
    .bind(board_id.to_string())
    .execute(&pool)
    .await
    .unwrap();

    (pool, board_id, done_id, other_id, card_id, sprint_id)
}

#[test]
fn test_sqlite_migration_drops_board_completion_columns_and_preserves_the_graph() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v8.db");
    let rt = make_rt();
    rt.block_on(async {
        let (seed_pool, board_id, done_id, other_id, card_id, sprint_id) =
            seed_v8_db(&path).await;
        seed_pool.close().await;

        let store = SqliteStore::open(&path).await.unwrap();

        let has_table: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='board_completion_columns'",
        )
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert!(!has_table, "board_completion_columns must be dropped");

        // Assert per entity type, not just `boards().len() == 1`: the table
        // drop must not touch anything it doesn't own.
        let board_exists: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM boards WHERE id = ?")
            .bind(board_id.to_string())
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert!(board_exists, "board must survive");

        let columns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM columns WHERE board_id = ?")
            .bind(board_id.to_string())
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(columns, 2, "both columns must survive");

        let done_status: Option<String> =
            sqlx::query_scalar("SELECT default_status FROM columns WHERE id = ?")
                .bind(done_id.to_string())
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(
            done_status,
            Some("Done".to_string()),
            "the completion column's default_status is the only surviving signal"
        );

        let other_status: Option<String> =
            sqlx::query_scalar("SELECT default_status FROM columns WHERE id = ?")
                .bind(other_id.to_string())
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(other_status, Some("Todo".to_string()));

        let card_exists: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM cards WHERE id = ?")
            .bind(card_id.to_string())
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert!(card_exists, "card must survive");

        let sprint_exists: bool =
            sqlx::query_scalar("SELECT COUNT(*) > 0 FROM sprints WHERE id = ?")
                .bind(sprint_id.to_string())
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert!(sprint_exists, "sprint must survive");

        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(version, 11, "schema_version must be bumped to 11");
    });
}

#[test]
fn test_sqlite_migration_v8_to_v9_writes_a_v8_backup_before_dropping() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v8.db");
    let rt = make_rt();
    rt.block_on(async {
        let (seed_pool, ..) = seed_v8_db(&path).await;
        seed_pool.close().await;

        SqliteStore::open(&path).await.unwrap();

        let backup = SqliteStore::backup_path_for(&path, 8);
        assert!(
            backup.exists(),
            "expected a <path>.v8.backup file after opening a schema-8 DB"
        );

        let backup_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(&backup))
            .await
            .unwrap();
        let has_table: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='board_completion_columns'",
        )
        .fetch_one(&backup_pool)
        .await
        .unwrap();
        assert!(
            has_table,
            "the backup must predate the board_completion_columns drop"
        );
        backup_pool.close().await;
    });
}
