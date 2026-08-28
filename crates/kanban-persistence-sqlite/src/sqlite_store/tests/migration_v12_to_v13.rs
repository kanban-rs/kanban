//! Schema 12 -> 13: `cards.prefix` is backed by a foreign key to
//! `prefixes(name)`, `ON DELETE RESTRICT ON UPDATE RESTRICT`.
//!
//! `cards` is rebuilt with foreign keys disabled -- `DROP TABLE cards` with
//! enforcement on would fire `ON DELETE CASCADE` on `sprint_logs` and
//! `archived_cards` and empty them. The copy is
//! verified against the constraint before the destructive swap, so a
//! database that cannot satisfy it is refused with the original table
//! untouched.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::path::Path;
use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;

const BOARD: &str = "00000000-0000-0000-0000-0000000000b1";
const COLUMN: &str = "00000000-0000-0000-0000-0000000000c1";
const SPRINT: &str = "00000000-0000-0000-0000-000000000051";
const CARD1: &str = "00000000-0000-0000-0000-0000000000a1";
const CARD2: &str = "00000000-0000-0000-0000-0000000000a2";
const TS: &str = "2024-01-01T00:00:00Z";

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

async fn count(pool: &Pool<Sqlite>, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

/// Builds a v12-shaped database holding a NON-TRIVIAL graph (board, column,
/// two cards under prefix 'KAN', a sprint, a spawns edge, a sprint log, an
/// archived card, and the 'kan' prefix row), then reverts the schema marker
/// so opening it triggers the 12 -> 13 migration.
///
/// Seeded through a real `SqliteStore::open` first so the shape cannot drift
/// from the real one, then reverted to the pre-FK `cards` table on a raw
/// FK-off pool.
async fn seed_v12_with_graph(path: &Path) {
    {
        let store = SqliteStore::open(path).await.unwrap();
        let pool = store.pool();
        sqlx::raw_sql(&format!(
            "INSERT INTO boards (id, name, card_prefix, created_at, updated_at)
                 VALUES ('{BOARD}','Board','KAN','{TS}','{TS}');
             INSERT INTO columns (id, board_id, name, position, created_at, updated_at)
                 VALUES ('{COLUMN}','{BOARD}','Todo',0,'{TS}','{TS}');
             INSERT INTO sprints (id, board_id, sprint_number, status, created_at, updated_at)
                 VALUES ('{SPRINT}','{BOARD}',3,'Planning','{TS}','{TS}');
             INSERT INTO prefixes (name, card_counter, sprint_counter)
                 VALUES ('kan', 2, 1);
             INSERT INTO cards (id, column_id, board_id, title, position, priority, status,
                                card_number, prefix, sprint_id, created_at, updated_at)
                 VALUES ('{CARD1}','{COLUMN}','{BOARD}','One',0,'medium','todo',1,'KAN','{SPRINT}','{TS}','{TS}'),
                        ('{CARD2}','{COLUMN}','{BOARD}','Two',1,'medium','todo',2,'KAN',NULL,'{TS}','{TS}');
             INSERT INTO spawns_edges (source_id, target_id, created_at)
                 VALUES ('{CARD1}','{CARD2}','{TS}');
             INSERT INTO sprint_logs (card_id, sprint_id, sprint_number, started_at, status)
                 VALUES ('{CARD1}','{SPRINT}',3,'{TS}','Active');
             INSERT INTO archived_cards (card_id, board_id, archived_at, original_column_id, original_position)
                 VALUES ('{CARD2}','{BOARD}','{TS}','{COLUMN}',1);"
        ))
        .execute(pool)
        .await
        .unwrap();
    }

    let pool = raw(path).await;
    let index_ddl: Vec<(String,)> = sqlx::query_as(
        "SELECT sql FROM sqlite_master WHERE type = 'index' AND tbl_name = 'cards' AND sql IS NOT NULL",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    sqlx::raw_sql(
        "PRAGMA foreign_keys = OFF;
        BEGIN;
        CREATE TABLE cards_legacy (
            id TEXT PRIMARY KEY,
            column_id TEXT NOT NULL,
            board_id TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT,
            priority TEXT NOT NULL DEFAULT 'Medium',
            status TEXT NOT NULL DEFAULT 'Todo',
            position INTEGER NOT NULL,
            due_date TEXT,
            points INTEGER CHECK (points >= 0 AND points <= 255),
            card_number INTEGER NOT NULL DEFAULT 0,
            prefix TEXT NOT NULL DEFAULT '',
            sprint_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            completed_at TEXT,
            FOREIGN KEY (sprint_id) REFERENCES sprints(id) ON DELETE SET NULL
        );
        INSERT INTO cards_legacy (id, column_id, board_id, title, description, priority, status,
            position, due_date, points, card_number, prefix, sprint_id, created_at, updated_at, completed_at)
            SELECT id, column_id, board_id, title, description, priority, status,
                position, due_date, points, card_number, prefix, sprint_id, created_at, updated_at, completed_at
            FROM cards;
        DROP TABLE cards;
        ALTER TABLE cards_legacy RENAME TO cards;
        COMMIT;
        PRAGMA foreign_keys = ON;",
    )
    .execute(&pool)
    .await
    .unwrap();

    for (sql,) in index_ddl {
        sqlx::raw_sql(&sql).execute(&pool).await.unwrap();
    }

    sqlx::raw_sql("UPDATE metadata SET schema_version = 12;")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
}

async fn fk_row(pool: &Pool<Sqlite>) -> Option<(String, String)> {
    sqlx::query_as(
        "SELECT \"on_delete\", \"on_update\" FROM pragma_foreign_key_list('cards') WHERE \"table\" = 'prefixes'",
    )
    .fetch_optional(pool)
    .await
    .unwrap()
}

#[test]
fn test_the_upgrade_puts_a_restricting_foreign_key_on_the_cards_table() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v12.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v12_with_graph(&path).await;
        let store = SqliteStore::open(&path).await.unwrap();

        let row = fk_row(store.pool()).await;
        let (on_delete, on_update) = row.expect("cards must carry an FK to prefixes");
        assert_eq!(on_delete, "RESTRICT");
        assert_eq!(on_update, "RESTRICT");
    });
}

#[test]
fn test_the_rebuild_preserves_the_whole_card_graph_and_every_index() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v12.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v12_with_graph(&path).await;

        let index_names_before: Vec<String> = {
            let pool = raw(&path).await;
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'cards' ORDER BY name",
            )
            .fetch_all(&pool)
            .await
            .unwrap();
            pool.close().await;
            rows.into_iter().map(|(n,)| n).collect()
        };

        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();

        assert!(fk_row(pool).await.is_some());

        let index_names_after: Vec<String> = {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT name FROM sqlite_master WHERE type = 'index' AND tbl_name = 'cards' ORDER BY name",
            )
            .fetch_all(pool)
            .await
            .unwrap();
            rows.into_iter().map(|(n,)| n).collect()
        };
        assert_eq!(index_names_before, index_names_after, "an index was dropped by the rebuild");

        for (what, sql, expected) in [
            ("cards", "SELECT COUNT(*) FROM cards", 2),
            ("sprint_logs", "SELECT COUNT(*) FROM sprint_logs", 1),
            ("archived_cards", "SELECT COUNT(*) FROM archived_cards", 1),
            ("spawns_edges", "SELECT COUNT(*) FROM spawns_edges", 1),
        ] {
            assert_eq!(count(pool, sql).await, expected, "{what} did not survive the rebuild");
        }

        assert_eq!(
            count(pool, &format!("SELECT COUNT(*) FROM cards WHERE id='{CARD1}' AND prefix='KAN' AND card_number=1")).await,
            1
        );
        assert_eq!(
            count(pool, &format!("SELECT COUNT(*) FROM cards WHERE id='{CARD2}' AND prefix='KAN' AND card_number=2")).await,
            1
        );
    });
}

#[test]
fn test_the_upgrade_leaves_a_durable_v12_backup() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v12.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v12_with_graph(&path).await;
        let store = SqliteStore::open(&path).await.unwrap();

        let live_version: i64 =
            sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(live_version, 13);

        let backup_path = SqliteStore::backup_path_for(&path, 12);
        assert!(backup_path.exists(), "no .v12.backup was written");

        let backup_pool = raw(&backup_path).await;
        let backup_version: i64 =
            sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
                .fetch_one(&backup_pool)
                .await
                .unwrap();
        assert_eq!(backup_version, 12);
    });
}

#[test]
fn test_the_upgrade_repairs_unbacked_and_empty_namespaces_before_it_constrains() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v12.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v12_with_graph(&path).await;

        let pool = raw(&path).await;
        let zzz_id = "00000000-0000-0000-0000-0000000000a4";
        let empty_id = "00000000-0000-0000-0000-0000000000a5";
        sqlx::raw_sql(&format!(
            "INSERT INTO cards (id, column_id, board_id, title, position, priority, status,
                                card_number, prefix, created_at, updated_at)
                 VALUES ('{zzz_id}','{COLUMN}','{BOARD}','Zzz',2,'medium','todo',9,'ZZZ','{TS}','{TS}'),
                        ('{empty_id}','{COLUMN}','{BOARD}','Empty',3,'medium','todo',10,'','{TS}','{TS}');"
        ))
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();

        assert_eq!(
            count(pool, "SELECT COUNT(*) FROM prefixes WHERE name = 'zzz' AND card_counter = 9").await,
            1,
            "the unbacked namespace must be repaired before the FK is added"
        );
        let empty_prefix: String =
            sqlx::query_scalar(&format!("SELECT prefix FROM cards WHERE id = '{empty_id}'"))
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(empty_prefix, "KAN", "the empty prefix must be stamped before the FK is added");

        assert!(fk_row(pool).await.is_some());
        let violations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pragma_foreign_key_check('cards')")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(violations, 0);
    });
}

#[test]
fn test_deleting_a_referenced_namespace_is_rejected_after_the_upgrade() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v12.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v12_with_graph(&path).await;
        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();

        let result = sqlx::query("DELETE FROM prefixes WHERE name = 'kan'")
            .execute(pool)
            .await;
        assert!(
            result.is_err(),
            "deleting a referenced namespace must be rejected"
        );

        assert_eq!(
            count(pool, "SELECT COUNT(*) FROM prefixes WHERE name = 'kan'").await,
            1
        );
        assert_eq!(count(pool, "SELECT COUNT(*) FROM cards").await, 2);
    });
}

#[test]
fn test_a_unicode_cased_prefix_is_healed_by_seeding_the_ascii_folded_row() {
    // Under the old Unicode fold this database was unsatisfiable and the
    // upgrade refused: the card's 'ÄKAN' NOCASE-matches neither the legacy
    // Unicode-folded 'äkan' row (NOCASE folds ASCII only) nor anything the
    // Unicode fold would seed. The ASCII fold seeds 'Äkan', which does
    // NOCASE-match the card, so the upgrade heals instead of refusing.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v12.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v12_with_graph(&path).await;

        let pool = raw(&path).await;
        let unicode_id = "00000000-0000-0000-0000-0000000000a6";
        sqlx::raw_sql(&format!(
            "DELETE FROM cards WHERE id IN ('{CARD1}','{CARD2}');
             DELETE FROM sprint_logs; DELETE FROM archived_cards; DELETE FROM spawns_edges;
             INSERT INTO prefixes (name, card_counter, sprint_counter) VALUES ('\u{e4}kan', 1, 0);
             INSERT INTO cards (id, column_id, board_id, title, position, priority, status,
                                card_number, prefix, created_at, updated_at)
                 VALUES ('{unicode_id}','{COLUMN}','{BOARD}','Umlaut',0,'medium','todo',1,'\u{c4}KAN','{TS}','{TS}');"
        ))
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let store = SqliteStore::open(&path)
            .await
            .expect("the ASCII fold makes the casing satisfiable, so the upgrade must heal");
        let pool = store.pool();

        let fk_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_foreign_key_list('cards') WHERE \"table\" = 'prefixes'",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(fk_count, 1, "the FK must be in place after the healed upgrade");

        assert_eq!(
            count(pool, &format!("SELECT COUNT(*) FROM cards WHERE id='{unicode_id}'")).await,
            1,
            "the card survives with its prefix intact"
        );
        assert_eq!(
            count(pool, "SELECT COUNT(*) FROM prefixes WHERE name = '\u{c4}kan'").await,
            1,
            "the ASCII-folded '\u{c4}kan' row was seeded to back the card"
        );
        assert_eq!(
            count(pool, "SELECT COUNT(*) FROM prefixes WHERE name = '\u{e4}kan'").await,
            1,
            "the legacy Unicode-folded row is left in place, counters intact"
        );

        let violations: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM pragma_foreign_key_check('cards')")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(violations, 0);

        assert!(SqliteStore::backup_path_for(&path, 12).exists());
    });
}
