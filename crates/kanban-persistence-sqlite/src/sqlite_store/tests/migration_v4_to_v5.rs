use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;
use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;

/// Seed a schema_version-4 shaped DB directly: the 2->3 migration has already
/// run (archived_cards has board_id, cards has no FK on column_id), but
/// cards.board_id (the 4->5 migration) has not. Returns
/// (board_id, column_id, card_id).
async fn seed_v4_db(path: &Path) -> (Uuid, Uuid, Uuid) {
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
            schema_version INTEGER NOT NULL DEFAULT 4,
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
            completion_column_id TEXT,
            position INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE columns (
            id TEXT PRIMARY KEY, board_id TEXT NOT NULL, name TEXT NOT NULL,
            position INTEGER NOT NULL, wip_limit INTEGER,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
            FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE
        );
        CREATE TABLE cards (
            id TEXT PRIMARY KEY,
            column_id TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT,
            priority TEXT NOT NULL DEFAULT 'Medium',
            status TEXT NOT NULL DEFAULT 'Todo',
            position INTEGER NOT NULL,
            due_date TEXT,
            points INTEGER CHECK (points >= 0 AND points <= 255),
            card_number INTEGER NOT NULL DEFAULT 0,
            sprint_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            completed_at TEXT
        );
        CREATE TABLE sprint_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT, card_id TEXT NOT NULL,
            sprint_id TEXT NOT NULL, sprint_number INTEGER NOT NULL,
            sprint_name TEXT, started_at TEXT NOT NULL, ended_at TEXT,
            status TEXT NOT NULL,
            FOREIGN KEY (card_id) REFERENCES cards(id) ON DELETE CASCADE
        );
        CREATE TABLE archived_cards (
            card_id TEXT PRIMARY KEY, archived_at TEXT NOT NULL,
            original_column_id TEXT NOT NULL, original_position INTEGER NOT NULL,
            board_id TEXT NOT NULL
        );",
    )
    .execute(&pool)
    .await
    .unwrap();

    let board_id = Uuid::new_v4();
    let column_id = Uuid::new_v4();
    let card_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO metadata (id, instance_id, saved_at, schema_version)
         VALUES (1, ?, '2024-01-01T00:00:00Z', 4)",
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
         VALUES (?, ?, 'Todo', 0, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
    )
    .bind(column_id.to_string())
    .bind(board_id.to_string())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO cards (id, column_id, title, position, card_number, created_at, updated_at)
         VALUES (?, ?, 'Card', 0, 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
    )
    .bind(card_id.to_string())
    .bind(column_id.to_string())
    .execute(&pool)
    .await
    .unwrap();

    pool.close().await;
    (board_id, column_id, card_id)
}

#[test]
fn test_open_schema4_db_writes_durable_backup_before_board_id_migration() {
    // Regression guard: the schema_version bump to SUPPORTED_SCHEMA_VERSION and
    // the pre-migration backup are coupled only by code review / doc comments
    // (mod.rs's SUPPORTED_SCHEMA_VERSION doc, init.rs's migrate() doc), not by
    // the type system. Existing backup tests only seed schema-2 DBs, leaving
    // the 4->5 boundary (migrate_v4_to_v5_cards_board_id) with no coverage at
    // all -- this pins that boundary specifically.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v4.db");
    let rt = make_rt();
    rt.block_on(async {
        let (board_id, _column_id, card_id) = seed_v4_db(&path).await;

        let store = SqliteStore::open(&path).await.unwrap();

        let backup = SqliteStore::backup_path_for(&path, 4);
        assert!(
            backup.exists(),
            "expected a <path>.v4.backup file after opening a schema-4 DB"
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
        assert_eq!(
            backup_version, 4,
            "backup must be a pre-migration schema_version=4 snapshot"
        );

        let backup_has_board_id: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('cards') WHERE name = 'board_id'",
        )
        .fetch_one(&backup_pool)
        .await
        .unwrap();
        assert!(
            !backup_has_board_id,
            "backup must predate the cards.board_id migration"
        );
        backup_pool.close().await;

        let live_version: u32 =
            sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(live_version, 5, "live store must be migrated to current");

        let live_board_id: String = sqlx::query_scalar("SELECT board_id FROM cards WHERE id = ?")
            .bind(card_id.to_string())
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(
            live_board_id,
            board_id.to_string(),
            "cards.board_id backfilled from column_id -> columns.board_id"
        );
    });
}

#[test]
fn test_open_schema5_db_writes_no_v4_backup() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v5.db");
    let rt = make_rt();
    rt.block_on(async {
        // A fresh open creates an already-current schema-5 DB.
        SqliteStore::open(&path).await.unwrap();
        // Second open sees pre_migrate_version == SUPPORTED_SCHEMA_VERSION.
        SqliteStore::open(&path).await.unwrap();

        assert!(
            !SqliteStore::backup_path_for(&path, 4).exists(),
            "an already-current DB must not get a v4 backup written"
        );
    });
}
