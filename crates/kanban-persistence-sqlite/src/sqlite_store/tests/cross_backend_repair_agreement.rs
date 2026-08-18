//! Executes BOTH the SQLite v12 -> v13 stamp/repair chain and the JSON
//! V17 -> V18 prefix-row repair, live and in one process, over a single
//! shared scenario, and asserts the two produce byte-identical results:
//! the same prefix rows (name, `card_counter`, `sprint_counter`), IN THE
//! SAME ORDER, and the same stamped prefix on every card.
//!
//! SQLite is read canonically with `ORDER BY name ASC`, exactly what
//! `list_prefixes` reads. The JSON side is read from `data.prefixes` in
//! array order, unsorted -- so this module pins the migration-boundary
//! ordering contract rather than the general (and backend-divergent)
//! runtime `list_prefixes()` ordering.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::path::Path;
use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;

struct BoardSpec {
    id: &'static str,
    card_prefix: Option<&'static str>,
}

struct ColumnSpec {
    id: &'static str,
    board_id: &'static str,
}

struct SprintSpec {
    id: &'static str,
    board_id: &'static str,
    card_prefix: Option<&'static str>,
    prefix: Option<&'static str>,
    sprint_number: i64,
}

struct CardSpec {
    id: &'static str,
    column_id: &'static str,
    board_id: &'static str,
    sprint_id: Option<&'static str>,
    prefix: &'static str,
    number: i64,
}

struct PrefixSpec {
    name: &'static str,
    card_counter: i64,
    sprint_counter: i64,
}

struct Scenario {
    boards: Vec<BoardSpec>,
    columns: Vec<ColumnSpec>,
    sprints: Vec<SprintSpec>,
    cards: Vec<CardSpec>,
    /// Declared in the order the fixture must store the pre-existing rows.
    prefixes: Vec<PrefixSpec>,
}

/// A repaired prefix row: name and its counters, in read order.
type Row = (String, i64, i64);
/// A card's id and its stored prefix, always sorted by id before comparing.
type Stamp = (String, String);

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

async fn seed_v12(scenario: &Scenario, path: &Path) {
    {
        let store = SqliteStore::open(path).await.unwrap();
        store.pool().close().await;
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

    for b in &scenario.boards {
        sqlx::query(
            "INSERT INTO boards (id, name, card_prefix, created_at, updated_at)
             VALUES (?, 'Board', ?, ?, ?)",
        )
        .bind(b.id)
        .bind(b.card_prefix)
        .bind(TS)
        .bind(TS)
        .execute(&pool)
        .await
        .unwrap();
    }
    for c in &scenario.columns {
        sqlx::query(
            "INSERT INTO columns (id, board_id, name, position, created_at, updated_at)
             VALUES (?, ?, 'Todo', 0, ?, ?)",
        )
        .bind(c.id)
        .bind(c.board_id)
        .bind(TS)
        .bind(TS)
        .execute(&pool)
        .await
        .unwrap();
    }
    for s in &scenario.sprints {
        sqlx::query(
            "INSERT INTO sprints (id, board_id, sprint_number, status, prefix, card_prefix, created_at, updated_at)
             VALUES (?, ?, ?, 'Planning', ?, ?, ?, ?)",
        )
        .bind(s.id)
        .bind(s.board_id)
        .bind(s.sprint_number)
        .bind(s.prefix)
        .bind(s.card_prefix)
        .bind(TS)
        .bind(TS)
        .execute(&pool)
        .await
        .unwrap();
    }
    for p in &scenario.prefixes {
        sqlx::query("INSERT INTO prefixes (name, card_counter, sprint_counter) VALUES (?, ?, ?)")
            .bind(p.name)
            .bind(p.card_counter)
            .bind(p.sprint_counter)
            .execute(&pool)
            .await
            .unwrap();
    }
    for c in &scenario.cards {
        sqlx::query(
            "INSERT INTO cards (id, column_id, board_id, title, position, priority, status,
                                card_number, prefix, sprint_id, created_at, updated_at)
             VALUES (?, ?, ?, 'Card', 0, 'medium', 'todo', ?, ?, ?, ?, ?)",
        )
        .bind(c.id)
        .bind(c.column_id)
        .bind(c.board_id)
        .bind(c.number)
        .bind(c.prefix)
        .bind(c.sprint_id)
        .bind(TS)
        .bind(TS)
        .execute(&pool)
        .await
        .unwrap();
    }

    pool.close().await;
}

async fn sqlite_repair(scenario: &Scenario) -> Result<(Vec<Row>, Vec<Stamp>), String> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v12.db");
    seed_v12(scenario, &path).await;

    let store = SqliteStore::open(&path).await.map_err(|e| e.to_string())?;
    let pool = store.pool();

    let rows: Vec<Row> =
        sqlx::query_as("SELECT name, card_counter, sprint_counter FROM prefixes ORDER BY name ASC")
            .fetch_all(pool)
            .await
            .unwrap();

    let mut stamps: Vec<Stamp> = sqlx::query_as("SELECT id, prefix FROM cards")
        .fetch_all(pool)
        .await
        .unwrap();
    stamps.sort();

    Ok((rows, stamps))
}

fn json_envelope(scenario: &Scenario) -> serde_json::Value {
    let boards: Vec<serde_json::Value> = scenario
        .boards
        .iter()
        .map(|b| {
            serde_json::json!({
                "id": b.id,
                "name": "Board",
                "card_prefix": b.card_prefix,
            })
        })
        .collect();
    let columns: Vec<serde_json::Value> = scenario
        .columns
        .iter()
        .map(|c| {
            serde_json::json!({
                "id": c.id,
                "board_id": c.board_id,
                "name": "Todo",
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
                "prefix": s.prefix,
                "sprint_number": s.sprint_number,
            })
        })
        .collect();
    let cards: Vec<serde_json::Value> = scenario
        .cards
        .iter()
        .map(|c| {
            serde_json::json!({
                "id": c.id,
                "column_id": c.column_id,
                "board_id": c.board_id,
                "sprint_id": c.sprint_id,
                "prefix": c.prefix,
                "card_number": c.number,
            })
        })
        .collect();
    let prefixes: Vec<serde_json::Value> = scenario
        .prefixes
        .iter()
        .map(|p| {
            serde_json::json!({
                "name": p.name,
                "card_counter": p.card_counter,
                "sprint_counter": p.sprint_counter,
            })
        })
        .collect();

    serde_json::json!({
        "version": 17,
        "metadata": {
            "instance_id": "00000000-0000-0000-0000-000000000001",
            "saved_at": TS
        },
        "data": {
            "boards": boards,
            "columns": columns,
            "cards": cards,
            "sprints": sprints,
            "prefixes": prefixes,
            "archived_cards": [],
            "archived_boards": [],
            "graph": {
                "spawns": { "edges": [] },
                "blocks": { "edges": [] },
                "relates": { "edges": [] }
            }
        }
    })
}

fn json_repair(scenario: &Scenario) -> Result<(Vec<Row>, Vec<Stamp>), String> {
    let mut env = json_envelope(scenario);
    kanban_persistence_json::migration_test_support::try_transform_v17_to_v18(&mut env)
        .map_err(|e| e.to_string())?;

    let rows: Vec<Row> = env["data"]["prefixes"]
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

    let mut stamps: Vec<Stamp> = env["data"]["cards"]
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
    stamps.sort();

    Ok((rows, stamps))
}

fn assert_backends_agree(scenario: &Scenario, what: &str) {
    let rt = make_rt();
    let sqlite = rt.block_on(sqlite_repair(scenario));
    let json = json_repair(scenario);

    match (&sqlite, &json) {
        (Ok((s_rows, s_stamps)), Ok((j_rows, j_stamps))) => {
            assert_eq!(
                s_rows, j_rows,
                "{what}: prefix rows disagree (name/counter/order).\nsqlite: {s_rows:#?}\njson:   {j_rows:#?}"
            );
            assert_eq!(
                s_stamps, j_stamps,
                "{what}: stamped card prefixes disagree.\nsqlite: {s_stamps:#?}\njson:   {j_stamps:#?}"
            );
        }
        (Err(se), Err(je)) => {
            let _ = (se, je);
        }
        (Ok((s_rows, _)), Err(je)) => panic!(
            "{what}: the SQLite repair succeeded but the JSON one rejected the same input.\n\
             sqlite: {s_rows:#?}\njson error: {je}"
        ),
        (Err(se), Ok((j_rows, _))) => panic!(
            "{what}: the JSON repair succeeded but the SQLite one rejected the same input.\n\
             json: {j_rows:#?}\nsqlite error: {se}"
        ),
    }
}

const B1: &str = "11111111-1111-1111-1111-111111111111";
const B2: &str = "22222222-2222-2222-2222-222222222222";
const B3: &str = "33333333-3333-3333-3333-333333333333";
const C1: &str = "00000000-0000-0000-0000-0000000000c1";
const C2: &str = "00000000-0000-0000-0000-0000000000c2";
const C3: &str = "00000000-0000-0000-0000-0000000000c3";
const S1: &str = "aaaaaaaa-1111-1111-1111-111111111111";
const A1: &str = "00000000-0000-0000-0000-0000000000a1";
const A2: &str = "00000000-0000-0000-0000-0000000000a2";
const A3: &str = "00000000-0000-0000-0000-0000000000a3";
const A4: &str = "00000000-0000-0000-0000-0000000000a4";
const A5: &str = "00000000-0000-0000-0000-0000000000a5";
const A6: &str = "00000000-0000-0000-0000-0000000000a6";

#[test]
fn test_both_backends_repair_an_orphaned_namespace_identically() {
    assert_backends_agree(
        &Scenario {
            boards: vec![BoardSpec {
                id: B1,
                card_prefix: Some("KAN"),
            }],
            columns: vec![ColumnSpec {
                id: C1,
                board_id: B1,
            }],
            sprints: vec![],
            prefixes: vec![],
            cards: vec![
                CardSpec {
                    id: A1,
                    column_id: C1,
                    board_id: B1,
                    sprint_id: None,
                    prefix: "KAN",
                    number: 7,
                },
                CardSpec {
                    id: A2,
                    column_id: C1,
                    board_id: B1,
                    sprint_id: None,
                    prefix: "kan",
                    number: 3,
                },
                CardSpec {
                    id: A3,
                    column_id: C1,
                    board_id: B1,
                    sprint_id: None,
                    prefix: "ops",
                    number: 2,
                },
            ],
        },
        "unbacked namespaces named by several cards with mixed casing",
    );
}

#[test]
fn test_both_backends_stamp_empty_prefix_cards_identically() {
    assert_backends_agree(
        &Scenario {
            boards: vec![
                BoardSpec {
                    id: B1,
                    card_prefix: Some("KAN"),
                },
                BoardSpec {
                    id: B2,
                    card_prefix: None,
                },
            ],
            columns: vec![
                ColumnSpec {
                    id: C1,
                    board_id: B1,
                },
                ColumnSpec {
                    id: C2,
                    board_id: B2,
                },
            ],
            sprints: vec![SprintSpec {
                id: S1,
                board_id: B1,
                card_prefix: Some("AUTH"),
                prefix: None,
                sprint_number: 1,
            }],
            prefixes: vec![],
            cards: vec![
                CardSpec {
                    id: A1,
                    column_id: C1,
                    board_id: B1,
                    sprint_id: None,
                    prefix: "",
                    number: 5,
                },
                CardSpec {
                    id: A2,
                    column_id: C2,
                    board_id: B2,
                    sprint_id: None,
                    prefix: "",
                    number: 2,
                },
                CardSpec {
                    id: A3,
                    column_id: C1,
                    board_id: B1,
                    sprint_id: Some(S1),
                    prefix: "",
                    number: 4,
                },
            ],
        },
        "empty-prefix cards under a board default, a missing default, and a sprint override",
    );
}

#[test]
fn test_both_backends_raise_a_lagging_namespace_identically() {
    assert_backends_agree(
        &Scenario {
            boards: vec![BoardSpec {
                id: B1,
                card_prefix: Some("KAN"),
            }],
            columns: vec![ColumnSpec {
                id: C1,
                board_id: B1,
            }],
            sprints: vec![],
            prefixes: vec![PrefixSpec {
                name: "kan",
                card_counter: 3,
                sprint_counter: 5,
            }],
            cards: vec![CardSpec {
                id: A1,
                column_id: C1,
                board_id: B1,
                sprint_id: None,
                prefix: "KAN",
                number: 9,
            }],
        },
        "a backed namespace whose row lags the card that names it",
    );
}

#[test]
fn test_both_backends_agree_on_the_full_repair_including_row_order() {
    assert_backends_agree(
        &Scenario {
            boards: vec![
                BoardSpec {
                    id: B1,
                    card_prefix: Some("KAN"),
                },
                BoardSpec {
                    id: B2,
                    card_prefix: Some("ZETA"),
                },
                BoardSpec {
                    id: B3,
                    card_prefix: None,
                },
            ],
            columns: vec![
                ColumnSpec {
                    id: C1,
                    board_id: B1,
                },
                ColumnSpec {
                    id: C2,
                    board_id: B2,
                },
                ColumnSpec {
                    id: C3,
                    board_id: B3,
                },
            ],
            sprints: vec![SprintSpec {
                id: S1,
                board_id: B1,
                card_prefix: Some("AUTH"),
                prefix: None,
                sprint_number: 1,
            }],
            prefixes: vec![
                PrefixSpec {
                    name: "zeta",
                    card_counter: 4,
                    sprint_counter: 0,
                },
                PrefixSpec {
                    name: "kan",
                    card_counter: 3,
                    sprint_counter: 1,
                },
            ],
            cards: vec![
                CardSpec {
                    id: A1,
                    column_id: C1,
                    board_id: B1,
                    sprint_id: None,
                    prefix: "KAN",
                    number: 9,
                },
                CardSpec {
                    id: A2,
                    column_id: C1,
                    board_id: B1,
                    sprint_id: None,
                    prefix: "kan",
                    number: 4,
                },
                CardSpec {
                    id: A3,
                    column_id: C2,
                    board_id: B2,
                    sprint_id: None,
                    prefix: "ZETA",
                    number: 2,
                },
                CardSpec {
                    id: A4,
                    column_id: C2,
                    board_id: B2,
                    sprint_id: None,
                    prefix: "ops",
                    number: 6,
                },
                CardSpec {
                    id: A5,
                    column_id: C3,
                    board_id: B3,
                    sprint_id: None,
                    prefix: "",
                    number: 3,
                },
                CardSpec {
                    id: A6,
                    column_id: C1,
                    board_id: B1,
                    sprint_id: Some(S1),
                    prefix: "",
                    number: 8,
                },
            ],
        },
        "a full scenario with pre-existing rows stored out of name order",
    );
}

const S2: &str = "aaaaaaaa-2222-2222-2222-222222222222";

#[test]
fn test_both_backends_preserve_sprint_counters_identically() {
    assert_backends_agree(
        &Scenario {
            boards: vec![BoardSpec {
                id: B1,
                card_prefix: Some("KAN"),
            }],
            columns: vec![ColumnSpec {
                id: C1,
                board_id: B1,
            }],
            sprints: vec![
                SprintSpec {
                    id: S1,
                    board_id: B1,
                    card_prefix: None,
                    prefix: Some("QTR"),
                    sprint_number: 2,
                },
                SprintSpec {
                    id: S2,
                    board_id: B1,
                    card_prefix: None,
                    prefix: Some("QTR"),
                    sprint_number: 1,
                },
            ],
            prefixes: vec![],
            cards: vec![CardSpec {
                id: A1,
                column_id: C1,
                board_id: B1,
                sprint_id: None,
                prefix: "KAN",
                number: 3,
            }],
        },
        "a sprint prefix a board no longer names",
    );

    let scenario = Scenario {
        boards: vec![BoardSpec {
            id: B1,
            card_prefix: Some("KAN"),
        }],
        columns: vec![ColumnSpec {
            id: C1,
            board_id: B1,
        }],
        sprints: vec![
            SprintSpec {
                id: S1,
                board_id: B1,
                card_prefix: None,
                prefix: Some("QTR"),
                sprint_number: 2,
            },
            SprintSpec {
                id: S2,
                board_id: B1,
                card_prefix: None,
                prefix: Some("QTR"),
                sprint_number: 1,
            },
        ],
        prefixes: vec![],
        cards: vec![CardSpec {
            id: A1,
            column_id: C1,
            board_id: B1,
            sprint_id: None,
            prefix: "KAN",
            number: 3,
        }],
    };
    let (rows, _) = json_repair(&scenario).unwrap();
    assert_eq!(
        rows,
        vec![("kan".to_string(), 3, 0), ("qtr".to_string(), 0, 2)],
        "both backends must agree AND preserve the QTR sprint counter"
    );
}
