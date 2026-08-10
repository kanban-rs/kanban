//! V12 makes `Board.completion_column_ids` durable, replacing the legacy
//! single `completion_column_id` key with a list and backfilling it so no
//! board changes behaviour on upgrade.
//!
//! V11 envelopes carry the legacy key, historically always null:
//!
//! ```json
//! "boards": [{ "id": "board-1", "completion_column_id": null }],
//! "columns": [
//!   { "id": "col-1", "board_id": "board-1", "position": 0 },
//!   { "id": "col-2", "board_id": "board-1", "position": 3 }
//! ]
//! ```
//!
//! V12 envelopes drop the legacy key and carry the backfilled list:
//!
//! ```json
//! "boards": [{ "id": "board-1", "completion_column_ids": ["col-2"] }]
//! ```
//!
//! When `completion_column_id` names a live column of the board, that id is
//! carried forward as the sole entry. Otherwise the list is backfilled with
//! the board's last column ordered by `position`, then `created_at`, then
//! `id` -- the same deterministic ordering `sorted_board_columns` uses
//! everywhere else, rather than the non-deterministic `max_by_key` tie-break
//! the old runtime fallback relied on. A board with no columns gets `[]`.

use std::path::Path;

use chrono::{DateTime, Utc};
use kanban_persistence::{PersistenceError, PersistenceResult};
use serde_json::Value;

/// Apply the V12 `completion_column_ids` backfill to a JSON file in-place
/// (atomic write). Skips the write when the file is already at version >= 12.
pub(crate) async fn migrate_v11_to_v12(path: &Path) -> PersistenceResult<()> {
    let content = tokio::fs::read_to_string(path).await?;
    let mut envelope: Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v11_to_v12_value(&mut envelope)? {
        return Ok(());
    }
    let out = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    crate::atomic_writer::AtomicWriter::write_atomic(path, out.as_bytes()).await?;
    tracing::info!(
        "Migrated {} from V11 to V12 (completion_column_ids backfill)",
        path.display()
    );
    Ok(())
}

/// Pure transform on an already-parsed envelope. Returns `true` if the
/// envelope was changed (needs writing back). Idempotent: a file already at
/// version >= 12 is left untouched (returns `false`).
pub(crate) fn transform_v11_to_v12_value(envelope: &mut Value) -> PersistenceResult<bool> {
    let version = envelope.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version >= 12 {
        return Ok(false);
    }

    let columns: Vec<Value> = envelope
        .get("data")
        .and_then(|d| d.get("columns"))
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();

    if let Some(boards) = envelope
        .get_mut("data")
        .and_then(|d| d.get_mut("boards"))
        .and_then(|b| b.as_array_mut())
    {
        for board in boards.iter_mut() {
            let Some(obj) = board.as_object_mut() else {
                continue;
            };
            let legacy_id = obj.remove("completion_column_id").and_then(|v| match v {
                Value::String(s) => Some(s),
                _ => None,
            });
            let Some(board_id) = obj.get("id").and_then(Value::as_str).map(str::to_string) else {
                obj.insert("completion_column_ids".to_string(), Value::Array(vec![]));
                continue;
            };

            let mut board_columns: Vec<&Value> = columns
                .iter()
                .filter(|c| c.get("board_id").and_then(Value::as_str) == Some(board_id.as_str()))
                .collect();

            let ids = if let Some(legacy_id) = legacy_id.as_deref() {
                if board_columns
                    .iter()
                    .any(|c| c.get("id").and_then(Value::as_str) == Some(legacy_id))
                {
                    vec![Value::String(legacy_id.to_string())]
                } else {
                    last_column_id(&mut board_columns)
                }
            } else {
                last_column_id(&mut board_columns)
            };

            obj.insert("completion_column_ids".to_string(), Value::Array(ids));
        }
    }

    envelope["version"] = Value::Number(12.into());
    Ok(true)
}

/// Sort by `position`, then `created_at`, then `id` and return the last
/// column's id as a single-element list, matching
/// `kanban_domain::sorted_board_columns`. Empty input yields `[]`.
///
/// The tie-breaks compare the stored STRINGS where the domain compares
/// `DateTime` and `Uuid`; the orders coincide because both backends write
/// uniform RFC 3339 timestamps and canonical lowercase-hex UUIDs, whose
/// lexicographic order equals the underlying chronological/byte order.
fn last_column_id(columns: &mut [&Value]) -> Vec<Value> {
    columns.sort_by(|a, b| {
        let pos_a = a.get("position").and_then(Value::as_i64).unwrap_or(0);
        let pos_b = b.get("position").and_then(Value::as_i64).unwrap_or(0);
        pos_a
            .cmp(&pos_b)
            .then_with(|| created_at(a).cmp(&created_at(b)))
            .then_with(|| id_str(a).cmp(id_str(b)))
    });
    columns
        .last()
        .and_then(|c| c.get("id"))
        .cloned()
        .into_iter()
        .collect()
}

fn created_at(column: &Value) -> DateTime<Utc> {
    column
        .get("created_at")
        .and_then(Value::as_str)
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_default()
}

fn id_str(column: &Value) -> &str {
    column.get("id").and_then(Value::as_str).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    const BOARD: &str = "11111111-1111-1111-1111-111111111111";
    const OTHER_BOARD: &str = "88888888-8888-8888-8888-888888888888";
    const COLUMN_A: &str = "22222222-2222-2222-2222-222222222222";
    const COLUMN_B: &str = "33333333-3333-3333-3333-333333333333";

    fn make_v11_envelope(boards: Value, columns: Value) -> Value {
        json!({
            "version": 11,
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

    fn board(id: &str, completion_column_id: Option<&str>) -> Value {
        json!({ "id": id, "name": "B", "completion_column_id": completion_column_id })
    }

    fn column(id: &str, board_id: &str, position: i64, created_at: &str) -> Value {
        json!({
            "id": id,
            "board_id": board_id,
            "position": position,
            "created_at": created_at
        })
    }

    #[test]
    fn test_migrate_v11_to_v12_backfills_last_column_by_sorted_order() {
        let mut env = make_v11_envelope(
            json!([board(BOARD, None)]),
            json!([
                column(COLUMN_A, BOARD, 0, "2024-01-01T00:00:00Z"),
                column(COLUMN_B, BOARD, 1, "2024-01-01T00:00:00Z"),
            ]),
        );

        let changed = transform_v11_to_v12_value(&mut env).unwrap();

        assert!(changed);
        assert_eq!(env["version"], 12);
        assert_eq!(
            env["data"]["boards"][0]["completion_column_ids"],
            json!([COLUMN_B]),
            "no legacy id: falls back to the last column by position"
        );
    }

    #[test]
    fn test_migrate_v11_to_v12_preserves_valid_existing_completion_column_id() {
        let mut env = make_v11_envelope(
            json!([board(BOARD, Some(COLUMN_A))]),
            json!([
                column(COLUMN_A, BOARD, 0, "2024-01-01T00:00:00Z"),
                column(COLUMN_B, BOARD, 1, "2024-01-01T00:00:00Z"),
            ]),
        );

        transform_v11_to_v12_value(&mut env).unwrap();

        assert_eq!(
            env["data"]["boards"][0]["completion_column_ids"],
            json!([COLUMN_A]),
            "a legacy id naming a live column of the board must be preserved"
        );
    }

    #[test]
    fn test_migrate_v11_to_v12_drops_dangling_completion_column_id() {
        let dangling = "99999999-9999-9999-9999-999999999999";
        let mut env = make_v11_envelope(
            json!([board(BOARD, Some(dangling))]),
            json!([
                column(COLUMN_A, BOARD, 0, "2024-01-01T00:00:00Z"),
                column(COLUMN_B, BOARD, 1, "2024-01-01T00:00:00Z"),
            ]),
        );

        transform_v11_to_v12_value(&mut env).unwrap();

        assert_eq!(
            env["data"]["boards"][0]["completion_column_ids"],
            json!([COLUMN_B]),
            "a dangling legacy id must fall back to the sorted-last column"
        );
    }

    #[test]
    fn test_migrate_v11_to_v12_ignores_column_of_another_board() {
        let mut env = make_v11_envelope(
            json!([board(BOARD, Some(COLUMN_A))]),
            json!([column(COLUMN_A, OTHER_BOARD, 0, "2024-01-01T00:00:00Z")]),
        );

        transform_v11_to_v12_value(&mut env).unwrap();

        assert_eq!(
            env["data"]["boards"][0]["completion_column_ids"],
            json!([]),
            "a legacy id naming another board's column must not be honoured, \
             and there are no columns of this board to fall back to"
        );
    }

    #[test]
    fn test_migrate_v11_to_v12_board_without_columns_yields_empty_list() {
        let mut env = make_v11_envelope(json!([board(BOARD, None)]), json!([]));

        transform_v11_to_v12_value(&mut env).unwrap();

        assert_eq!(env["data"]["boards"][0]["completion_column_ids"], json!([]));
    }

    #[test]
    fn test_migrate_v11_to_v12_tie_break_prefers_created_at_then_id() {
        let mut env = make_v11_envelope(
            json!([board(BOARD, None)]),
            json!([
                column(COLUMN_B, BOARD, 0, "2024-01-02T00:00:00Z"),
                column(COLUMN_A, BOARD, 0, "2024-01-01T00:00:00Z"),
            ]),
        );

        transform_v11_to_v12_value(&mut env).unwrap();

        assert_eq!(
            env["data"]["boards"][0]["completion_column_ids"],
            json!([COLUMN_B]),
            "equal position: the later created_at sorts last and wins"
        );
    }

    #[test]
    fn test_migrate_v11_to_v12_is_idempotent() {
        let v12 = json!({
            "version": 12,
            "data": {
                "boards": [{ "id": BOARD, "completion_column_ids": [COLUMN_A] }],
                "columns": [column(COLUMN_A, BOARD, 0, "2024-01-01T00:00:00Z")]
            }
        });
        let mut env = v12.clone();

        let changed = transform_v11_to_v12_value(&mut env).unwrap();

        assert!(!changed);
        assert_eq!(env, v12, "a version:12 envelope must be returned unchanged");
    }

    #[test]
    fn test_migrate_v11_to_v12_removes_legacy_completion_column_id_key() {
        let mut env = make_v11_envelope(
            json!([board(BOARD, Some(COLUMN_A))]),
            json!([column(COLUMN_A, BOARD, 0, "2024-01-01T00:00:00Z")]),
        );

        transform_v11_to_v12_value(&mut env).unwrap();

        assert!(
            env["data"]["boards"][0]
                .as_object()
                .unwrap()
                .get("completion_column_id")
                .is_none(),
            "the legacy completion_column_id key must be removed"
        );
    }

    #[tokio::test]
    async fn test_migrate_v11_to_v12_writes_file_with_version_12() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v11.json");
        let env = make_v11_envelope(
            json!([board(BOARD, None)]),
            json!([column(COLUMN_A, BOARD, 0, "2024-01-01T00:00:00Z")]),
        );
        tokio::fs::write(&path, serde_json::to_string_pretty(&env).unwrap())
            .await
            .unwrap();

        migrate_v11_to_v12(&path).await.unwrap();

        let after: Value =
            serde_json::from_str(&tokio::fs::read_to_string(&path).await.unwrap()).unwrap();
        assert_eq!(after["version"], 12);
        assert_eq!(
            after["data"]["boards"][0]["completion_column_ids"],
            json!([COLUMN_A])
        );
    }
}
