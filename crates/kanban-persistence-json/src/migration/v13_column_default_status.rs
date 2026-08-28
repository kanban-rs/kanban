//! V13 makes `Column.default_status` durable, backfilling `default_status:
//! null` onto every existing column so no board changes behaviour on
//! upgrade.
//!
//! V12 envelopes carry columns with no `default_status` key:
//!
//! ```json
//! "columns": [{ "id": "col-1", "board_id": "board-1", "name": "Doing" }]
//! ```
//!
//! V13 envelopes carry the backfilled key on every column:
//!
//! ```json
//! "columns": [{ "id": "col-1", "board_id": "board-1", "name": "Doing", "default_status": null }]
//! ```
//!
//! The backfill never reads the column `name` — it is a pure additive key
//! write, not an inference. A column already carrying `default_status` is
//! left untouched.

use std::path::Path;

use kanban_persistence::{PersistenceError, PersistenceResult};
use serde_json::Value;

/// Apply the V13 `default_status` backfill to a JSON file in-place (atomic
/// write). Skips the write when the file is already at version >= 13.
pub(crate) async fn migrate_v12_to_v13(path: &Path) -> PersistenceResult<()> {
    let content = tokio::fs::read_to_string(path).await?;
    let mut envelope: Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v12_to_v13_value(&mut envelope)? {
        return Ok(());
    }
    let out = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    crate::atomic_writer::AtomicWriter::write_atomic(path, out.as_bytes()).await?;
    tracing::info!(
        "Migrated {} from V12 to V13 (default_status backfill)",
        path.display()
    );
    Ok(())
}

/// Pure transform on an already-parsed envelope. Returns `true` if the
/// envelope was changed (needs writing back). Idempotent: a file already at
/// version >= 13 is left untouched (returns `false`).
pub(crate) fn transform_v12_to_v13_value(envelope: &mut Value) -> PersistenceResult<bool> {
    let version = envelope.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version >= 13 {
        return Ok(false);
    }

    if let Some(columns) = envelope
        .get_mut("data")
        .and_then(|d| d.get_mut("columns"))
        .and_then(|c| c.as_array_mut())
    {
        for column in columns.iter_mut() {
            if let Some(obj) = column.as_object_mut() {
                obj.entry("default_status").or_insert(Value::Null);
            }
        }
    }

    envelope["version"] = Value::Number(13.into());
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn make_v12_envelope(columns: Value) -> Value {
        json!({
            "version": 12,
            "metadata": {
                "instance_id": "00000000-0000-0000-0000-000000000001",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [],
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

    #[test]
    fn test_v12_to_v13_adds_null_default_status_to_every_column() {
        let mut env = make_v12_envelope(json!([
            { "id": "col-1", "board_id": "board-1", "name": "To Do" },
            { "id": "col-2", "board_id": "board-1", "name": "Done" },
        ]));

        let changed = transform_v12_to_v13_value(&mut env).unwrap();

        assert!(changed);
        assert_eq!(env["version"], 13);
        for column in env["data"]["columns"].as_array().unwrap() {
            let obj = column.as_object().unwrap();
            assert!(
                obj.contains_key("default_status"),
                "column {:?} must carry an explicit default_status key",
                obj.get("id")
            );
            assert_eq!(obj["default_status"], Value::Null);
        }
    }

    #[test]
    fn test_v12_to_v13_does_not_infer_status_from_column_name() {
        let mut env = make_v12_envelope(json!([
            { "id": "col-1", "board_id": "board-1", "name": "Doing" }
        ]));

        transform_v12_to_v13_value(&mut env).unwrap();

        let obj = env["data"]["columns"][0].as_object().unwrap();
        assert!(obj.contains_key("default_status"));
        assert_eq!(
            obj["default_status"],
            Value::Null,
            "a column named 'Doing' must still get null, never an inferred status"
        );
    }

    #[test]
    fn test_v12_to_v13_is_idempotent_on_an_already_v13_file() {
        let v13 = json!({
            "version": 13,
            "data": {
                "columns": [{ "id": "col-1", "board_id": "board-1", "default_status": null }]
            }
        });
        let mut env = v13.clone();

        let changed = transform_v12_to_v13_value(&mut env).unwrap();

        assert!(!changed);
        assert_eq!(env, v13, "a version:13 envelope must be returned unchanged");
    }

    #[tokio::test]
    async fn test_migrate_v12_to_v13_writes_file_with_version_13() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v12.json");
        let env = make_v12_envelope(json!([
            { "id": "col-1", "board_id": "board-1", "name": "To Do" }
        ]));
        tokio::fs::write(&path, serde_json::to_string_pretty(&env).unwrap())
            .await
            .unwrap();

        migrate_v12_to_v13(&path).await.unwrap();

        let after: Value =
            serde_json::from_str(&tokio::fs::read_to_string(&path).await.unwrap()).unwrap();
        assert_eq!(after["version"], 13);
        let obj = after["data"]["columns"][0].as_object().unwrap();
        assert!(obj.contains_key("default_status"));
        assert_eq!(obj["default_status"], Value::Null);
    }
}
