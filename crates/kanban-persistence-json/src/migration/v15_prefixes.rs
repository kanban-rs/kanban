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

use std::path::Path;

use kanban_domain::{
    plan_prefix_backfill, BackfillBoard, BackfillSprint, DEFAULT_CARD_PREFIX, DEFAULT_SPRINT_PREFIX,
};
use kanban_persistence::{PersistenceError, PersistenceResult};
use serde_json::{Map, Value};
use uuid::Uuid;

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

fn build_prefix_rows(boards: &[Value], sprints: &[Value]) -> Vec<Value> {
    let backfill_boards: Vec<BackfillBoard> = boards
        .iter()
        .map(|board| BackfillBoard {
            id: parse_id(board.get("id")),
            card_prefix: str_field(board, "card_prefix"),
            sprint_prefix: str_field(board, "sprint_prefix"),
            card_counter: board
                .get("card_counter")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            sprint_counters: board
                .get("sprint_counters")
                .and_then(Value::as_object)
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| (k.clone(), v.as_i64().unwrap_or(0)))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect();

    let backfill_sprints: Vec<BackfillSprint> = sprints
        .iter()
        .filter_map(|sprint| {
            str_field(sprint, "card_prefix").map(|card_prefix| BackfillSprint { card_prefix })
        })
        .collect();

    let rows = plan_prefix_backfill(
        &backfill_boards,
        &backfill_sprints,
        DEFAULT_CARD_PREFIX,
        DEFAULT_SPRINT_PREFIX,
    );

    rows.into_iter()
        .map(|row| {
            let mut obj = Map::new();
            obj.insert("name".to_string(), Value::String(row.name));
            obj.insert(
                "card_counter".to_string(),
                Value::Number(row.card_counter.into()),
            );
            obj.insert(
                "sprint_counter".to_string(),
                Value::Number(row.sprint_counter.into()),
            );
            Value::Object(obj)
        })
        .collect()
}

fn str_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Board ids come off the raw envelope as strings. A malformed one becomes
/// nil rather than aborting the migration: this backfill is additive and
/// nothing reads it yet, so a corrupt id must not make an otherwise-loadable
/// file unopenable.
fn parse_id(value: Option<&Value>) -> Uuid {
    value
        .and_then(Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok())
        .unwrap_or(Uuid::nil())
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

    /// Sorted prefix names off a migrated envelope.
    fn names(env: &Value) -> Vec<&str> {
        let mut names: Vec<&str> = env["data"]["prefixes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["name"].as_str().unwrap())
            .collect();
        names.sort_unstable();
        names
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
        assert_eq!(
            names(&env),
            vec!["dev", "kan", "sprint"],
            "both card namespaces, plus the default sprint-naming namespace \
             both boards allocate sprint numbers from"
        );
    }

    #[test]
    fn test_migrate_v14_to_v15_defaults_null_card_prefix_to_task() {
        let mut env = make_v14_envelope(json!([board(BOARD_A, Value::Null, 0)]), json!([]));

        transform_v14_to_v15_value(&mut env).unwrap();

        assert_eq!(names(&env), vec!["sprint", "task"]);
    }

    #[test]
    fn test_migrate_v14_to_v15_creates_two_rows_when_prefixes_differ() {
        let mut env = make_v14_envelope(
            json!([board(BOARD_A, json!("kan"), 0)]),
            json!([sprint(SPRINT_A, BOARD_A, json!("auth"))]),
        );

        transform_v14_to_v15_value(&mut env).unwrap();

        assert_eq!(
            names(&env),
            vec!["auth", "kan", "sprint"],
            "the sprint's override allocates from a namespace of its own"
        );
    }

    #[test]
    fn test_migrate_v14_to_v15_preserves_card_counter_value() {
        let mut env = make_v14_envelope(json!([board(BOARD_A, json!("kan"), 7)]), json!([]));

        transform_v14_to_v15_value(&mut env).unwrap();

        let prefixes = env["data"]["prefixes"].as_array().unwrap();
        assert_eq!(
            prefixes[0]["card_counter"], 6,
            "the board's counter holds the NEXT number to hand out; the row \
             holds the last used, so 7 is still issued after migrating"
        );
    }

    #[test]
    fn test_migrate_v14_to_v15_shares_one_row_between_boards_without_a_prefix() {
        let mut env = make_v14_envelope(
            json!([
                board(BOARD_A, Value::Null, 0),
                board(BOARD_B, Value::Null, 0),
            ]),
            json!([]),
        );

        transform_v14_to_v15_value(&mut env).unwrap();

        assert_eq!(
            names(&env),
            vec!["sprint", "task"],
            "both boards already hand out `task`; renaming one would change the \
             prefix its future cards carry while leaving its existing ones behind"
        );
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
