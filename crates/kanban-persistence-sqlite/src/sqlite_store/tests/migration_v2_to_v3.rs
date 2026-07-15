use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::Row;
use std::path::Path;
use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;

/// Seed a schema_version-2 shaped DB directly (no board_id on archived_cards,
/// cards.column_id carrying the ON DELETE CASCADE FK). Returns
/// (board_id, column_id, card_id). `original_column_id` on the archived row is
/// taken from `orig_col` so a caller can simulate a since-deleted column.
pub(crate) async fn seed_v2_db(path: &Path, orig_col: Uuid) -> (Uuid, Uuid, Uuid) {
    // foreign_keys(false): the seed inserts forward references and the v2 shape
    // is asserted structurally, not enforced here.
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
            schema_version INTEGER NOT NULL DEFAULT 2,
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
            id TEXT PRIMARY KEY, column_id TEXT NOT NULL, title TEXT NOT NULL,
            description TEXT, priority TEXT NOT NULL DEFAULT 'Medium',
            status TEXT NOT NULL DEFAULT 'Todo', position INTEGER NOT NULL,
            due_date TEXT, points INTEGER, card_number INTEGER NOT NULL DEFAULT 0,
            sprint_id TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
            completed_at TEXT,
            FOREIGN KEY (column_id) REFERENCES columns(id) ON DELETE CASCADE
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
            FOREIGN KEY (card_id) REFERENCES cards(id) ON DELETE CASCADE
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
         VALUES (1, ?, '2024-01-01T00:00:00Z', 2)",
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
         VALUES (?, ?, 'Archived', 0, 1, '2024-01-01T00:00:00Z', '2024-02-02T00:00:00Z')",
    )
    .bind(card_id.to_string())
    .bind(column_id.to_string())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sprint_logs (card_id, sprint_id, sprint_number, sprint_name, started_at, status)
         VALUES (?, ?, 1, 'S1', '2024-01-10T00:00:00Z', 'Completed')",
    )
    .bind(card_id.to_string())
    .bind(Uuid::new_v4().to_string())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO archived_cards (card_id, archived_at, original_column_id, original_position)
         VALUES (?, '2024-03-03T00:00:00Z', ?, 0)",
    )
    .bind(card_id.to_string())
    .bind(orig_col.to_string())
    .execute(&pool)
    .await
    .unwrap();

    pool.close().await;
    (board_id, column_id, card_id)
}

async fn archived_board_id(store: &SqliteStore, card_id: Uuid) -> String {
    sqlx::query("SELECT board_id FROM archived_cards WHERE card_id = ?")
        .bind(card_id.to_string())
        .fetch_one(store.pool())
        .await
        .unwrap()
        .try_get::<String, _>("board_id")
        .unwrap()
}

#[test]
fn test_migrate_v2_db_adds_board_id_and_backfills() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v2.db");
    let rt = make_rt();
    rt.block_on(async {
        let (board_id, column_id, card_id) = seed_v2_db(&path, /* orig_col = */ Uuid::nil()).await;
        // Point the archived row's original_column_id at the real column so the
        // backfill has something to resolve.
        let seed = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .foreign_keys(false),
            )
            .await
            .unwrap();
        sqlx::query("UPDATE archived_cards SET original_column_id = ? WHERE card_id = ?")
            .bind(column_id.to_string())
            .bind(card_id.to_string())
            .execute(&seed)
            .await
            .unwrap();
        seed.close().await;

        let store = SqliteStore::open(&path).await.unwrap();

        let has_board_id: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('archived_cards') WHERE name = 'board_id'",
        )
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert!(has_board_id, "board_id column must be added");

        assert_eq!(
            archived_board_id(&store, card_id).await,
            board_id.to_string(),
            "board_id backfilled from original_column_id -> columns.board_id"
        );

        // Card row still lives in `cards` (no row move under D10).
        let card_still_present: bool =
            sqlx::query_scalar("SELECT COUNT(*) > 0 FROM cards WHERE id = ?")
                .bind(card_id.to_string())
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert!(card_still_present, "card row must remain in cards");

        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(
            version, 4,
            "schema_version bumped to 4 (2->3 archived_cards then generic bump)"
        );
    });
}

#[test]
fn test_migrate_v2_db_backfills_nil_when_original_column_missing() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v2.db");
    let rt = make_rt();
    rt.block_on(async {
        // original_column_id points at a column that never existed.
        let (_board_id, _column_id, card_id) = seed_v2_db(&path, Uuid::new_v4()).await;
        let store = SqliteStore::open(&path).await.unwrap();
        assert_eq!(
            archived_board_id(&store, card_id).await,
            Uuid::nil().to_string(),
            "unresolvable column -> nil board_id"
        );
    });
}

#[test]
fn test_migrate_v2_db_preserves_archived_sprint_logs() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v2.db");
    let rt = make_rt();
    rt.block_on(async {
        let (_b, _c, card_id) = seed_v2_db(&path, Uuid::nil()).await;
        let store = SqliteStore::open(&path).await.unwrap();
        // D10 regression guard: the card never left `cards`, so its sprint_logs
        // (FK -> cards) must survive the migration untouched.
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sprint_logs WHERE card_id = ?")
            .bind(card_id.to_string())
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(count, 1, "archived card's sprint_logs must be preserved");
    });
}

#[test]
fn test_migrate_is_idempotent_on_v3_db() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v2.db");
    let rt = make_rt();
    rt.block_on(async {
        let (_b, _c, card_id) = seed_v2_db(&path, Uuid::nil()).await;
        // First open migrates to v3.
        SqliteStore::open(&path).await.unwrap();
        // Second open must be a no-op: no duplicate board_id column, version stays 3.
        let store = SqliteStore::open(&path).await.unwrap();

        let board_id_cols: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('archived_cards') WHERE name = 'board_id'",
        )
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(board_id_cols, 1, "board_id column must not be duplicated");

        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(version, 4);

        let still_there: bool =
            sqlx::query_scalar("SELECT COUNT(*) > 0 FROM archived_cards WHERE card_id = ?")
                .bind(card_id.to_string())
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert!(still_there, "archived card survives idempotent re-open");
    });
}
