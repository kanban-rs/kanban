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

async fn prefix_rows(pool: &Pool<Sqlite>) -> Vec<(String, i64, i64)> {
    let mut rows: Vec<(String, i64, i64)> =
        sqlx::query_as("SELECT name, card_counter, sprint_counter FROM prefixes")
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
            vec!["auth", "dev", "kan", "rel", "sprint", "task"],
            "one row per DISTINCT prefix: Seed and BoardB both fall back to the \
             default, so they share `task` and `sprint` rather than one being renamed"
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
        let (_, card_counter, sprint_counter) = kan_rows[0];
        assert_eq!(
            *card_counter, 4,
            "BoardA created 3 cards off card_counter starting at 1"
        );
        assert_eq!(
            *sprint_counter, 2,
            "BoardA created 1 sprint off a counter starting at 1"
        );
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
        assert_eq!(
            dev.1, 3,
            "BoardC created 2 cards off card_counter starting at 1"
        );
        assert_eq!(dev.2, 0, "the card-prefix row carries no sprint counter");

        let rel = rows.iter().find(|(n, ..)| n == "rel").unwrap();
        assert_eq!(rel.1, 0, "the sprint-naming row carries no card counter");
        assert_eq!(
            rel.2, 0,
            "BoardC created no sprints, so its REL naming row starts at 0"
        );
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

        let prefixes: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT id, card_prefix FROM boards")
                .fetch_all(store.pool())
                .await
                .unwrap();

        let rows = prefix_rows(store.pool()).await;
        for (board_id, counter) in board_counters {
            let name = prefixes
                .iter()
                .find(|(id, _)| id == &board_id)
                .and_then(|(_, p)| p.clone())
                .unwrap_or_else(|| "task".to_string())
                .to_lowercase();
            let row = rows.iter().find(|(n, ..)| n == &name).unwrap();
            assert!(
                row.1 >= counter,
                "board {board_id} allocates from `{name}`, whose counter ({}) must be at \
                 least its own ({counter}) -- a shared counter is a high-water mark, and \
                 starting below one would re-mint a number an existing card carries",
                row.1
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
        for (board_id, prefix, counter) in counters {
            let name = prefix.to_lowercase();
            let row = rows
                .iter()
                .find(|(n, ..)| n == &name)
                .unwrap_or_else(|| panic!("no row for sprint namespace {name}"));
            assert!(
                row.2 >= counter,
                "board {board_id}'s sprint namespace `{name}` must carry a counter ({}) \
                 at least as high as the board's own ({counter})",
                row.2
            );
        }
    });
}

#[test]
fn test_migrate_v9_to_v10_shares_one_row_between_boards_without_a_prefix() {
    let (_dir, path) = open_fixture_copy();
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let rows = prefix_rows(store.pool()).await;

        // Seed and BoardB both leave card_prefix/sprint_prefix unset, so both
        // already hand out `task`/`sprint` before the migration. Renaming one
        // could not repair the identifiers its existing cards carry -- those
        // are stored -- and would only split that board across two namespaces.
        assert_eq!(
            rows.iter().filter(|(n, ..)| n.starts_with("task")).count(),
            1,
            "the two defaulting boards share one `task` namespace: {rows:?}"
        );
        assert_eq!(
            rows.iter()
                .filter(|(n, ..)| n.starts_with("sprint"))
                .count(),
            1,
            "and one `sprint` namespace: {rows:?}"
        );

        let board_counters: Vec<(i64,)> =
            sqlx::query_as("SELECT card_counter FROM boards WHERE card_prefix IS NULL")
                .fetch_all(store.pool())
                .await
                .unwrap();
        let highest = board_counters.iter().map(|(c,)| *c).max().unwrap();
        let task = rows.iter().find(|(n, ..)| n == "task").unwrap();
        assert_eq!(
            task.1, highest,
            "the shared counter is the high-water mark across both boards"
        );
    });
}

#[test]
fn test_migrate_v9_to_v10_shares_one_row_between_boards_explicitly_given_one_prefix() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("collision.db");
    let rt = make_rt();
    rt.block_on(async {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
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

        let store = SqliteStore::open(&path)
            .await
            .expect("two boards deliberately set to one prefix is legal, not an error");
        let rows = prefix_rows(store.pool()).await;

        assert_eq!(
            rows.iter().filter(|(n, ..)| n == "dup").count(),
            1,
            "both boards asked for `dup`; they get one shared namespace: {rows:?}"
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
        assert_eq!(version, 11);
    });
}

#[test]
fn test_boards_card_counter_and_board_sprint_counters_unchanged_by_migration() {
    let (_dir, path) = open_fixture_copy();
    let rt = make_rt();

    let before_pool = rt.block_on(raw_pool(&path));
    let boards_before: Vec<(String, i64)> = rt
        .block_on(
            sqlx::query_as("SELECT id, card_counter FROM boards ORDER BY id")
                .fetch_all(&before_pool),
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

        assert_eq!(
            boards_before, boards_after,
            "boards.card_counter must be untouched"
        );
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

/// Two boards that never chose a prefix, each already carrying cards
/// numbered from 1. Their identifiers ALREADY collide before this migration
/// runs -- that is the defect the prefixes table exists to stop repeating,
/// and it is precisely the data a real tracker has today.
///
/// The migration must accept it, not reject it and not repair it. Repairing
/// would mean renumbering cards or renaming a board's prefix, and a card's
/// prefix and number are what a user has already put in branch names and
/// links. The collision is history; the fix is that no NEW one can be minted.
#[test]
fn test_migrate_v9_to_v10_accepts_existing_cross_board_duplicates_without_renumbering() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("dupes.db");
    let rt = make_rt();
    rt.block_on(async {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
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
                card_prefix TEXT, start_date TEXT, end_date TEXT,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL
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

        // Board A: 5 cards. Board B: 3 cards. Both unprefixed, both numbering
        // from 1, so task-1..task-3 each name two different cards.
        let board_a = "11111111-1111-1111-1111-111111111111";
        let board_b = "22222222-2222-2222-2222-222222222222";
        for (board, count, counter) in [(board_a, 5, 6), (board_b, 3, 4)] {
            sqlx::query(
                "INSERT INTO boards (id, name, card_prefix, card_counter, created_at, updated_at)
                 VALUES (?, ?, NULL, ?, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
            )
            .bind(board)
            .bind(format!("Board {board}"))
            .bind(counter)
            .execute(&pool)
            .await
            .unwrap();
            for n in 1..=count {
                sqlx::query(
                    "INSERT INTO cards (id, column_id, board_id, title, position, card_number,
                                        created_at, updated_at)
                     VALUES (?, 'col', ?, ?, ?, ?, '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
                )
                .bind(uuid::Uuid::new_v4().to_string())
                .bind(board)
                .bind(format!("card {n}"))
                .bind(n)
                .bind(n)
                .execute(&pool)
                .await
                .unwrap();
            }
        }
        pool.close().await;

        let before_pool = raw_pool(&path).await;
        let before = card_identifiers(&before_pool).await;
        before_pool.close().await;

        let duplicated: Vec<&String> = before.iter().filter(|id| id.ends_with(":task-1")).collect();
        assert_eq!(
            duplicated.len(),
            2,
            "the fixture must genuinely contain a cross-board duplicate: {before:?}"
        );

        let store = SqliteStore::open(&path)
            .await
            .expect("a store with pre-existing duplicate identifiers must still open");

        let after = card_identifiers(store.pool()).await;
        assert_eq!(
            before, after,
            "no card may be renumbered: the identifiers users already reference are history, \
             and the migration is additive"
        );

        let rows = prefix_rows(store.pool()).await;
        let task = rows.iter().find(|(n, ..)| n == "task").unwrap();
        assert_eq!(
            task.1, 6,
            "the shared counter is the high-water mark across both boards (6, not 4 and not \
             10), so the next card minted from either board is task-6 -- a number neither \
             board has used"
        );
    });
}
