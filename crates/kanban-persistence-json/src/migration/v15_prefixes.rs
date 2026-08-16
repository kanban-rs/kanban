//! V15 backfills a `prefixes` array onto the envelope, one row per
//! currently-effective prefix (mirroring `kanban_domain::prefix`'s pure
//! resolution: every board's `card_prefix` or the `"task"` fallback, plus
//! every sprint override), seeding each row's `card_counter` /
//! `sprint_counter` from the owner's existing counter and resolving a
//! collision on the SAME normalised name by appending an incrementing
//! numeric suffix to every collider after the first (`task`, `task2`,
//! `task3`, ...).
//!
//! `boards.card_counter` and the board-level sprint-counter equivalent stay
//! untouched in the envelope; this migration is additive only.

use std::collections::HashMap;
use std::path::Path;

use kanban_persistence::{PersistenceError, PersistenceResult};
use serde_json::{Map, Value};

pub(crate) async fn migrate_v14_to_v15(path: &Path) -> PersistenceResult<()> {
    let content = tokio::fs::read_to_string(path).await?;
    let mut envelope: Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v14_to_v15_value(&mut envelope)? {
        return Ok(());
    }
    let out = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    crate::atomic_writer::AtomicWriter::write_atomic(path, out.as_bytes()).await?;
    tracing::info!(
        "Migrated {} from V14 to V15 (prefixes backfill)",
        path.display()
    );
    Ok(())
}

pub(crate) fn transform_v14_to_v15_value(envelope: &mut Value) -> PersistenceResult<bool> {
    let version = envelope.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version >= 15 {
        return Ok(false);
    }

    let boards: Vec<Value> = envelope
        .get("data")
        .and_then(|d| d.get("boards"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let sprints: Vec<Value> = envelope
        .get("data")
        .and_then(|d| d.get("sprints"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let rows = build_prefix_rows(&boards, &sprints);

    if let Some(data) = envelope.get_mut("data").and_then(Value::as_object_mut) {
        data.insert("prefixes".to_string(), Value::Array(rows));
    }

    envelope["version"] = Value::Number(15.into());
    Ok(true)
}

enum Owner {
    Board(String),
    Sprint(String),
}

fn build_prefix_rows(boards: &[Value], sprints: &[Value]) -> Vec<Value> {
    struct Entry {
        name: String,
        owner: Owner,
        card_counter: u64,
        sprint_counter: u64,
    }

    let mut entries: Vec<Entry> = Vec::new();

    for board in boards {
        let id = board.get("id").and_then(Value::as_str).unwrap_or("");
        let prefix = board
            .get("card_prefix")
            .and_then(Value::as_str)
            .unwrap_or("task");
        let card_counter = board
            .get("card_counter")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        // A board's sprint-naming prefix is a SEPARATE namespace from its
        // card prefix. They coincide on most boards, in which case one row
        // carries both counters. When they differ the board owns TWO rows,
        // each carrying only the counter that belongs to it. Missing this
        // meant a board with `card_prefix=DEV, sprint_prefix=REL` produced no
        // `rel` row at all, so the JSON backfill silently diverged from the
        // SQLite one, which has always emitted both.
        let sprint_prefix = board.get("sprint_prefix").and_then(Value::as_str);
        let sprint_counter = board
            .get("next_sprint_number")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let merges = sprint_prefix
            .map(|sp| normalize(sp) == normalize(prefix))
            .unwrap_or(true);

        entries.push(Entry {
            name: normalize(prefix),
            owner: Owner::Board(id.to_string()),
            card_counter,
            sprint_counter: if merges { sprint_counter } else { 0 },
        });

        if !merges {
            if let Some(sp) = sprint_prefix {
                entries.push(Entry {
                    name: normalize(sp),
                    owner: Owner::Board(id.to_string()),
                    card_counter: 0,
                    sprint_counter,
                });
            }
        }
    }

    for sprint in sprints {
        let Some(prefix) = sprint.get("card_prefix").and_then(Value::as_str) else {
            continue;
        };
        let id = sprint.get("id").and_then(Value::as_str).unwrap_or("");
        entries.push(Entry {
            name: normalize(prefix),
            owner: Owner::Sprint(id.to_string()),
            card_counter: 0,
            sprint_counter: 0,
        });
    }

    for board in boards {
        let sprint_counter = board
            .get("sprint_counter")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if sprint_counter == 0 {
            continue;
        }
        let id = board.get("id").and_then(Value::as_str).unwrap_or("");
        if let Some(entry) = entries.iter_mut().find(|e| match &e.owner {
            Owner::Board(bid) => bid == id,
            Owner::Sprint(_) => false,
        }) {
            entry.sprint_counter = sprint_counter;
        }
    }

    let mut seen: HashMap<String, u32> = HashMap::new();
    let mut rows = Vec::with_capacity(entries.len());
    for entry in entries {
        let count = seen.entry(entry.name.clone()).or_insert(0);
        *count += 1;
        let resolved_name = if *count == 1 {
            entry.name.clone()
        } else {
            format!("{}{}", entry.name, *count)
        };

        let mut obj = Map::new();
        obj.insert("name".to_string(), Value::String(resolved_name));
        match &entry.owner {
            Owner::Board(id) => {
                obj.insert("owner_type".to_string(), Value::String("board".to_string()));
                obj.insert("owner_id".to_string(), Value::String(id.clone()));
            }
            Owner::Sprint(id) => {
                obj.insert(
                    "owner_type".to_string(),
                    Value::String("sprint".to_string()),
                );
                obj.insert("owner_id".to_string(), Value::String(id.clone()));
            }
        }
        obj.insert(
            "card_counter".to_string(),
            Value::Number(entry.card_counter.into()),
        );
        obj.insert(
            "sprint_counter".to_string(),
            Value::Number(entry.sprint_counter.into()),
        );
        rows.push(Value::Object(obj));
    }

    rows
}

fn normalize(raw: &str) -> String {
    kanban_domain::prefix::Prefix::normalize(raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    const BOARD_A: &str = "11111111-1111-1111-1111-111111111111";
    const BOARD_B: &str = "22222222-2222-2222-2222-222222222222";
    const SPRINT_A: &str = "33333333-3333-3333-3333-333333333333";

    fn make_v14_envelope(boards: Value, sprints: Value) -> Value {
        json!({
            "version": 14,
            "metadata": {
                "instance_id": "00000000-0000-0000-0000-000000000001",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": boards,
                "columns": [],
                "cards": [],
                "archived_cards": [],
                "sprints": sprints,
                "graph": {
                    "spawns": { "edges": [] },
                    "blocks": { "edges": [] },
                    "relates": { "edges": [] }
                }
            }
        })
    }

    fn board(id: &str, prefix: Value, card_counter: u64) -> Value {
        json!({ "id": id, "name": "B", "card_prefix": prefix, "card_counter": card_counter })
    }

    fn sprint(id: &str, board_id: &str, prefix: Value) -> Value {
        json!({ "id": id, "board_id": board_id, "card_prefix": prefix })
    }

    #[test]
    fn test_migrate_v14_to_v15_creates_one_prefix_per_distinct_effective_prefix() {
        let mut env = make_v14_envelope(
            json!([
                board(BOARD_A, json!("kan"), 0),
                board(BOARD_B, json!("dev"), 0),
            ]),
            json!([]),
        );

        let changed = transform_v14_to_v15_value(&mut env).unwrap();

        assert!(changed);
        assert_eq!(env["version"], 15);
        let prefixes = env["data"]["prefixes"].as_array().unwrap();
        assert_eq!(prefixes.len(), 2);
        let names: Vec<&str> = prefixes
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"kan"));
        assert!(names.contains(&"dev"));
    }

    #[test]
    fn test_migrate_v14_to_v15_defaults_null_card_prefix_to_task() {
        let mut env = make_v14_envelope(json!([board(BOARD_A, Value::Null, 0)]), json!([]));

        transform_v14_to_v15_value(&mut env).unwrap();

        let prefixes = env["data"]["prefixes"].as_array().unwrap();
        assert_eq!(prefixes.len(), 1);
        assert_eq!(prefixes[0]["name"], "task");
    }

    #[test]
    fn test_migrate_v14_to_v15_creates_two_rows_when_prefixes_differ() {
        let mut env = make_v14_envelope(
            json!([board(BOARD_A, json!("kan"), 0)]),
            json!([sprint(SPRINT_A, BOARD_A, json!("auth"))]),
        );

        transform_v14_to_v15_value(&mut env).unwrap();

        let prefixes = env["data"]["prefixes"].as_array().unwrap();
        assert_eq!(prefixes.len(), 2);
    }

    #[test]
    fn test_migrate_v14_to_v15_preserves_card_counter_value() {
        let mut env = make_v14_envelope(json!([board(BOARD_A, json!("kan"), 7)]), json!([]));

        transform_v14_to_v15_value(&mut env).unwrap();

        let prefixes = env["data"]["prefixes"].as_array().unwrap();
        assert_eq!(prefixes[0]["card_counter"], 7);
    }

    #[test]
    fn test_migrate_v14_to_v15_increments_on_default_prefix_collision() {
        let mut env = make_v14_envelope(
            json!([
                board(BOARD_A, Value::Null, 0),
                board(BOARD_B, Value::Null, 0),
            ]),
            json!([]),
        );

        transform_v14_to_v15_value(&mut env).unwrap();

        let prefixes = env["data"]["prefixes"].as_array().unwrap();
        let mut names: Vec<&str> = prefixes
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, vec!["task", "task2"]);
    }

    #[test]
    fn test_migrate_v14_to_v15_writes_v14_backup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v14.json");
        let env = make_v14_envelope(json!([board(BOARD_A, json!("kan"), 0)]), json!([]));
        std::fs::write(&path, serde_json::to_string_pretty(&env).unwrap()).unwrap();

        tokio_test_migrate(&path);

        assert!(
            !path.with_extension("v14.backup").exists(),
            ".v14.backup must be removed after a successful V14 -> V15 migration"
        );
    }

    fn tokio_test_migrate(path: &std::path::Path) {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            super::super::migrator::Migrator::migrate(
                kanban_persistence::FormatVersion::V14,
                kanban_persistence::FormatVersion::MAX,
                path,
            )
            .await
            .unwrap();
        });
    }

    #[test]
    fn test_migrate_v14_to_v15_is_idempotent_on_already_migrated_envelope() {
        let v15 = json!({
            "version": 15,
            "data": {
                "boards": [board(BOARD_A, json!("kan"), 0)],
                "sprints": [],
                "prefixes": [{
                    "name": "kan", "owner_type": "board", "owner_id": BOARD_A,
                    "card_counter": 0, "sprint_counter": 0
                }]
            }
        });
        let mut env = v15.clone();

        let changed = transform_v14_to_v15_value(&mut env).unwrap();

        assert!(!changed);
        assert_eq!(env, v15, "a version:15 envelope must be returned unchanged");
    }

    #[test]
    fn test_migrate_v14_to_v15_leaves_legacy_card_counter_untouched() {
        let mut env = make_v14_envelope(json!([board(BOARD_A, json!("kan"), 5)]), json!([]));

        transform_v14_to_v15_value(&mut env).unwrap();

        assert_eq!(
            env["data"]["boards"][0]["card_counter"], 5,
            "boards.card_counter is untouched by this additive-only migration"
        );
    }
}
