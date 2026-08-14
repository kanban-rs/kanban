//! V14 derives every column's `default_status` from its board's
//! `completion_column_ids`, so the stored value agrees with the derivation
//! helpers in `kanban_domain::completion_derivation`.
//!
//! V13 envelopes carry columns whose `default_status` may still be `null`:
//!
//! ```json
//! "boards": [{ "id": "board-1", "completion_column_ids": ["col-2"] }],
//! "columns": [
//!   { "id": "col-1", "board_id": "board-1", "default_status": null },
//!   { "id": "col-2", "board_id": "board-1", "default_status": null }
//! ]
//! ```
//!
//! V14 envelopes carry a non-null `default_status` on every column:
//!
//! ```json
//! "columns": [
//!   { "id": "col-1", "board_id": "board-1", "default_status": "Todo" },
//!   { "id": "col-2", "board_id": "board-1", "default_status": "Done" }
//! ]
//! ```
//!
//! An already-present non-null `default_status` always wins over the
//! derivation. `completion_column_ids` is left in place; it is removed by a
//! later migration.

use std::collections::HashSet;
use std::path::Path;

use kanban_persistence::{PersistenceError, PersistenceResult};
use serde_json::Value;

/// Apply the V14 `default_status` derivation to a JSON file in-place (atomic
/// write). Skips the write when the file is already at version >= 14.
pub(crate) async fn migrate_v13_to_v14(path: &Path) -> PersistenceResult<()> {
    let content = tokio::fs::read_to_string(path).await?;
    let mut envelope: Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v13_to_v14_value(&mut envelope)? {
        return Ok(());
    }
    let out = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    crate::atomic_writer::AtomicWriter::write_atomic(path, out.as_bytes()).await?;
    tracing::info!(
        "Migrated {} from V13 to V14 (default_status derivation)",
        path.display()
    );
    Ok(())
}

/// Pure transform on an already-parsed envelope. Returns `true` if the
/// envelope was changed (needs writing back). Idempotent: a file already at
/// version >= 14 is left untouched (returns `false`).
pub(crate) fn transform_v13_to_v14_value(envelope: &mut Value) -> PersistenceResult<bool> {
    let version = envelope.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version >= 14 {
        return Ok(false);
    }

    let boards: Vec<Value> = envelope
        .get("data")
        .and_then(|d| d.get("boards"))
        .and_then(|b| b.as_array())
        .cloned()
        .unwrap_or_default();

    let mut done_column_ids: HashSet<String> = HashSet::new();
    for board in &boards {
        if let Some(ids) = board.get("completion_column_ids").and_then(Value::as_array) {
            for id in ids {
                if let Some(s) = id.as_str() {
                    done_column_ids.insert(s.to_string());
                }
            }
        }
    }

    if let Some(columns) = envelope
        .get_mut("data")
        .and_then(|d| d.get_mut("columns"))
        .and_then(|c| c.as_array_mut())
    {
        for column in columns.iter_mut() {
            let Some(obj) = column.as_object_mut() else {
                continue;
            };
            let already_set = obj
                .get("default_status")
                .map(|v| !v.is_null())
                .unwrap_or(false);
            if already_set {
                continue;
            }
            let column_id = obj.get("id").and_then(Value::as_str).unwrap_or("");
            let status = if done_column_ids.contains(column_id) {
                "Done"
            } else {
                "Todo"
            };
            obj.insert(
                "default_status".to_string(),
                Value::String(status.to_string()),
            );
        }
    }

    envelope["version"] = Value::Number(14.into());
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    const BOARD: &str = "11111111-1111-1111-1111-111111111111";
    const COLUMN_A: &str = "22222222-2222-2222-2222-222222222222";
    const COLUMN_B: &str = "33333333-3333-3333-3333-333333333333";

    fn make_v13_envelope(boards: Value, columns: Value) -> Value {
        json!({
            "version": 13,
            "metadata": {
                "instance_id": "00000000-0000-0000-0000-000000000001",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": boards,
                "columns": columns,
                "cards": [],
                "archived_cards": [],
                "sprints": [],
                "graph": {
                    "spawns": { "edges": [] },
                    "blocks": { "edges": [] },
                    "relates": { "edges": [] }
                }
            }
        })
    }

    fn board(id: &str, completion_column_ids: Value) -> Value {
        json!({ "id": id, "name": "B", "completion_column_ids": completion_column_ids })
    }

    fn column(id: &str, board_id: &str, default_status: Value) -> Value {
        json!({ "id": id, "board_id": board_id, "default_status": default_status })
    }

    #[test]
    fn test_v13_to_v14_sets_done_for_columns_in_completion_column_ids() {
        let mut env = make_v13_envelope(
            json!([board(BOARD, json!([COLUMN_B]))]),
            json!([
                column(COLUMN_A, BOARD, Value::Null),
                column(COLUMN_B, BOARD, Value::Null),
            ]),
        );

        let changed = transform_v13_to_v14_value(&mut env).unwrap();

        assert!(changed);
        assert_eq!(env["version"], 14);
        assert_eq!(env["data"]["columns"][1]["default_status"], "Done");
    }

    #[test]
    fn test_v13_to_v14_sets_todo_for_other_columns() {
        let mut env = make_v13_envelope(
            json!([board(BOARD, json!([COLUMN_B]))]),
            json!([
                column(COLUMN_A, BOARD, Value::Null),
                column(COLUMN_B, BOARD, Value::Null),
            ]),
        );

        transform_v13_to_v14_value(&mut env).unwrap();

        assert_eq!(env["data"]["columns"][0]["default_status"], "Todo");
    }

    #[test]
    fn test_v13_to_v14_existing_default_status_wins_over_derivation() {
        let mut env = make_v13_envelope(
            json!([board(BOARD, json!([COLUMN_A]))]),
            json!([column(COLUMN_A, BOARD, json!("InProgress"))]),
        );

        transform_v13_to_v14_value(&mut env).unwrap();

        assert_eq!(
            env["data"]["columns"][0]["default_status"], "InProgress",
            "a column already marked as the completion column but carrying \
             an explicit non-Done default_status must keep that explicit value"
        );
    }

    #[test]
    fn test_v13_to_v14_board_with_empty_completion_column_ids_gets_all_todo() {
        let mut env = make_v13_envelope(
            json!([board(BOARD, json!([]))]),
            json!([
                column(COLUMN_A, BOARD, Value::Null),
                column(COLUMN_B, BOARD, Value::Null),
            ]),
        );

        transform_v13_to_v14_value(&mut env).unwrap();

        for column in env["data"]["columns"].as_array().unwrap() {
            assert_eq!(column["default_status"], "Todo");
        }
    }

    #[test]
    fn test_v13_to_v14_leaves_completion_column_ids_in_place() {
        let mut env = make_v13_envelope(
            json!([board(BOARD, json!([COLUMN_A]))]),
            json!([column(COLUMN_A, BOARD, Value::Null)]),
        );

        transform_v13_to_v14_value(&mut env).unwrap();

        assert_eq!(
            env["data"]["boards"][0]["completion_column_ids"],
            json!([COLUMN_A]),
            "completion_column_ids must survive this migration; a later card removes it"
        );
    }

    #[test]
    fn test_v14_migration_is_idempotent() {
        let v14 = json!({
            "version": 14,
            "data": {
                "boards": [{ "id": BOARD, "completion_column_ids": [COLUMN_A] }],
                "columns": [column(COLUMN_A, BOARD, json!("Done"))]
            }
        });
        let mut env = v14.clone();

        let changed = transform_v13_to_v14_value(&mut env).unwrap();

        assert!(!changed);
        assert_eq!(env, v14, "a version:14 envelope must be returned unchanged");
    }

    #[tokio::test]
    async fn test_v13_to_v14_writes_a_v13_backup_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v13.json");
        let env = make_v13_envelope(
            json!([board(BOARD, json!([COLUMN_A]))]),
            json!([column(COLUMN_A, BOARD, Value::Null)]),
        );
        tokio::fs::write(&path, serde_json::to_string_pretty(&env).unwrap())
            .await
            .unwrap();

        super::super::migrator::Migrator::migrate(
            kanban_persistence::FormatVersion::V13,
            kanban_persistence::FormatVersion::MAX,
            &path,
        )
        .await
        .unwrap();

        assert!(
            !path.with_extension("v13.backup").exists(),
            ".v13.backup must be removed after a successful V13 -> V14 migration"
        );
    }
}
