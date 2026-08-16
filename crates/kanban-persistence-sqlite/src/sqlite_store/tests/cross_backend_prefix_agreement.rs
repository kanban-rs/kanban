//! Executes BOTH the SQLite v9 -> v10 and the JSON V14 -> V15 prefix
//! backfills, live and in one process, over a single shared scenario, and
//! asserts the two produce the same `prefixes` content.
//!
//! The per-backend migration suites each pin their own side against a
//! hand-recorded expectation of what the other side "would" do. That is not
//! agreement: a recorded constant cannot notice the other backend changing,
//! and no fixture on either side happened to exercise the shapes where they
//! actually differ. This module removes the recording and runs both.
//!
//! Both backends are driven from one `Scenario` so a shape can only be added
//! to the comparison for both at once.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;

/// One board's pre-migration prefix state, in backend-neutral terms.
struct BoardSpec {
    id: &'static str,
    name: &'static str,
    card_prefix: Option<&'static str>,
    sprint_prefix: Option<&'static str>,
    card_counter: i64,
    /// Per-prefix sprint counters, exactly as both backends really store
    /// them: SQLite in `board_sprint_counters(board_id, prefix, counter)`,
    /// JSON in the board's `sprint_counters` map.
    sprint_counters: Vec<(&'static str, i64)>,
}

/// A sprint carrying its own prefix override.
struct SprintSpec {
    id: &'static str,
    board_id: &'static str,
    card_prefix: Option<&'static str>,
}

struct Scenario {
    boards: Vec<BoardSpec>,
    sprints: Vec<SprintSpec>,
}

/// A backfilled prefix row: a namespace and its counters. Neither backend
/// records an owner -- several boards may share one row.
type Row = (String, i64, i64);

const V9_SCHEMA: &str = "
    CREATE TABLE metadata (
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
    CREATE TABLE board_sprint_counters (
        board_id TEXT NOT NULL, prefix TEXT NOT NULL, counter INTEGER NOT NULL,
        PRIMARY KEY (board_id, prefix)
    );
    CREATE TABLE sprints (
        id TEXT PRIMARY KEY, board_id TEXT NOT NULL, name TEXT NOT NULL,
        number INTEGER NOT NULL, status TEXT NOT NULL DEFAULT 'Planned',
        card_prefix TEXT, start_date TEXT, end_date TEXT,
        created_at TEXT NOT NULL, updated_at TEXT NOT NULL
    );
";

const TS: &str = "2024-01-01T00:00:00Z";

async fn build_v9_db(scenario: &Scenario, path: &std::path::Path) -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::raw_sql(V9_SCHEMA).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO metadata (id, instance_id, saved_at, schema_version)
         VALUES (1, '00000000-0000-0000-0000-000000000001', ?, 9)",
    )
    .bind(TS)
    .execute(&pool)
    .await
    .unwrap();

    for b in &scenario.boards {
        sqlx::query(
            "INSERT INTO boards (id, name, card_prefix, sprint_prefix, card_counter,
                                 created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(b.id)
        .bind(b.name)
        .bind(b.card_prefix)
        .bind(b.sprint_prefix)
        .bind(b.card_counter)
        .bind(TS)
        .bind(TS)
        .execute(&pool)
        .await
        .unwrap();
        for (prefix, counter) in &b.sprint_counters {
            sqlx::query(
                "INSERT INTO board_sprint_counters (board_id, prefix, counter)
                 VALUES (?, ?, ?)",
            )
            .bind(b.id)
            .bind(*prefix)
            .bind(*counter)
            .execute(&pool)
            .await
            .unwrap();
        }
    }
    for (i, s) in scenario.sprints.iter().enumerate() {
        sqlx::query(
            "INSERT INTO sprints (id, board_id, name, number, card_prefix, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(s.id)
        .bind(s.board_id)
        .bind(format!("Sprint {i}"))
        .bind(i as i64 + 1)
        .bind(s.card_prefix)
        .bind(TS)
        .bind(TS)
        .execute(&pool)
        .await
        .unwrap();
    }
    pool
}

async fn sqlite_rows(scenario: &Scenario) -> Result<Vec<Row>, String> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v9.db");
    // Seed at schema 9, then drive the real entry point: `open` lays down the
    // full DDL (the `prefixes` table among it) before running the migrate
    // chain, so calling the backfill against a bare v9 pool would find no
    // table to write into.
    build_v9_db(scenario, &path).await.close().await;
    let store = SqliteStore::open(&path).await.map_err(|e| e.to_string())?;
    let mut rows: Vec<Row> =
        sqlx::query_as("SELECT name, card_counter, sprint_counter FROM prefixes")
            .fetch_all(store.pool())
            .await
            .unwrap();
    rows.sort();
    Ok(rows)
}

fn json_envelope(scenario: &Scenario) -> serde_json::Value {
    let boards: Vec<serde_json::Value> = scenario
        .boards
        .iter()
        .map(|b| {
            serde_json::json!({
                "id": b.id,
                "name": b.name,
                "card_prefix": b.card_prefix,
                "sprint_prefix": b.sprint_prefix,
                "card_counter": b.card_counter,
                "sprint_counters": b.sprint_counters
                    .iter()
                    .map(|(p, c)| ((*p).to_string(), serde_json::json!(c)))
                    .collect::<serde_json::Map<String, serde_json::Value>>(),
            })
        })
        .collect();
    let sprints: Vec<serde_json::Value> = scenario
        .sprints
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id,
                "board_id": s.board_id,
                "card_prefix": s.card_prefix,
            })
        })
        .collect();
    serde_json::json!({
        "version": 14,
        "metadata": {
            "instance_id": "00000000-0000-0000-0000-000000000001",
            "saved_at": TS
        },
        "data": {
            "boards": boards,
            "columns": [], "cards": [], "archived_cards": [],
            "sprints": sprints,
            "graph": {
                "spawns": { "edges": [] },
                "blocks": { "edges": [] },
                "relates": { "edges": [] }
            }
        }
    })
}

fn json_rows(scenario: &Scenario) -> Result<Vec<Row>, String> {
    let mut env = json_envelope(scenario);
    kanban_persistence_json::migration_test_support::try_transform_v14_to_v15(&mut env)
        .map_err(|e| e.to_string())?;
    let mut rows: Vec<Row> = env["data"]["prefixes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["name"].as_str().unwrap().to_string(),
                r["card_counter"].as_i64().unwrap(),
                r["sprint_counter"].as_i64().unwrap(),
            )
        })
        .collect();
    rows.sort();
    Ok(rows)
}

/// Runs both migrations over `scenario` and asserts they agree, on success
/// AND on failure. A backend that rejects input the other silently accepts
/// is a divergence even though neither "lost" data.
fn assert_backends_agree(scenario: &Scenario, what: &str) {
    let rt = make_rt();
    let sqlite = rt.block_on(sqlite_rows(scenario));
    let json = json_rows(scenario);

    match (&sqlite, &json) {
        (Ok(s), Ok(j)) => assert_eq!(
            s, j,
            "{what}: both backfills succeeded but produced different prefixes.\n\
             sqlite: {s:#?}\njson:   {j:#?}"
        ),
        (Err(se), Err(je)) => {
            let _ = (se, je);
        }
        (Ok(s), Err(je)) => panic!(
            "{what}: the SQLite backfill succeeded but the JSON one rejected the \
             same input.\nsqlite: {s:#?}\njson error: {je}"
        ),
        (Err(se), Ok(j)) => panic!(
            "{what}: the JSON backfill succeeded but the SQLite one rejected the \
             same input.\njson: {j:#?}\nsqlite error: {se}"
        ),
    }
}

fn board(
    id: &'static str,
    name: &'static str,
    card_prefix: Option<&'static str>,
    sprint_prefix: Option<&'static str>,
) -> BoardSpec {
    BoardSpec {
        id,
        name,
        card_prefix,
        sprint_prefix,
        card_counter: 0,
        sprint_counters: vec![],
    }
}

const B1: &str = "11111111-1111-1111-1111-111111111111";
const B2: &str = "22222222-2222-2222-2222-222222222222";
const B3: &str = "33333333-3333-3333-3333-333333333333";
const B4: &str = "44444444-4444-4444-4444-444444444444";
const S1: &str = "aaaaaaaa-1111-1111-1111-111111111111";

#[test]
fn test_both_backfills_agree_on_an_explicit_prefix() {
    assert_backends_agree(
        &Scenario {
            boards: vec![board(B1, "Alpha", Some("KAN"), Some("KAN"))],
            sprints: vec![],
        },
        "a single board with an explicit prefix",
    );
}

#[test]
fn test_both_backfills_agree_when_card_and_sprint_prefixes_differ() {
    assert_backends_agree(
        &Scenario {
            boards: vec![board(B1, "Beta", Some("DEV"), Some("REL"))],
            sprints: vec![],
        },
        "a board whose card and sprint prefixes differ",
    );
}

#[test]
fn test_both_backfills_agree_on_a_sprint_prefix_override() {
    assert_backends_agree(
        &Scenario {
            boards: vec![board(B1, "Alpha", Some("KAN"), Some("KAN"))],
            sprints: vec![SprintSpec {
                id: S1,
                board_id: B1,
                card_prefix: Some("AUTH"),
            }],
        },
        "a sprint overriding its board's prefix",
    );
}

#[test]
fn test_both_backfills_agree_on_counter_preservation() {
    let scenario = Scenario {
        boards: vec![BoardSpec {
            card_counter: 12,
            sprint_counters: vec![("kan", 7)],
            ..board(B1, "Alpha", Some("KAN"), Some("KAN"))
        }],
        sprints: vec![],
    };
    assert_backends_agree(&scenario, "a board with non-zero card and sprint counters");

    // Agreement alone would be satisfied by both backends dropping the
    // counter to zero, so pin the values themselves.
    let rt = make_rt();
    let rows = rt.block_on(sqlite_rows(&scenario)).unwrap();
    assert_eq!(
        rows,
        vec![("kan".to_string(), 12, 7)],
        "the migrated row must carry BOTH counters forward, not just match the other backend"
    );
}

#[test]
fn test_both_backfills_agree_on_a_default_prefix_collision() {
    assert_backends_agree(
        &Scenario {
            boards: vec![
                board(B1, "Gamma", None, None),
                board(B2, "Delta", None, None),
            ],
            sprints: vec![],
        },
        "two boards both falling back to the default prefix",
    );
}

/// A board explicitly named `task2` alongside two boards that fall back to
/// `task`. Under a renaming scheme this is where a generated name collides
/// with an explicitly held one; under a shared namespace no name is ever
/// generated, so `task2` belongs to exactly the board that asked for it.
#[test]
fn test_both_backfills_agree_when_a_board_is_explicitly_named_like_a_suffix() {
    assert_backends_agree(
        &Scenario {
            boards: vec![
                board(B1, "Gamma", None, None),
                board(B2, "Explicit", Some("task2"), Some("task2")),
                board(B3, "Delta", None, None),
            ],
            sprints: vec![],
        },
        "a board explicitly named task2 beside two defaulting boards",
    );
}

/// Two boards each explicitly configured with the SAME prefix. They are
/// asking for one namespace, and get one shared row -- neither an error nor
/// a rename of a name the user deliberately chose.
#[test]
fn test_both_backfills_agree_on_an_explicit_prefix_collision() {
    assert_backends_agree(
        &Scenario {
            boards: vec![
                board(B1, "One", Some("alpha"), Some("alpha")),
                board(B2, "Two", Some("alpha"), Some("alpha")),
            ],
            sprints: vec![],
        },
        "two boards explicitly configured with the same prefix",
    );
}

/// With one shared row there is no winner to pick, so storage order cannot
/// matter. The JSON envelope deliberately stores these boards in the reverse
/// of their id order: a backfill that still resolved per-owner would betray
/// itself here.
#[test]
fn test_both_backfills_agree_regardless_of_board_storage_order() {
    assert_backends_agree(
        &Scenario {
            boards: vec![
                board(B4, "Later id", None, None),
                board(B3, "Earlier id", None, None),
            ],
            sprints: vec![],
        },
        "colliding boards stored in the reverse of their id order",
    );
}
