//! Whole-graph cross-backend agreement: one board, two columns, four cards
//! (two bound to a sprint) driven through BOTH the SQLite and the JSON
//! prefix-backfill and card-prefix-stamp transforms, asserting the two
//! backends agree on the resulting `prefixes` rows and per-card stamped
//! prefixes.
//!
//! `cross_backend_prefix_agreement.rs` and `cross_backend_card_stamp_agreement.rs`
//! already prove backend agreement, but only over boards-and-sprints-only or
//! single-column scenarios. This adds the missing multi-column,
//! sprint-bound-card shape to that same comparison, reusing the identical
//! transform entry points those files use.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;

const TS: &str = "2024-01-01T00:00:00Z";

const BOARD: &str = "11111111-1111-1111-1111-111111111111";
const COL_TODO: &str = "22222222-2222-2222-2222-222222222222";
const COL_DOING: &str = "33333333-3333-3333-3333-333333333333";
const SPRINT: &str = "44444444-4444-4444-4444-444444444444";
const CARD1: &str = "aaaaaaaa-1111-1111-1111-111111111111";
const CARD2: &str = "aaaaaaaa-2222-2222-2222-222222222222";
const CARD3: &str = "aaaaaaaa-3333-3333-3333-333333333333";
const CARD4: &str = "aaaaaaaa-4444-4444-4444-444444444444";

struct CardRow {
    id: &'static str,
    column_id: &'static str,
    sprint_id: Option<&'static str>,
    number: i64,
}

fn scenario_cards() -> Vec<CardRow> {
    vec![
        CardRow {
            id: CARD1,
            column_id: COL_TODO,
            sprint_id: None,
            number: 1,
        },
        CardRow {
            id: CARD2,
            column_id: COL_TODO,
            sprint_id: Some(SPRINT),
            number: 2,
        },
        CardRow {
            id: CARD3,
            column_id: COL_DOING,
            sprint_id: Some(SPRINT),
            number: 3,
        },
        CardRow {
            id: CARD4,
            column_id: COL_DOING,
            sprint_id: None,
            number: 4,
        },
    ]
}

type PrefixRow = (String, i64, i64);
type StampRow = (String, String);

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
    );";

async fn build_v9_db(path: &std::path::Path) -> Pool<Sqlite> {
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
    sqlx::raw_sql(&format!(
        "INSERT INTO metadata (id, instance_id, saved_at, schema_version)
             VALUES (1, '00000000-0000-0000-0000-000000000001', '{TS}', 9);
         INSERT INTO boards (id, name, card_prefix, created_at, updated_at)
             VALUES ('{BOARD}', 'Board', 'KAN', '{TS}', '{TS}');
         INSERT INTO columns (id, board_id, name, position, created_at, updated_at)
             VALUES ('{COL_TODO}', '{BOARD}', 'Todo', 0, '{TS}', '{TS}'),
                    ('{COL_DOING}', '{BOARD}', 'Doing', 1, '{TS}', '{TS}');
         INSERT INTO sprints (id, board_id, name, number, created_at, updated_at)
             VALUES ('{SPRINT}', '{BOARD}', 'Sprint One', 1, '{TS}', '{TS}');"
    ))
    .execute(&pool)
    .await
    .unwrap();
    for c in scenario_cards() {
        sqlx::query(
            "INSERT INTO cards (id, column_id, board_id, title, position, card_number, sprint_id,
                                created_at, updated_at)
             VALUES (?, ?, ?, 'Card', 0, ?, ?, ?, ?)",
        )
        .bind(c.id)
        .bind(c.column_id)
        .bind(BOARD)
        .bind(c.number)
        .bind(c.sprint_id)
        .bind(TS)
        .bind(TS)
        .execute(&pool)
        .await
        .unwrap();
    }
    pool
}

async fn sqlite_prefix_rows() -> Vec<PrefixRow> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v9.db");
    build_v9_db(&path).await.close().await;
    let store = SqliteStore::open(&path).await.unwrap();
    let mut rows: Vec<PrefixRow> =
        sqlx::query_as("SELECT name, card_counter, sprint_counter FROM prefixes")
            .fetch_all(store.pool())
            .await
            .unwrap();
    rows.sort();
    rows
}

async fn sqlite_card_stamps() -> Vec<StampRow> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v9.db");
    build_v9_db(&path).await.close().await;
    let store = SqliteStore::open(&path).await.unwrap();
    let mut rows: Vec<StampRow> = sqlx::query_as("SELECT id, prefix FROM cards")
        .fetch_all(store.pool())
        .await
        .unwrap();
    rows.sort();
    rows
}

fn json_envelope_v14() -> serde_json::Value {
    let columns = serde_json::json!([
        { "id": COL_TODO, "board_id": BOARD },
        { "id": COL_DOING, "board_id": BOARD }
    ]);
    let cards: Vec<serde_json::Value> = scenario_cards()
        .into_iter()
        .map(|c| {
            serde_json::json!({
                "id": c.id, "column_id": c.column_id, "board_id": BOARD,
                "sprint_id": c.sprint_id, "card_number": c.number,
            })
        })
        .collect();
    serde_json::json!({
        "version": 14,
        "metadata": { "instance_id": "00000000-0000-0000-0000-000000000001", "saved_at": TS },
        "data": {
            "boards": [{ "id": BOARD, "name": "Board", "card_prefix": "KAN" }],
            "columns": columns,
            "cards": cards,
            "archived_cards": [],
            "sprints": [{ "id": SPRINT, "board_id": BOARD, "card_prefix": null }],
            "graph": { "spawns": { "edges": [] }, "blocks": { "edges": [] }, "relates": { "edges": [] } }
        }
    })
}

async fn json_prefix_rows() -> Vec<PrefixRow> {
    use kanban_persistence::FormatVersion;
    use kanban_persistence_json::migration::Migrator;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("board.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&json_envelope_v14()).unwrap(),
    )
    .unwrap();
    Migrator::migrate(FormatVersion::V14, FormatVersion::MAX, &path)
        .await
        .unwrap();
    let env: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let mut rows: Vec<PrefixRow> = env["data"]["prefixes"]
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
    rows
}

fn json_card_stamps() -> Vec<StampRow> {
    let mut env = json_envelope_v14();
    env["version"] = serde_json::json!(15);
    kanban_persistence_json::migration_test_support::try_transform_v15_to_v16(&mut env).unwrap();
    let mut rows: Vec<StampRow> = env["data"]["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["id"].as_str().unwrap().to_string(),
                c["prefix"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    rows.sort();
    rows
}

#[test]
fn test_both_backends_agree_on_prefix_rows_for_the_whole_graph() {
    let rt = make_rt();
    let sqlite = rt.block_on(sqlite_prefix_rows());
    let json = rt.block_on(json_prefix_rows());
    assert_eq!(
        sqlite, json,
        "prefix rows diverged over the whole-graph scenario.\nsqlite: {sqlite:#?}\njson: {json:#?}"
    );
}

#[test]
fn test_both_backends_agree_on_card_stamps_for_the_whole_graph() {
    let rt = make_rt();
    let sqlite = rt.block_on(sqlite_card_stamps());
    let json = json_card_stamps();
    assert_eq!(
        sqlite, json,
        "card stamps diverged over the whole-graph scenario.\nsqlite: {sqlite:#?}\njson: {json:#?}"
    );
    assert_eq!(
        json,
        vec![
            (CARD1.to_string(), "KAN".to_string()),
            (CARD2.to_string(), "KAN".to_string()),
            (CARD3.to_string(), "KAN".to_string()),
            (CARD4.to_string(), "KAN".to_string()),
        ],
        "agreement alone would pass if both backends stamped the wrong value; \
         pin the actual expected prefix too"
    );
}
