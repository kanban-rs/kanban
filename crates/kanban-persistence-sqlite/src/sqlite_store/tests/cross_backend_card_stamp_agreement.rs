//! Executes both the SQLite `stamp_empty_card_prefixes` sweep and the JSON
//! V15 -> V16 card-prefix transform, live and in one process, over a single
//! shared scenario of cards that carry no prefix, and asserts the two
//! backends stamp the same value for the same card.
//!
//! Deliberately excludes a board configured with an empty `card_prefix`: this
//! sweep and the JSON transform diverge there on purpose (see KAN-1275).

use sqlx::{Pool, Sqlite};
use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;

struct BoardSpec {
    id: &'static str,
    card_prefix: Option<&'static str>,
}

struct SprintSpec {
    id: &'static str,
    board_id: &'static str,
    card_prefix: Option<&'static str>,
}

struct ColumnSpec {
    id: &'static str,
    board_id: &'static str,
}

struct CardSpec {
    id: &'static str,
    column_id: &'static str,
    board_id: &'static str,
    sprint_id: Option<&'static str>,
    number: i64,
}

struct Scenario {
    boards: Vec<BoardSpec>,
    sprints: Vec<SprintSpec>,
    columns: Vec<ColumnSpec>,
    cards: Vec<CardSpec>,
}

const TS: &str = "2024-01-01T00:00:00Z";

async fn sqlite_prefixes(scenario: &Scenario) -> Vec<(String, String)> {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let pool: Pool<Sqlite> = {
        let store = SqliteStore::open(&path).await.unwrap();
        store.pool().clone()
    };

    for b in &scenario.boards {
        sqlx::query("INSERT INTO boards (id, name, card_prefix, created_at, updated_at) VALUES (?, 'Board', ?, ?, ?)")
            .bind(b.id)
            .bind(b.card_prefix)
            .bind(TS)
            .bind(TS)
            .execute(&pool)
            .await
            .unwrap();
    }
    for c in &scenario.columns {
        sqlx::query("INSERT INTO columns (id, board_id, name, position, created_at, updated_at) VALUES (?, ?, 'Todo', 0, ?, ?)")
            .bind(c.id)
            .bind(c.board_id)
            .bind(TS)
            .bind(TS)
            .execute(&pool)
            .await
            .unwrap();
    }
    for s in &scenario.sprints {
        sqlx::query("INSERT INTO sprints (id, board_id, sprint_number, status, card_prefix, created_at, updated_at) VALUES (?, ?, 1, 'Planning', ?, ?, ?)")
            .bind(s.id)
            .bind(s.board_id)
            .bind(s.card_prefix)
            .bind(TS)
            .bind(TS)
            .execute(&pool)
            .await
            .unwrap();
    }
    for c in &scenario.cards {
        sqlx::query(
            "INSERT INTO cards (id, column_id, board_id, title, position, priority, status, \
             card_number, prefix, sprint_id, created_at, updated_at) \
             VALUES (?, ?, ?, 'Card', 0, 'medium', 'todo', ?, '', ?, ?, ?)",
        )
        .bind(c.id)
        .bind(c.column_id)
        .bind(c.board_id)
        .bind(c.number)
        .bind(c.sprint_id)
        .bind(TS)
        .bind(TS)
        .execute(&pool)
        .await
        .unwrap();
    }
    pool.close().await;

    let store = SqliteStore::open(&path).await.unwrap();
    let pool = store.pool();
    let mut rows: Vec<(String, String)> = Vec::new();
    for c in &scenario.cards {
        let prefix: String = sqlx::query_scalar("SELECT prefix FROM cards WHERE id = ?")
            .bind(c.id)
            .fetch_one(pool)
            .await
            .unwrap();
        rows.push((c.id.to_string(), prefix));
    }
    rows.sort();
    rows
}

fn json_prefixes(scenario: &Scenario) -> Vec<(String, String)> {
    let boards: Vec<serde_json::Value> = scenario
        .boards
        .iter()
        .map(|b| serde_json::json!({ "id": b.id, "card_prefix": b.card_prefix }))
        .collect();
    let columns: Vec<serde_json::Value> = scenario
        .columns
        .iter()
        .map(|c| serde_json::json!({ "id": c.id, "board_id": c.board_id }))
        .collect();
    let sprints: Vec<serde_json::Value> = scenario
        .sprints
        .iter()
        .map(|s| serde_json::json!({ "id": s.id, "board_id": s.board_id, "card_prefix": s.card_prefix }))
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
                "card_number": c.number,
            })
        })
        .collect();

    let mut env = serde_json::json!({
        "version": 15,
        "metadata": {
            "instance_id": "00000000-0000-0000-0000-000000000001",
            "saved_at": TS
        },
        "data": {
            "boards": boards, "columns": columns, "cards": cards,
            "archived_cards": [], "sprints": sprints,
            "graph": {
                "spawns": { "edges": [] },
                "blocks": { "edges": [] },
                "relates": { "edges": [] }
            }
        }
    });

    kanban_persistence_json::migration_test_support::try_transform_v15_to_v16(&mut env).unwrap();

    let mut rows: Vec<(String, String)> = env["data"]["cards"]
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

fn assert_backends_agree(scenario: &Scenario, what: &str) {
    let rt = make_rt();
    let sqlite = rt.block_on(sqlite_prefixes(scenario));
    let json = json_prefixes(scenario);
    assert_eq!(
        sqlite, json,
        "{what}: sqlite and json stamped different prefixes for the same content.\n\
         sqlite: {sqlite:#?}\njson:   {json:#?}"
    );
}

const B1: &str = "11111111-1111-1111-1111-111111111111";
const B2: &str = "22222222-2222-2222-2222-222222222222";
const C1: &str = "aaaaaaaa-1111-1111-1111-111111111111";
const CD: &str = "aaaaaaaa-2222-2222-2222-222222222222";
const S1: &str = "bbbbbbbb-1111-1111-1111-111111111111";
const CARD1: &str = "cccccccc-1111-1111-1111-111111111111";

#[test]
fn test_both_stamp_the_same_value_for_a_plain_board_prefix() {
    assert_backends_agree(
        &Scenario {
            boards: vec![BoardSpec {
                id: B1,
                card_prefix: Some("KAN"),
            }],
            sprints: vec![],
            columns: vec![ColumnSpec {
                id: C1,
                board_id: B1,
            }],
            cards: vec![CardSpec {
                id: CARD1,
                column_id: C1,
                board_id: B1,
                sprint_id: None,
                number: 5,
            }],
        },
        "a plain board prefix",
    );
}

#[test]
fn test_both_stamp_the_same_value_when_a_sprint_overrides_the_board() {
    assert_backends_agree(
        &Scenario {
            boards: vec![BoardSpec {
                id: B1,
                card_prefix: Some("KAN"),
            }],
            sprints: vec![SprintSpec {
                id: S1,
                board_id: B1,
                card_prefix: Some("AUTH"),
            }],
            columns: vec![ColumnSpec {
                id: C1,
                board_id: B1,
            }],
            cards: vec![CardSpec {
                id: CARD1,
                column_id: C1,
                board_id: B1,
                sprint_id: Some(S1),
                number: 5,
            }],
        },
        "a sprint override beating the board",
    );
}

#[test]
fn test_both_stamp_the_builtin_when_the_column_names_no_board() {
    assert_backends_agree(
        &Scenario {
            boards: vec![BoardSpec {
                id: B1,
                card_prefix: Some("KAN"),
            }],
            sprints: vec![],
            columns: vec![],
            cards: vec![CardSpec {
                id: CARD1,
                column_id: CD,
                board_id: B1,
                sprint_id: None,
                number: 5,
            }],
        },
        "a column naming no board",
    );
}

#[test]
fn test_both_follow_the_column_not_the_cards_board_id() {
    assert_backends_agree(
        &Scenario {
            boards: vec![
                BoardSpec {
                    id: B1,
                    card_prefix: Some("COL"),
                },
                BoardSpec {
                    id: B2,
                    card_prefix: Some("FIELD"),
                },
            ],
            sprints: vec![],
            columns: vec![ColumnSpec {
                id: C1,
                board_id: B1,
            }],
            cards: vec![CardSpec {
                id: CARD1,
                column_id: C1,
                board_id: B2,
                sprint_id: None,
                number: 1,
            }],
        },
        "card.board_id disagreeing with the column's board",
    );
}
