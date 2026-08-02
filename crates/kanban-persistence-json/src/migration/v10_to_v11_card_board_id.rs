//! V11 backfills `board_id` onto each historical `cards` entry that lacks it,
//! mirroring V8's `archived_cards.board_id` backfill one layer earlier:
//! `Card.board_id` is now a durable field set at creation and kept in sync on
//! every move (KAN-963), independent of `column_id` -- a card whose column is
//! later deleted (archived cards don't block column deletion) must still
//! resolve its board.
//!
//! V10 envelopes carry live cards without a `board_id` key:
//!
//! ```json
//! "data": {
//!   "columns": [{ "id": "col-1", "board_id": "board-1" }],
//!   "cards": [{ "id": "card-1", "column_id": "col-1", ... }]
//! }
//! ```
//!
//! V11 envelopes carry the backfilled `board_id`:
//!
//! ```json
//! "cards": [{ "id": "card-1", "column_id": "col-1", "board_id": "board-1", ... }]
//! ```

use std::collections::HashMap;
use std::path::Path;

use kanban_persistence::{PersistenceError, PersistenceResult};
use serde_json::Value;

/// Apply the V11 cards.board_id backfill to a JSON file in-place (atomic
/// write). Skips the write when the file is already at version >= 11.
pub(crate) async fn migrate_v10_to_v11(path: &Path) -> PersistenceResult<()> {
    let content = tokio::fs::read_to_string(path).await?;
    let mut envelope: Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v10_to_v11_value(&mut envelope)? {
        return Ok(());
    }
    let out = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    crate::atomic_writer::AtomicWriter::write_atomic(path, out.as_bytes()).await?;
    tracing::info!(
        "Migrated {} from V10 to V11 (cards.board_id backfill)",
        path.display()
    );
    Ok(())
}

/// Pure transform on an already-parsed envelope. Returns `true` if the
/// envelope was changed (needs writing back). Idempotent: a file already at
/// version >= 11 is left untouched (returns `false`).
///
/// For each live card lacking a `board_id` key, backfills it from the
/// `column_id`->column->`board_id` map. A dangling `column_id` (no matching
/// column) yields a nil UUID, matching the tolerance already established for
/// `archived_cards.board_id` in the V8 migration.
pub(crate) fn transform_v10_to_v11_value(envelope: &mut Value) -> PersistenceResult<bool> {
    let version = envelope.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version >= 11 {
        return Ok(false);
    }

    if let Some(data) = envelope.get_mut("data").and_then(|d| d.as_object_mut()) {
        // Build column_id -> board_id map from the live column graph.
        let col_to_board: HashMap<String, Value> = data
            .get("columns")
            .and_then(|c| c.as_array())
            .into_iter()
            .flatten()
            .filter_map(|c| {
                let id = c.get("id")?.as_str()?.to_string();
                let board_id = c.get("board_id")?.clone();
                Some((id, board_id))
            })
            .collect();

        let nil = || Value::String(uuid::Uuid::nil().to_string());

        if let Some(cards) = data.get_mut("cards").and_then(|c| c.as_array_mut()) {
            for card in cards.iter_mut() {
                if card.get("board_id").is_some() {
                    continue;
                }
                let board_id = card
                    .get("column_id")
                    .and_then(|v| v.as_str())
                    .and_then(|c| col_to_board.get(c))
                    .cloned()
                    .unwrap_or_else(nil);
                if let Some(obj) = card.as_object_mut() {
                    obj.insert("board_id".to_string(), board_id);
                }
            }
        }
    }

    envelope["version"] = Value::Number(11.into());
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    const BOARD: &str = "11111111-1111-1111-1111-111111111111";
    const COLUMN: &str = "22222222-2222-2222-2222-222222222222";
    const CARD: &str = "33333333-3333-3333-3333-333333333333";

    fn make_v10_envelope(cards: Value) -> Value {
        json!({
            "version": 10,
            "metadata": {
                "instance_id": "00000000-0000-0000-0000-000000000001",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [{ "id": BOARD, "name": "B" }],
                "columns": [{ "id": COLUMN, "board_id": BOARD }],
                "cards": cards,
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

    fn card(column_id: &str) -> Value {
        json!({ "id": CARD, "column_id": column_id, "title": "T" })
    }

    #[test]
    fn test_transform_backfills_board_id_from_column_id() {
        let mut env = make_v10_envelope(json!([card(COLUMN)]));

        let changed = transform_v10_to_v11_value(&mut env).unwrap();

        assert!(changed);
        assert_eq!(env["version"], 11);
        assert_eq!(
            env["data"]["cards"][0]["board_id"].as_str().unwrap(),
            BOARD,
            "board_id must be backfilled from column_id -> column.board_id"
        );
    }

    #[test]
    fn test_transform_sets_nil_board_id_when_column_missing() {
        let missing = "99999999-9999-9999-9999-999999999999";
        let mut env = make_v10_envelope(json!([card(missing)]));

        transform_v10_to_v11_value(&mut env).unwrap();

        assert_eq!(
            env["data"]["cards"][0]["board_id"].as_str().unwrap(),
            uuid::Uuid::nil().to_string(),
            "a dangling column_id yields a nil board_id, not an error"
        );
    }

    #[test]
    fn test_transform_preserves_existing_board_id() {
        // If board_id is already present (e.g. a card created by a V11 binary
        // before the migration ran), it must not be overwritten.
        let existing = "44444444-4444-4444-4444-444444444444";
        let mut c = card(COLUMN);
        c.as_object_mut()
            .unwrap()
            .insert("board_id".to_string(), json!(existing));
        let mut env = make_v10_envelope(json!([c]));

        transform_v10_to_v11_value(&mut env).unwrap();

        assert_eq!(
            env["data"]["cards"][0]["board_id"].as_str().unwrap(),
            existing,
            "an already-present board_id must be preserved, not clobbered"
        );
    }

    #[test]
    fn test_transform_is_noop_on_v11_envelope() {
        let v11 = json!({
            "version": 11,
            "data": {
                "columns": [{ "id": COLUMN, "board_id": BOARD }],
                "cards": [card(COLUMN)]
            }
        });
        let mut env = v11.clone();
        let changed = transform_v10_to_v11_value(&mut env).unwrap();
        assert!(!changed);
        assert_eq!(env, v11, "a version:11 envelope must be returned unchanged");
    }

    #[test]
    fn test_transform_bumps_version_when_no_cards() {
        let mut env = make_v10_envelope(json!([]));
        let changed = transform_v10_to_v11_value(&mut env).unwrap();
        assert!(changed);
        assert_eq!(env["version"], 11);
    }

    #[tokio::test]
    async fn test_migrate_v10_to_v11_writes_file_with_version_11() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("v10.json");
        let env = make_v10_envelope(json!([card(COLUMN)]));
        tokio::fs::write(&path, serde_json::to_string_pretty(&env).unwrap())
            .await
            .unwrap();

        migrate_v10_to_v11(&path).await.unwrap();

        let after: Value =
            serde_json::from_str(&tokio::fs::read_to_string(&path).await.unwrap()).unwrap();
        assert_eq!(after["version"], 11);
        assert_eq!(
            after["data"]["cards"][0]["board_id"].as_str().unwrap(),
            BOARD
        );
    }
}
