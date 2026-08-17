//! Schema 11 -> 12: drop `boards.card_counter` and the
//! `board_sprint_counters` table.
//!
//! The headline test here is not the column being gone -- that is one
//! `pragma_table_info` query. It is that the BOARD SUBTREE SURVIVES.
//!
//! SQLite cannot drop a column in place at the version this project targets,
//! so `boards` is rebuilt and swapped. Every child table references
//! `boards(id)` with `ON DELETE CASCADE`, so performing that swap with foreign
//! keys enforced would fire the cascade on `DROP TABLE boards` and take every
//! column, card, sprint and edge with it. That is the KAN-863 failure mode: a
//! migration that returns Ok and leaves an empty workspace.

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

async fn count(pool: &Pool<Sqlite>, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

/// Builds a v11-shaped database holding a NON-TRIVIAL graph, then reverts the
/// schema markers so opening it triggers the 11 -> 12 migration.
///
/// Seeded through a real `SqliteStore::open` first so the graph is written by
/// the production code paths rather than hand-rolled SQL that could drift from
/// the real shape.
async fn seed_v11_with_graph(path: &Path) {
    {
        let store = SqliteStore::open(path).await.unwrap();
        let pool = store.pool();
        sqlx::raw_sql(
            "INSERT INTO boards (id, name, card_prefix, created_at, updated_at)
                 VALUES ('00000000-0000-0000-0000-0000000000b1','Board','KAN','2024-01-01T00:00:00Z','2024-01-01T00:00:00Z');
             INSERT INTO columns (id, board_id, name, position, wip_limit, created_at, updated_at)
                 VALUES ('00000000-0000-0000-0000-0000000000c1','00000000-0000-0000-0000-0000000000b1','Todo',0,7,'2024-01-01T00:00:00Z','2024-01-01T00:00:00Z'),
                        ('00000000-0000-0000-0000-0000000000c2','00000000-0000-0000-0000-0000000000b1','Done',1,NULL,'2024-01-01T00:00:00Z','2024-01-01T00:00:00Z');
             INSERT INTO sprints (id, board_id, sprint_number, status, created_at, updated_at)
                 VALUES ('00000000-0000-0000-0000-000000000051','00000000-0000-0000-0000-0000000000b1',3,'Planning','2024-01-01T00:00:00Z','2024-01-01T00:00:00Z');
             INSERT INTO cards (id, column_id, board_id, title, position, priority, status,
                                card_number, prefix, sprint_id, created_at, updated_at)
                 VALUES ('00000000-0000-0000-0000-0000000000a1','00000000-0000-0000-0000-0000000000c1','00000000-0000-0000-0000-0000000000b1','One',0,'medium','todo',1,'KAN','00000000-0000-0000-0000-000000000051',
                         '2024-01-01T00:00:00Z','2024-01-01T00:00:00Z'),
                        ('00000000-0000-0000-0000-0000000000a2','00000000-0000-0000-0000-0000000000c1','00000000-0000-0000-0000-0000000000b1','Two',1,'medium','todo',2,'KAN',NULL,
                         '2024-01-01T00:00:00Z','2024-01-01T00:00:00Z'),
                        ('00000000-0000-0000-0000-0000000000a3','00000000-0000-0000-0000-0000000000c2','00000000-0000-0000-0000-0000000000b1','Three',0,'medium','todo',3,'KAN',NULL,
                         '2024-01-01T00:00:00Z','2024-01-01T00:00:00Z');
             INSERT INTO spawns_edges (source_id, target_id, created_at)
                 VALUES ('00000000-0000-0000-0000-0000000000a1','00000000-0000-0000-0000-0000000000a2','2024-01-01T00:00:00Z');
             INSERT INTO prefixes (name, card_counter, sprint_counter)
                 VALUES ('kan', 3, 3);",
        )
        .execute(pool)
        .await
        .unwrap();
    }

    // Revert to the v11 shape: the legacy column and table back, version down.
    let pool = raw(path).await;
    // Conditional so this fixture builds a v11 shape whether or not the
    // current schema still declares the column.
    let has_col = count(
        &pool,
        "SELECT COUNT(*) FROM pragma_table_info('boards') WHERE name = 'card_counter'",
    )
    .await;
    if has_col == 0 {
        sqlx::raw_sql("ALTER TABLE boards ADD COLUMN card_counter INTEGER NOT NULL DEFAULT 1")
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS board_sprint_counters (
             board_id TEXT NOT NULL,
             prefix   TEXT NOT NULL,
             counter  INTEGER NOT NULL,
             PRIMARY KEY (board_id, prefix),
             FOREIGN KEY (board_id) REFERENCES boards(id) ON DELETE CASCADE
         );
         UPDATE boards SET card_counter = 4;
         INSERT INTO board_sprint_counters (board_id, prefix, counter) VALUES ('00000000-0000-0000-0000-0000000000b1','KAN',4);
         UPDATE metadata SET schema_version = 11;",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;
}

#[test]
fn test_migrate_v11_to_v12_drops_the_legacy_column_and_table() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v11.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v11_with_graph(&path).await;
        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();

        assert_eq!(
            count(
                pool,
                "SELECT COUNT(*) FROM pragma_table_info('boards') WHERE name = 'card_counter'"
            )
            .await,
            0,
            "boards.card_counter must be gone"
        );
        assert_eq!(
            count(
                pool,
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='board_sprint_counters'"
            )
            .await,
            0,
            "board_sprint_counters must be gone"
        );
    });
}

/// The one that matters. Rebuilding `boards` with foreign keys enforced fires
/// ON DELETE CASCADE across every child table; the migration would return Ok
/// having emptied the workspace.
#[test]
fn test_migrate_v11_to_v12_preserves_the_whole_board_subtree() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v11.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v11_with_graph(&path).await;
        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();

        for (what, sql, expected) in [
            ("boards", "SELECT COUNT(*) FROM boards", 1),
            ("columns", "SELECT COUNT(*) FROM columns", 2),
            ("cards", "SELECT COUNT(*) FROM cards", 3),
            ("sprints", "SELECT COUNT(*) FROM sprints", 1),
            ("spawns edges", "SELECT COUNT(*) FROM spawns_edges", 1),
            ("prefix rows", "SELECT COUNT(*) FROM prefixes", 1),
        ] {
            assert_eq!(
                count(pool, sql).await,
                expected,
                "{what} did not survive the boards rebuild -- a cascade wiped the subtree"
            );
        }

        // Not just row counts: the surviving rows must be the SAME rows.
        assert_eq!(
            count(pool, "SELECT COUNT(*) FROM columns WHERE id='00000000-0000-0000-0000-0000000000c1' AND wip_limit=7").await,
            1,
            "the column's WIP limit was lost"
        );
        assert_eq!(
            count(pool, "SELECT COUNT(*) FROM cards WHERE id='00000000-0000-0000-0000-0000000000a1' AND sprint_id='00000000-0000-0000-0000-000000000051' AND card_number=1 AND prefix='KAN'").await,
            1,
            "the card's sprint binding, number or prefix was lost"
        );
        assert_eq!(
            count(pool, "SELECT COUNT(*) FROM boards WHERE id='00000000-0000-0000-0000-0000000000b1' AND card_prefix='KAN' AND name='Board'").await,
            1,
            "the board's own columns were not carried across the rebuild"
        );
    });
}

/// The counters the prefix row inherited must be untouched by their removal,
/// or numbering restarts and re-mints identifiers that already exist.
#[test]
fn test_migrate_v11_to_v12_leaves_the_prefix_row_counters_intact() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v11.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v11_with_graph(&path).await;
        let store = SqliteStore::open(&path).await.unwrap();
        let row: (i64, i64) =
            sqlx::query_as("SELECT card_counter, sprint_counter FROM prefixes WHERE name='kan'")
                .fetch_one(store.pool())
                .await
                .unwrap();
        assert_eq!(
            row,
            (3, 3),
            "dropping the legacy counters must not disturb the row that replaced them"
        );
    });
}

/// Re-opening an already-migrated database must not re-run the rebuild.
#[test]
fn test_migrate_v11_to_v12_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v11.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v11_with_graph(&path).await;
        SqliteStore::open(&path).await.unwrap();
        let store = SqliteStore::open(&path).await.unwrap();
        assert_eq!(
            count(store.pool(), "SELECT COUNT(*) FROM cards").await,
            3,
            "a second open must be a no-op, not another rebuild"
        );
    });
}

/// The `prefixes` rows are the sole source of card and sprint numbering. A
/// snapshot that drops them silently restarts every namespace at 1, which
/// re-mints identifiers that already exist on cards in the same file.
///
/// This is the JSON -> SQLite `kanban migrate` path, and the SQLite save/load
/// path generally. It was masked until schema 12: `migrate_v9_to_v10_prefixes`
/// re-seeded the table from `boards.card_counter` on every open, so losing the
/// rows in a round-trip repaired itself. Dropping that column removes the
/// safety net, so the round-trip has to carry them for real.
#[test]
fn test_snapshot_round_trip_preserves_prefix_rows() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("rt.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        sqlx::raw_sql(
            "INSERT INTO prefixes (name, card_counter, sprint_counter)
             VALUES ('kan', 1258, 22), ('auth', 4, 1);",
        )
        .execute(store.pool())
        .await
        .unwrap();

        let snap = store.snapshot_async().await.unwrap();
        assert_eq!(
            snap.prefixes.len(),
            2,
            "reading a snapshot must carry the prefix rows"
        );

        let target = dir.path().join("rt2.db");
        let store2 = SqliteStore::open(&target).await.unwrap();
        store2.apply_snapshot_async(snap).await.unwrap();

        let rows: Vec<(String, i64, i64)> =
            sqlx::query_as("SELECT name, card_counter, sprint_counter FROM prefixes ORDER BY name")
                .fetch_all(store2.pool())
                .await
                .unwrap();
        assert_eq!(
            rows,
            vec![("auth".to_string(), 4, 1), ("kan".to_string(), 1258, 22),],
            "writing a snapshot must carry the prefix rows, counters included"
        );
    });
}

/// A database written by the released binary's `kanban migrate <json> sqlite`
/// has `boards.card_counter` set and `prefixes` EMPTY -- the full-snapshot
/// write dropped the rows. Opening it here must repair the numbering, not
/// finish the job by dropping the only remaining copy of it.
///
/// The repair is purely an ordering property: 9 -> 10 seeds the prefix rows
/// from `card_counter`, and only then does 11 -> 12 drop the column. Reorder
/// those two and every such database silently restarts at 1, re-minting
/// identifiers that already exist. That is why this is pinned.
#[test]
fn test_a_v11_db_with_empty_prefixes_recovers_its_numbering_before_the_drop() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("stale.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v11_with_graph(&path).await;

        // The state the buggy write path left behind: counters on the board,
        // nothing in `prefixes`.
        let pool = raw(&path).await;
        sqlx::raw_sql("DELETE FROM prefixes; UPDATE boards SET card_counter = 42;")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;

        let store = SqliteStore::open(&path).await.unwrap();

        let row: (i64,) = sqlx::query_as("SELECT card_counter FROM prefixes WHERE name = 'kan'")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(
            row.0, 41,
            "the prefix row must be seeded from the legacy counter before it is \
             dropped; 42 was the next number to hand out, so 41 was the last used"
        );
    });
}
