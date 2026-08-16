//! Coverage for the schema 9 -> 10 migration: backfilling the `prefixes`
//! table from a real, binary-generated schema-9 database.
//!
//! The fixture at `tests/fixtures/v9_prefixes.db` was produced by the
//! `kanban` CLI at commit fd1e3ba9072f6c82d7e7d30adb7c3678eb45ca13 (develop,
//! predating this migration) via:
//!
//! ```text
//! kanban FIX init --board Seed
//! kanban FIX board create --name BoardA --card-prefix KAN --with-default-columns
//! kanban FIX board update BoardA --sprint-prefix KAN
//! kanban FIX board create --name BoardB --with-default-columns
//! kanban FIX board create --name BoardC --card-prefix DEV --with-default-columns
//! kanban FIX board update BoardC --sprint-prefix REL
//! kanban FIX sprint create --board BoardA --name "Sprint A1"
//! kanban FIX sprint create --board BoardB --name "Sprint B1"
//! kanban FIX sprint create --board BoardB --name "Sprint B2"
//! kanban FIX sprint update <Sprint B2 id> --card-prefix AUTH
//! kanban FIX card create --board BoardA --column TODO --title A-1 (x3)
//! kanban FIX card create --board BoardB --column TODO --title B-1
//! kanban FIX card create --board BoardB --column TODO --title B-2 --assign <Sprint B2 id>
//! kanban FIX card create --board BoardC --column TODO --title C-1 (x2)
//! ```
//!
//! Board `Seed` and board `BoardB` both leave `card_prefix`/`sprint_prefix`
//! unset, so this fixture carries a REAL (not hand-authored) collision on
//! the default `task`/`sprint` fallbacks: `BoardB`'s id sorts before
//! `Seed`'s, so `BoardB` keeps the base name and `Seed` is bumped.

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;

fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v9_prefixes.db")
}

/// Copies the checked-in fixture into a fresh tempdir so `open()` (which
/// mutates the file in place) never touches the committed asset.
fn open_fixture_copy() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v9.db");
    std::fs::copy(fixture_path(), &path).unwrap();
    (dir, path)
}

async fn card_identifiers(pool: &Pool<Sqlite>) -> Vec<String> {
    // Mirrors the dynamic resolution this migration must not disturb:
    // sprint.card_prefix -> board.card_prefix -> "task", lowercased.
    let rows: Vec<(String, i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT cards.id, cards.card_number, sprints.card_prefix, boards.card_prefix
         FROM cards
         JOIN boards ON boards.id = cards.board_id
         LEFT JOIN sprints ON sprints.id = cards.sprint_id",
    )
    .fetch_all(pool)
    .await
    .unwrap();

    let mut out: Vec<String> = rows
        .into_iter()
        .map(|(card_id, number, sprint_prefix, board_prefix)| {
            let prefix = sprint_prefix
                .or(board_prefix)
                .unwrap_or_else(|| "task".to_string())
                .to_lowercase();
            format!("{card_id}:{prefix}-{number}")
        })
        .collect();
    out.sort();
    out
}

async fn prefix_rows(pool: &Pool<Sqlite>) -> Vec<(String, String, String, i64, i64)> {
    let mut rows: Vec<(String, String, String, i64, i64)> = sqlx::query_as(
        "SELECT name, owner_kind, owner_id, card_counter, sprint_counter FROM prefixes",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    rows.sort();
    rows
}

async fn raw_pool(path: &Path) -> Pool<Sqlite> {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(path))
        .await
        .unwrap()
}

#[test]
fn test_migrate_v9_to_v10_creates_one_row_per_distinct_effective_prefix() {
    let (_dir, path) = open_fixture_copy();
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let rows = prefix_rows(store.pool()).await;
        let names: Vec<&str> = rows.iter().map(|(n, ..)| n.as_str()).collect();
        assert_eq!(
            names,
            vec!["auth", "dev", "kan", "rel", "sprint", "sprint2", "task", "task2"],
            "expected one row per distinct effective card/sprint-naming prefix"
        );
    });
}

#[test]
fn test_migrate_v9_to_v10_merges_equal_card_and_sprint_prefix_into_one_row() {
    let (_dir, path) = open_fixture_copy();
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let rows = prefix_rows(store.pool()).await;
        let kan_rows: Vec<_> = rows.iter().filter(|(n, ..)| n == "kan").collect();
        assert_eq!(
            kan_rows.len(),
            1,
            "BoardA sets card_prefix = sprint_prefix = KAN; must collapse to one row"
        );
        let (_, owner_kind, _, card_counter, sprint_counter) = kan_rows[0];
        assert_eq!(owner_kind, "board");
        assert_eq!(*card_counter, 4, "BoardA created 3 cards off card_counter starting at 1");
        assert_eq!(*sprint_counter, 2, "BoardA created 1 sprint off a counter starting at 1");
    });
}

#[test]
fn test_migrate_v9_to_v10_creates_two_rows_when_card_and_sprint_prefix_differ() {
    let (_dir, path) = open_fixture_copy();
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let rows = prefix_rows(store.pool()).await;

        let dev = rows.iter().find(|(n, ..)| n == "dev").unwrap();
        assert_eq!(dev.1, "board");
        assert_eq!(dev.3, 3, "BoardC created 2 cards off card_counter starting at 1");
        assert_eq!(dev.4, 0, "the card-prefix row carries no sprint counter");

        let rel = rows.iter().find(|(n, ..)| n == "rel").unwrap();
        assert_eq!(rel.1, "board");
        assert_eq!(rel.3, 0, "the sprint-naming row carries no card counter");
        assert_eq!(
            rel.4, 0,
            "BoardC created no sprints, so its REL naming row starts at 0"
        );
        assert_eq!(dev.2, rel.2, "both rows are owned by the same board");
    });
}

#[test]
fn test_migrate_v9_to_v10_preserves_card_counter_value() {
    let (_dir, path) = open_fixture_copy();
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let board_counters: Vec<(String, i64)> =
            sqlx::query_as("SELECT id, card_counter FROM boards")
                .fetch_all(store.pool())
                .await
                .unwrap();

        let rows = prefix_rows(store.pool()).await;
        for (board_id, counter) in board_counters {
            let matching: Vec<_> = rows
                .iter()
                .filter(|(_, owner_kind, owner_id, ..)| owner_kind == "board" && owner_id == &board_id)
                .collect();
            let total: i64 = matching.iter().map(|(_, _, _, c, _)| *c).sum();
            assert_eq!(
                total, counter,
                "board {board_id}'s card_counter must be preserved exactly on its prefixes row(s)"
            );
        }
    });
}

#[test]
fn test_migrate_v9_to_v10_preserves_sprint_counter_value() {
    let (_dir, path) = open_fixture_copy();
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let counters: Vec<(String, String, i64)> =
            sqlx::query_as("SELECT board_id, prefix, counter FROM board_sprint_counters")
                .fetch_all(store.pool())
                .await
                .unwrap();
        assert!(!counters.is_empty(), "fixture must exercise this table");

        let rows = prefix_rows(store.pool()).await;
        for (board_id, _prefix, counter) in counters {
            let total: i64 = rows
                .iter()
                .filter(|(_, owner_kind, owner_id, ..)| owner_kind == "board" && owner_id == &board_id)
                .map(|(_, _, _, _, s)| *s)
                .sum();
            assert_eq!(
                total, counter,
                "board {board_id}'s sprint counter must be preserved exactly on its prefixes row(s)"
            );
        }
    });
}

#[test]
fn test_migrate_v9_to_v10_increments_on_default_prefix_collision() {
    let (_dir, path) = open_fixture_copy();
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let rows = prefix_rows(store.pool()).await;

        let boards: Vec<(String, String)> = sqlx::query_as("SELECT id, name FROM boards")
            .fetch_all(store.pool())
            .await
            .unwrap();
        let board_b = boards.iter().find(|(_, n)| n == "BoardB").unwrap().0.clone();
        let seed = boards.iter().find(|(_, n)| n == "Seed").unwrap().0.clone();

        let task = rows.iter().find(|(n, ..)| n == "task").unwrap();
        let task2 = rows.iter().find(|(n, ..)| n == "task2").unwrap();
        assert_eq!(task.2, board_b, "the lexicographically-first owner keeps the base name");
        assert_eq!(task2.2, seed, "the other colliding owner is bumped to task2");

        let sprint_row = rows.iter().find(|(n, ..)| n == "sprint").unwrap();
        let sprint2_row = rows.iter().find(|(n, ..)| n == "sprint2").unwrap();
        assert_eq!(sprint_row.2, board_b);
        assert_eq!(sprint2_row.2, seed);
    });
}

#[test]
fn test_migrate_v9_to_v10_fails_loud_on_explicit_prefix_collision() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("collision.db");
    let rt = make_rt();
    rt.block_on(async {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(&path).create_if_missing(true))
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE metadata (
                id INTEGER PRIMARY KEY CHECK (id = 1), instance_id TEXT NOT NULL,
                saved_at TEXT NOT NULL, schema_version INTEGER NOT NULL,
                writer_version TEXT, writer_commit TEXT
            );
            CREATE TABLE boards (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT,
                sprint_prefix TEXT, card_prefix TEXT,
                task_sort_field TEXT NOT NULL DEFAULT 'Default',
                task_sort_order TEXT NOT NULL DEFAULT 'Ascending',
                sprint_duration_days INTEGER,
                sprint_name_used_count INTEGER NOT NULL DEFAULT 0,
                next_sprint_number INTEGER NOT NULL DEFAULT 1,
                active_sprint_id TEXT, task_list_view TEXT NOT NULL DEFAULT 'Flat',
                card_counter INTEGER NOT NULL DEFAULT 1, position INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL
            );
            CREATE TABLE columns (
                id TEXT PRIMARY KEY, board_id TEXT NOT NULL, name TEXT NOT NULL,
                position INTEGER NOT NULL, wip_limit INTEGER, default_status TEXT,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL
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

        sqlx::query(
            "INSERT INTO metadata (id, instance_id, saved_at, schema_version)
             VALUES (1, ?, '2024-01-01T00:00:00Z', 9)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .execute(&pool)
        .await
        .unwrap();

        for name in ["Board1", "Board2"] {
            sqlx::query(
                "INSERT INTO boards (id, name, card_prefix, created_at, updated_at)
                 VALUES (?, ?, 'DUP', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
        }
        pool.close().await;

        let result = SqliteStore::open(&path).await;
        assert!(
            result.is_err(),
            "two boards explicitly set to the same prefix must abort open()"
        );
        let message = result.err().unwrap().to_string();
        assert!(
            message.contains("dup"),
            "error must name the colliding prefix: {message}"
        );
    });
}

#[test]
fn test_migrate_v9_to_v10_writes_a_v9_backup() {
    let (_dir, path) = open_fixture_copy();
    let rt = make_rt();
    rt.block_on(async {
        SqliteStore::open(&path).await.unwrap();

        let backup = SqliteStore::backup_path_for(&path, 9);
        assert!(backup.exists(), "expected a <path>.v9.backup file");

        let backup_pool = raw_pool(&backup).await;
        let has_prefixes: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='prefixes'",
        )
        .fetch_one(&backup_pool)
        .await
        .unwrap();
        assert!(
            !has_prefixes,
            "the backup must predate the prefixes backfill (schema-9 shape has no rows in it yet)"
        );
        backup_pool.close().await;
    });
}

#[test]
fn test_migrate_v9_to_v10_bumps_schema_version_to_10() {
    let (_dir, path) = open_fixture_copy();
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(version, 10);
    });
}

#[test]
fn test_boards_card_counter_and_board_sprint_counters_unchanged_by_migration() {
    let (_dir, path) = open_fixture_copy();
    let rt = make_rt();

    let before_pool = rt.block_on(raw_pool(&path));
    let boards_before: Vec<(String, i64)> = rt.block_on(
        sqlx::query_as("SELECT id, card_counter FROM boards ORDER BY id").fetch_all(&before_pool),
    )
    .unwrap();
    let counters_before: Vec<(String, String, i64)> = rt.block_on(
        sqlx::query_as("SELECT board_id, prefix, counter FROM board_sprint_counters ORDER BY board_id, prefix")
            .fetch_all(&before_pool),
    )
    .unwrap();
    rt.block_on(before_pool.close());

    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let boards_after: Vec<(String, i64)> =
            sqlx::query_as("SELECT id, card_counter FROM boards ORDER BY id")
                .fetch_all(store.pool())
                .await
                .unwrap();
        let counters_after: Vec<(String, String, i64)> = sqlx::query_as(
            "SELECT board_id, prefix, counter FROM board_sprint_counters ORDER BY board_id, prefix",
        )
        .fetch_all(store.pool())
        .await
        .unwrap();

        assert_eq!(boards_before, boards_after, "boards.card_counter must be untouched");
        assert_eq!(
            counters_before, counters_after,
            "board_sprint_counters must be untouched"
        );
    });
}

/// Every existing card's dynamically-resolved identifier (the OLD
/// resolution: sprint.card_prefix -> board.card_prefix -> default,
/// unaffected by this migration since nothing yet reads `prefixes`) must be
/// identical before and after `open()` runs the migration.
#[test]
fn test_migrate_v9_to_v10_preserves_every_cards_dynamically_resolved_identifier() {
    let (_dir, path) = open_fixture_copy();
    let rt = make_rt();

    let before_pool = rt.block_on(raw_pool(&path));
    let before = rt.block_on(card_identifiers(&before_pool));
    rt.block_on(before_pool.close());
    assert!(!before.is_empty(), "fixture must contain cards");

    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let after = card_identifiers(store.pool()).await;
        assert_eq!(
            before, after,
            "no card's identifier may change as a side effect of this additive migration"
        );
    });
}
