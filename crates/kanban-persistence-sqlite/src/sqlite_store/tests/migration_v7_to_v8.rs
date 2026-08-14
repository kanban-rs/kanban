use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::Pool;
use sqlx::Sqlite;
use std::path::Path;
use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;

/// Seed a schema_version-7 shaped DB directly: `columns.default_status`
/// already exists (nullable), `board_completion_columns` already exists.
/// Returns (pool, board_id, done_column_id, other_column_id).
async fn seed_v7_db(path: &Path) -> (Pool<Sqlite>, Uuid, Uuid, Uuid) {
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
    let done_column_id = Uuid::new_v4();
    let other_column_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO metadata (id, instance_id, saved_at, schema_version)
         VALUES (1, ?, '2024-01-01T00:00:00Z', 7)",
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
    for (cid, name, pos) in [
        (done_column_id, "Complete", 1),
        (other_column_id, "Doing", 0),
    ] {
        sqlx::query(
            "INSERT INTO columns (id, board_id, name, position, created_at, updated_at)
             VALUES (?, ?, ?, ?, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        )
        .bind(cid.to_string())
        .bind(board_id.to_string())
        .bind(name)
        .bind(pos)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO board_completion_columns (board_id, column_id, position)
         VALUES (?, ?, 0)",
    )
    .bind(board_id.to_string())
    .bind(done_column_id.to_string())
    .execute(&pool)
    .await
    .unwrap();

    (pool, board_id, done_column_id, other_column_id)
}

async fn default_status_of(pool: &Pool<Sqlite>, column_id: Uuid) -> Option<String> {
    sqlx::query_scalar("SELECT default_status FROM columns WHERE id = ?")
        .bind(column_id.to_string())
        .fetch_one(pool)
        .await
        .unwrap()
}

#[test]
fn test_v7_to_v8_sets_done_for_columns_in_board_completion_columns() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v7.db");
    let rt = make_rt();
    rt.block_on(async {
        let (seed_pool, _board_id, done_column_id, _other) = seed_v7_db(&path).await;
        seed_pool.close().await;

        let store = SqliteStore::open(&path).await.unwrap();

        let status = default_status_of(store.pool(), done_column_id).await;
        assert_eq!(status, Some("Done".to_string()));
    });
}

#[test]
fn test_v7_to_v8_sets_todo_for_other_columns() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v7.db");
    let rt = make_rt();
    rt.block_on(async {
        let (seed_pool, _board_id, _done, other_column_id) = seed_v7_db(&path).await;
        seed_pool.close().await;

        let store = SqliteStore::open(&path).await.unwrap();

        let status = default_status_of(store.pool(), other_column_id).await;
        assert_eq!(status, Some("Todo".to_string()));
    });
}

#[test]
fn test_v7_to_v8_existing_default_status_wins_over_derivation() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v7.db");
    let rt = make_rt();
    rt.block_on(async {
        let (seed_pool, _board_id, done_column_id, _other) = seed_v7_db(&path).await;
        sqlx::query("UPDATE columns SET default_status = 'InProgress' WHERE id = ?")
            .bind(done_column_id.to_string())
            .execute(&seed_pool)
            .await
            .unwrap();
        seed_pool.close().await;

        let store = SqliteStore::open(&path).await.unwrap();

        let status = default_status_of(store.pool(), done_column_id).await;
        assert_eq!(
            status,
            Some("InProgress".to_string()),
            "a completion column carrying an explicit non-Done default_status must keep it"
        );
    });
}

#[test]
fn test_v7_to_v8_leaves_board_completion_columns_in_place() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v7.db");
    let rt = make_rt();
    rt.block_on(async {
        let (seed_pool, board_id, done_column_id, _other) = seed_v7_db(&path).await;
        seed_pool.close().await;

        let store = SqliteStore::open(&path).await.unwrap();

        let rows = super::completion_rows(&store, board_id).await;
        assert_eq!(rows, vec![done_column_id.to_string()]);
    });
}

#[test]
fn test_v7_to_v8_writes_a_v7_backup_before_migrating() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v7.db");
    let rt = make_rt();
    rt.block_on(async {
        let (seed_pool, _board_id, _done, _other) = seed_v7_db(&path).await;
        seed_pool.close().await;

        SqliteStore::open(&path).await.unwrap();

        let backup = SqliteStore::backup_path_for(&path, 7);
        assert!(
            backup.exists(),
            "expected a <path>.v7.backup file after opening a schema-7 DB"
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
        assert_eq!(backup_version, 7, "backup must be a pre-migration snapshot");
        backup_pool.close().await;
    });
}

#[test]
fn test_v7_to_v8_preserves_cards_columns_sprints_and_edges() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v7.db");
    let rt = make_rt();
    rt.block_on(async {
        let (seed_pool, board_id, done_column_id, other_column_id) = seed_v7_db(&path).await;

        let card_a = Uuid::new_v4();
        let card_b = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO cards (id, column_id, board_id, title, position, card_number,
                created_at, updated_at)
             VALUES (?, ?, ?, 'Card A', 0, 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        )
        .bind(card_a.to_string())
        .bind(other_column_id.to_string())
        .bind(board_id.to_string())
        .execute(&seed_pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO cards (id, column_id, board_id, title, position, card_number,
                created_at, updated_at)
             VALUES (?, ?, ?, 'Card B', 1, 2, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        )
        .bind(card_b.to_string())
        .bind(done_column_id.to_string())
        .bind(board_id.to_string())
        .execute(&seed_pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO sprints (id, board_id, name, number, created_at, updated_at)
             VALUES (?, ?, 'Sprint 1', 1, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(board_id.to_string())
        .execute(&seed_pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO spawns_edges (source_id, target_id) VALUES (?, ?)")
            .bind(card_a.to_string())
            .bind(card_b.to_string())
            .execute(&seed_pool)
            .await
            .unwrap();
        seed_pool.close().await;

        let store = SqliteStore::open(&path).await.unwrap();

        let boards: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM boards")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(boards, 1, "board survived");

        let columns: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM columns")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(columns, 2, "both columns survived");

        let cards: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cards")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(cards, 2, "both cards survived");

        let sprints: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sprints")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(sprints, 1, "sprint survived");

        let edges: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM spawns_edges")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(edges, 1, "spawns edge survived");

        let completion_rows_after = super::completion_rows(&store, board_id).await;
        assert_eq!(completion_rows_after, vec![done_column_id.to_string()]);
    });
}

#[test]
fn test_v8_migration_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v7.db");
    let rt = make_rt();
    rt.block_on(async {
        let (seed_pool, _board_id, done_column_id, other_column_id) = seed_v7_db(&path).await;
        seed_pool.close().await;

        {
            let store = SqliteStore::open(&path).await.unwrap();
            assert_eq!(
                default_status_of(store.pool(), done_column_id).await,
                Some("Done".to_string())
            );
        }

        let store = SqliteStore::open(&path).await.unwrap();
        assert_eq!(
            default_status_of(store.pool(), done_column_id).await,
            Some("Done".to_string())
        );
        assert_eq!(
            default_status_of(store.pool(), other_column_id).await,
            Some("Todo".to_string())
        );
    });
}
