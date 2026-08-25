//! Whole-graph coverage for the schema 12 -> 13 migration (the `cards`
//! rebuild that backs `prefix` with a foreign key to `prefixes(name)`),
//! matching `kanban-persistence-json/tests/whole_graph_migration.rs` on the
//! SQLite side of the same risk area: `cards` is dropped and rebuilt, so a
//! test that seeds only one card and one edge kind cannot prove the rebuild
//! carries a real graph through intact.
//!
//! Extends the seed/revert pattern from `migration_v12_to_v13.rs` (seed the
//! full graph through a real `SqliteStore::open`, then revert only `cards`
//! to its pre-FK shape) with a second column carrying a WIP limit, a
//! sprint-bound card, and a blocks edge and a relates edge alongside the
//! existing spawns edge.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::path::Path;
use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;

const BOARD: &str = "00000000-0000-0000-0000-0000000000b1";
const COLUMN_TODO: &str = "00000000-0000-0000-0000-0000000000c1";
const COLUMN_DOING: &str = "00000000-0000-0000-0000-0000000000c2";
const SPRINT: &str = "00000000-0000-0000-0000-000000000051";
const CARD1: &str = "00000000-0000-0000-0000-0000000000a1";
const CARD2: &str = "00000000-0000-0000-0000-0000000000a2";
const CARD3: &str = "00000000-0000-0000-0000-0000000000a3";
const CARD4: &str = "00000000-0000-0000-0000-0000000000a4";
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

async fn seed_v12_with_whole_graph(path: &Path) {
    {
        let store = SqliteStore::open(path).await.unwrap();
        let pool = store.pool();
        sqlx::raw_sql(&format!(
            "INSERT INTO boards (id, name, card_prefix, created_at, updated_at)
                 VALUES ('{BOARD}','Board','KAN','{TS}','{TS}');
             INSERT INTO columns (id, board_id, name, position, wip_limit, created_at, updated_at)
                 VALUES ('{COLUMN_TODO}','{BOARD}','Todo',0,NULL,'{TS}','{TS}'),
                        ('{COLUMN_DOING}','{BOARD}','Doing',1,3,'{TS}','{TS}');
             INSERT INTO sprints (id, board_id, sprint_number, status, created_at, updated_at)
                 VALUES ('{SPRINT}','{BOARD}',3,'Planning','{TS}','{TS}');
             INSERT INTO prefixes (name, card_counter, sprint_counter)
                 VALUES ('kan', 4, 1);
             INSERT INTO cards (id, column_id, board_id, title, position, priority, status,
                                card_number, prefix, sprint_id, created_at, updated_at)
                 VALUES ('{CARD1}','{COLUMN_TODO}','{BOARD}','One',0,'medium','todo',1,'KAN',NULL,'{TS}','{TS}'),
                        ('{CARD2}','{COLUMN_TODO}','{BOARD}','Two',1,'medium','todo',2,'KAN','{SPRINT}','{TS}','{TS}'),
                        ('{CARD3}','{COLUMN_DOING}','{BOARD}','Three',0,'medium','todo',3,'KAN','{SPRINT}','{TS}','{TS}'),
                        ('{CARD4}','{COLUMN_DOING}','{BOARD}','Four',1,'medium','todo',4,'KAN',NULL,'{TS}','{TS}');
             INSERT INTO spawns_edges (source_id, target_id, created_at)
                 VALUES ('{CARD1}','{CARD2}','{TS}');
             INSERT INTO blocks_edges (source_id, target_id, severity, created_at)
                 VALUES ('{CARD2}','{CARD3}','High','{TS}');
             INSERT INTO relates_edges (source_id, target_id, kind, created_at)
                 VALUES ('{CARD3}','{CARD4}','Duplicates','{TS}');
             INSERT INTO sprint_logs (card_id, sprint_id, sprint_number, started_at, status)
                 VALUES ('{CARD2}','{SPRINT}',3,'{TS}','Active');
             INSERT INTO archived_cards (card_id, board_id, archived_at, original_column_id, original_position)
                 VALUES ('{CARD4}','{BOARD}','{TS}','{COLUMN_DOING}',1);"
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

/// Not a red step: the migration is believed correct and this is a
/// regression pin over a graph shape this migration's own suite did not
/// previously cover (a second column with a WIP limit, a sprint-bound card,
/// and all three edge kinds rather than just `spawns`). A genuine failure
/// here would be a real defect in the `cards` rebuild, not something to work
/// around in the test.
#[test]
fn test_the_rebuild_preserves_the_whole_graph_across_two_columns_and_every_edge_kind() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v12.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v12_with_whole_graph(&path).await;

        let store = SqliteStore::open(&path).await.unwrap();
        let pool = store.pool();

        let schema_version: i64 =
            sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(schema_version, 13);

        for (what, sql, expected) in [
            ("columns", "SELECT COUNT(*) FROM columns", 2),
            ("cards", "SELECT COUNT(*) FROM cards", 4),
            ("sprints", "SELECT COUNT(*) FROM sprints", 1),
            ("sprint_logs", "SELECT COUNT(*) FROM sprint_logs", 1),
            ("archived_cards", "SELECT COUNT(*) FROM archived_cards", 1),
            ("spawns_edges", "SELECT COUNT(*) FROM spawns_edges", 1),
            ("blocks_edges", "SELECT COUNT(*) FROM blocks_edges", 1),
            ("relates_edges", "SELECT COUNT(*) FROM relates_edges", 1),
        ] {
            assert_eq!(
                count(pool, sql).await,
                expected,
                "{what} did not survive the rebuild"
            );
        }

        let wip_limit: Option<i64> =
            sqlx::query_scalar("SELECT wip_limit FROM columns WHERE id = ?")
                .bind(COLUMN_DOING)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(wip_limit, Some(3), "the Doing column's WIP limit survives");

        for (card_id, expected_position) in [(CARD3, 0i64), (CARD4, 1i64)] {
            let position: i64 = sqlx::query_scalar("SELECT position FROM cards WHERE id = ?")
                .bind(card_id)
                .fetch_one(pool)
                .await
                .unwrap();
            assert_eq!(
                position, expected_position,
                "position of {card_id} survives"
            );
        }

        for card_id in [CARD2, CARD3] {
            let sprint_id: Option<String> =
                sqlx::query_scalar("SELECT sprint_id FROM cards WHERE id = ?")
                    .bind(card_id)
                    .fetch_one(pool)
                    .await
                    .unwrap();
            assert_eq!(
                sprint_id.as_deref(),
                Some(SPRINT),
                "{card_id}'s sprint binding survives"
            );
        }

        let (source, target): (String, String) = sqlx::query_as(
            "SELECT source_id, target_id FROM blocks_edges WHERE source_id = ? AND target_id = ?",
        )
        .bind(CARD2)
        .bind(CARD3)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!((source.as_str(), target.as_str()), (CARD2, CARD3));

        let (source, target): (String, String) = sqlx::query_as(
            "SELECT source_id, target_id FROM relates_edges WHERE source_id = ? AND target_id = ?",
        )
        .bind(CARD3)
        .bind(CARD4)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!((source.as_str(), target.as_str()), (CARD3, CARD4));

        for card_id in [CARD1, CARD2, CARD3, CARD4] {
            let prefix: String = sqlx::query_scalar("SELECT prefix FROM cards WHERE id = ?")
                .bind(card_id)
                .fetch_one(pool)
                .await
                .unwrap();
            assert_eq!(prefix, "KAN", "{card_id} keeps its stamped prefix");
        }
    });
}
