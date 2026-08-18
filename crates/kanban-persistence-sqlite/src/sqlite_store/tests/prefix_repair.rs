//! Coverage for `SqliteStore::repair_unbacked_card_namespaces`: inserts a row
//! for an unbacked namespace and raises a backed row's counters to cover the
//! cards naming it.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::path::Path;
use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;

async fn raw(path: &Path) -> Pool<Sqlite> {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .foreign_keys(false),
        )
        .await
        .unwrap()
}

async fn seed_card(
    pool: &Pool<Sqlite>,
    board_id: &str,
    column_id: &str,
    card_id: &str,
    prefix: &str,
    number: i64,
) {
    sqlx::raw_sql(&format!(
        "PRAGMA foreign_keys = OFF;
         INSERT INTO boards (id, name, card_prefix, created_at, updated_at)
             VALUES ('{board_id}','Board','{prefix}','2024-01-01T00:00:00Z','2024-01-01T00:00:00Z')
             ON CONFLICT(id) DO NOTHING;
         INSERT INTO columns (id, board_id, name, position, created_at, updated_at)
             VALUES ('{column_id}','{board_id}','Todo',0,'2024-01-01T00:00:00Z','2024-01-01T00:00:00Z')
             ON CONFLICT(id) DO NOTHING;
         INSERT INTO cards (id, column_id, board_id, title, position, priority, status,
                            card_number, prefix, created_at, updated_at)
             VALUES ('{card_id}','{column_id}','{board_id}','Card',0,'medium','todo',
                     {number},'{prefix}','2024-01-01T00:00:00Z','2024-01-01T00:00:00Z');
         PRAGMA foreign_keys = ON;"
    ))
    .execute(pool)
    .await
    .unwrap();
}

async fn prefix_rows(pool: &Pool<Sqlite>) -> Vec<(String, i64, i64)> {
    let mut rows: Vec<(String, i64, i64)> =
        sqlx::query_as("SELECT name, card_counter, sprint_counter FROM prefixes")
            .fetch_all(pool)
            .await
            .unwrap();
    rows.sort();
    rows
}

#[test]
fn test_repair_inserts_a_row_for_an_unbacked_namespace() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();
        seed_card(
            pool,
            "00000000-0000-0000-0000-0000000000b1",
            "00000000-0000-0000-0000-0000000000c1",
            "00000000-0000-0000-0000-0000000000a1",
            "KAN",
            7,
        )
        .await;

        let inserted = SqliteStore::repair_unbacked_card_namespaces(pool)
            .await
            .unwrap();
        assert_eq!(inserted, 1);
        assert_eq!(prefix_rows(pool).await, vec![("kan".to_string(), 7, 0)]);
    });
}

#[test]
fn test_the_repaired_counter_is_the_high_water_mark() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();
        seed_card(
            pool,
            "00000000-0000-0000-0000-0000000000b1",
            "00000000-0000-0000-0000-0000000000c1",
            "00000000-0000-0000-0000-0000000000a1",
            "KAN",
            9,
        )
        .await;
        seed_card(
            pool,
            "00000000-0000-0000-0000-0000000000b2",
            "00000000-0000-0000-0000-0000000000c2",
            "00000000-0000-0000-0000-0000000000a2",
            "kan",
            2,
        )
        .await;

        let inserted = SqliteStore::repair_unbacked_card_namespaces(pool)
            .await
            .unwrap();
        assert_eq!(inserted, 1);
        assert_eq!(prefix_rows(pool).await, vec![("kan".to_string(), 9, 0)]);
    });
}

#[test]
fn test_the_repair_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();
        seed_card(
            pool,
            "00000000-0000-0000-0000-0000000000b1",
            "00000000-0000-0000-0000-0000000000c1",
            "00000000-0000-0000-0000-0000000000a1",
            "KAN",
            3,
        )
        .await;

        let first = SqliteStore::repair_unbacked_card_namespaces(pool)
            .await
            .unwrap();
        assert_eq!(first, 1);
        let second = SqliteStore::repair_unbacked_card_namespaces(pool)
            .await
            .unwrap();
        assert_eq!(second, 0);
        assert_eq!(prefix_rows(pool).await.len(), 1);
    });
}

#[test]
fn test_the_repair_never_lowers_an_existing_counter() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();
        sqlx::raw_sql(
            "INSERT INTO prefixes (name, card_counter, sprint_counter) VALUES ('kan', 50, 4)",
        )
        .execute(pool)
        .await
        .unwrap();
        seed_card(
            pool,
            "00000000-0000-0000-0000-0000000000b1",
            "00000000-0000-0000-0000-0000000000c1",
            "00000000-0000-0000-0000-0000000000a1",
            "KAN",
            3,
        )
        .await;

        let inserted = SqliteStore::repair_unbacked_card_namespaces(pool)
            .await
            .unwrap();
        assert_eq!(inserted, 0);
        assert_eq!(prefix_rows(pool).await, vec![("kan".to_string(), 50, 4)]);
    });
}

#[test]
fn test_the_repair_ignores_empty_prefix_cards() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();
        seed_card(
            pool,
            "00000000-0000-0000-0000-0000000000b1",
            "00000000-0000-0000-0000-0000000000c1",
            "00000000-0000-0000-0000-0000000000a1",
            "",
            5,
        )
        .await;

        let inserted = SqliteStore::repair_unbacked_card_namespaces(pool)
            .await
            .unwrap();
        assert_eq!(inserted, 0);
        assert_eq!(prefix_rows(pool).await.len(), 0);
    });
}

/// The regression for the dangling-column archived-card orphan path found
/// during planning: a pre-v11 database where an archived card's column was
/// deleted so `migrate_v10_to_v11_card_prefix` stamps it `task`, while
/// `migrate_v9_to_v10_prefixes` never creates a `task` row because every
/// board sets an explicit `card_prefix`.
#[test]
fn test_a_card_stamped_from_a_dangling_column_is_backed_after_open() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("stale.db");
    let rt = make_rt();
    rt.block_on(async {
        let board = "00000000-0000-0000-0000-0000000000b1";
        let column = "00000000-0000-0000-0000-0000000000c1";
        let live_card = "00000000-0000-0000-0000-0000000000a1";
        let dangling_card = "00000000-0000-0000-0000-0000000000a2";
        let dangling_column = "00000000-0000-0000-0000-0000000000cd";

        {
            let store = SqliteStore::open(&path).await.unwrap();
            let pool = store.pool();
            sqlx::raw_sql(&format!(
                "INSERT INTO boards (id, name, card_prefix, created_at, updated_at)
                     VALUES ('{board}','Board','KAN','2024-01-01T00:00:00Z','2024-01-01T00:00:00Z');
                 INSERT INTO columns (id, board_id, name, position, created_at, updated_at)
                     VALUES ('{column}','{board}','Todo',0,'2024-01-01T00:00:00Z','2024-01-01T00:00:00Z');
                 INSERT INTO cards (id, column_id, board_id, title, position, priority, status,
                                    card_number, created_at, updated_at)
                     VALUES ('{live_card}','{column}','{board}','Live',0,'medium','todo',1,
                             '2024-01-01T00:00:00Z','2024-01-01T00:00:00Z'),
                            ('{dangling_card}','{dangling_column}','{board}','Archived',0,'medium','todo',4,
                             '2024-01-01T00:00:00Z','2024-01-01T00:00:00Z');
                 INSERT INTO archived_cards (card_id, board_id, archived_at, original_column_id, original_position)
                     VALUES ('{dangling_card}','{board}','2024-01-01T00:00:00Z','{dangling_column}',0);"
            ))
            .execute(pool)
            .await
            .unwrap();
        }

        let pool = raw(&path).await;
        sqlx::raw_sql(
            "DROP INDEX IF EXISTS idx_cards_prefix_nocase_number;
             CREATE TABLE cards_v9 (
                 id TEXT PRIMARY KEY, column_id TEXT NOT NULL, board_id TEXT NOT NULL,
                 title TEXT NOT NULL, description TEXT,
                 priority TEXT NOT NULL DEFAULT 'Medium', status TEXT NOT NULL DEFAULT 'Todo',
                 position INTEGER NOT NULL, due_date TEXT,
                 points INTEGER CHECK (points >= 0 AND points <= 255),
                 card_number INTEGER NOT NULL DEFAULT 0, sprint_id TEXT,
                 created_at TEXT NOT NULL, updated_at TEXT NOT NULL, completed_at TEXT
             );
             INSERT INTO cards_v9 (id, column_id, board_id, title, description, priority, status,
                                   position, due_date, points, card_number, sprint_id,
                                   created_at, updated_at, completed_at)
                 SELECT id, column_id, board_id, title, description, priority, status,
                        position, due_date, points, card_number, sprint_id,
                        created_at, updated_at, completed_at FROM cards;
             DROP TABLE cards;
             ALTER TABLE cards_v9 RENAME TO cards;
             ALTER TABLE boards ADD COLUMN card_counter INTEGER NOT NULL DEFAULT 1;
             UPDATE boards SET card_counter = 5;
             DELETE FROM prefixes;
             UPDATE metadata SET schema_version = 9;",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();

        let unbacked: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM cards c WHERE c.prefix <> '' AND NOT EXISTS \
             (SELECT 1 FROM prefixes p WHERE p.name = c.prefix COLLATE NOCASE)",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(unbacked, 0, "every card's namespace must now be backed");

        let dangling_prefix: String =
            sqlx::query_scalar("SELECT prefix FROM cards WHERE id = ?")
                .bind(dangling_card)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(dangling_prefix, "task");

        let task_row: (i64,) =
            sqlx::query_as("SELECT card_counter FROM prefixes WHERE name = 'task'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(task_row.0, 4);

        let kan_row: (i64,) =
            sqlx::query_as("SELECT card_counter FROM prefixes WHERE name = 'kan'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(
            kan_row.0, 4,
            "kan was already backed by the v9->v10 migration (from boards.card_counter=5, \
             high-water 4); the repair must not touch it"
        );
    });
}

#[test]
fn test_the_repair_raises_a_backed_counter_to_cover_its_cards() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();
        sqlx::raw_sql(
            "INSERT INTO prefixes (name, card_counter, sprint_counter) VALUES ('task', 3, 0)",
        )
        .execute(pool)
        .await
        .unwrap();
        seed_card(
            pool,
            "00000000-0000-0000-0000-0000000000b1",
            "00000000-0000-0000-0000-0000000000c1",
            "00000000-0000-0000-0000-0000000000a1",
            "task",
            7,
        )
        .await;

        let repaired = SqliteStore::repair_unbacked_card_namespaces(pool)
            .await
            .unwrap();
        assert_eq!(repaired, 1);
        assert_eq!(prefix_rows(pool).await, vec![("task".to_string(), 7, 0)]);
    });
}

#[test]
fn test_the_repair_raises_and_inserts_in_one_pass() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();
        sqlx::raw_sql(
            "INSERT INTO prefixes (name, card_counter, sprint_counter) VALUES ('task', 3, 0)",
        )
        .execute(pool)
        .await
        .unwrap();
        seed_card(
            pool,
            "00000000-0000-0000-0000-0000000000b1",
            "00000000-0000-0000-0000-0000000000c1",
            "00000000-0000-0000-0000-0000000000a1",
            "task",
            7,
        )
        .await;
        seed_card(
            pool,
            "00000000-0000-0000-0000-0000000000b2",
            "00000000-0000-0000-0000-0000000000c2",
            "00000000-0000-0000-0000-0000000000a2",
            "ops",
            2,
        )
        .await;

        let repaired = SqliteStore::repair_unbacked_card_namespaces(pool)
            .await
            .unwrap();
        assert_eq!(repaired, 2);
        assert_eq!(
            prefix_rows(pool).await,
            vec![("ops".to_string(), 2, 0), ("task".to_string(), 7, 0)]
        );
    });
}

#[test]
fn test_the_raise_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();
        sqlx::raw_sql(
            "INSERT INTO prefixes (name, card_counter, sprint_counter) VALUES ('task', 3, 0)",
        )
        .execute(pool)
        .await
        .unwrap();
        seed_card(
            pool,
            "00000000-0000-0000-0000-0000000000b1",
            "00000000-0000-0000-0000-0000000000c1",
            "00000000-0000-0000-0000-0000000000a1",
            "task",
            7,
        )
        .await;

        let first = SqliteStore::repair_unbacked_card_namespaces(pool)
            .await
            .unwrap();
        assert_eq!(first, 1);
        let second = SqliteStore::repair_unbacked_card_namespaces(pool)
            .await
            .unwrap();
        assert_eq!(second, 0);
        assert_eq!(prefix_rows(pool).await, vec![("task".to_string(), 7, 0)]);
    });
}

#[test]
fn test_a_dangling_column_card_raises_an_already_backed_default_namespace() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("stale.db");
    let rt = make_rt();
    rt.block_on(async {
        let b1 = "00000000-0000-0000-0000-0000000001b1";
        let b2 = "00000000-0000-0000-0000-0000000001b2";
        let c1 = "00000000-0000-0000-0000-0000000001c1";
        let c2 = "00000000-0000-0000-0000-0000000001c2";
        let live_card = "00000000-0000-0000-0000-0000000001a1";
        let dangling_card = "00000000-0000-0000-0000-0000000001a2";
        let dangling_column = "00000000-0000-0000-0000-0000000001cd";

        {
            let store = SqliteStore::open(&path).await.unwrap();
            let pool = store.pool();
            sqlx::raw_sql(&format!(
                "INSERT INTO boards (id, name, card_prefix, created_at, updated_at)
                     VALUES ('{b1}','Board1','KAN','2024-01-01T00:00:00Z','2024-01-01T00:00:00Z'),
                            ('{b2}','Board2','task','2024-01-01T00:00:00Z','2024-01-01T00:00:00Z');
                 INSERT INTO columns (id, board_id, name, position, created_at, updated_at)
                     VALUES ('{c1}','{b1}','Todo',0,'2024-01-01T00:00:00Z','2024-01-01T00:00:00Z'),
                            ('{c2}','{b2}','Todo',0,'2024-01-01T00:00:00Z','2024-01-01T00:00:00Z');
                 INSERT INTO cards (id, column_id, board_id, title, position, priority, status,
                                    card_number, created_at, updated_at)
                     VALUES ('{live_card}','{c1}','{b1}','Live',0,'medium','todo',1,
                             '2024-01-01T00:00:00Z','2024-01-01T00:00:00Z'),
                            ('{dangling_card}','{dangling_column}','{b1}','Archived',0,'medium','todo',7,
                             '2024-01-01T00:00:00Z','2024-01-01T00:00:00Z');
                 INSERT INTO archived_cards (card_id, board_id, archived_at, original_column_id, original_position)
                     VALUES ('{dangling_card}','{b1}','2024-01-01T00:00:00Z','{dangling_column}',0);"
            ))
            .execute(pool)
            .await
            .unwrap();
        }

        let pool = raw(&path).await;
        sqlx::raw_sql(
            "DROP INDEX IF EXISTS idx_cards_prefix_nocase_number;
             CREATE TABLE cards_v9 (
                 id TEXT PRIMARY KEY, column_id TEXT NOT NULL, board_id TEXT NOT NULL,
                 title TEXT NOT NULL, description TEXT,
                 priority TEXT NOT NULL DEFAULT 'Medium', status TEXT NOT NULL DEFAULT 'Todo',
                 position INTEGER NOT NULL, due_date TEXT,
                 points INTEGER CHECK (points >= 0 AND points <= 255),
                 card_number INTEGER NOT NULL DEFAULT 0, sprint_id TEXT,
                 created_at TEXT NOT NULL, updated_at TEXT NOT NULL, completed_at TEXT
             );
             INSERT INTO cards_v9 (id, column_id, board_id, title, description, priority, status,
                                   position, due_date, points, card_number, sprint_id,
                                   created_at, updated_at, completed_at)
                 SELECT id, column_id, board_id, title, description, priority, status,
                        position, due_date, points, card_number, sprint_id,
                        created_at, updated_at, completed_at FROM cards;
             DROP TABLE cards;
             ALTER TABLE cards_v9 RENAME TO cards;
             ALTER TABLE boards ADD COLUMN card_counter INTEGER NOT NULL DEFAULT 1;
             UPDATE boards SET card_counter = 11 WHERE id = '{b1}';
             UPDATE boards SET card_counter = 4 WHERE id = '{b2}';
             DELETE FROM prefixes;
             UPDATE metadata SET schema_version = 9;"
                .replace("{b1}", b1)
                .replace("{b2}", b2)
                .as_str(),
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();

        let task_row: (i64,) =
            sqlx::query_as("SELECT card_counter FROM prefixes WHERE name = 'task'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(task_row.0, 7);

        let kan_row: (i64,) =
            sqlx::query_as("SELECT card_counter FROM prefixes WHERE name = 'kan'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(kan_row.0, 10);
    });
}
