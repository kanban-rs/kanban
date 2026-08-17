//! V17 drops the per-board `card_counter` and `sprint_counters` keys.
//!
//! The `prefixes` rows are the sole source of card and sprint numbering, so
//! these carry nothing. Removing them is safe only because V15 already read
//! `card_counter` off the raw envelope to seed those rows, and the chain runs
//! V15 before V17 — strip first and the numbering would be lost, restarting
//! every namespace at 1 and re-minting identifiers that already exist.

use std::path::Path;

use kanban_persistence::{PersistenceError, PersistenceResult};
use serde_json::Value;

pub(crate) async fn migrate_v16_to_v17(path: &Path) -> PersistenceResult<()> {
    let content = tokio::fs::read_to_string(path).await?;
    let mut envelope: Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v16_to_v17_value(&mut envelope)? {
        return Ok(());
    }
    let out = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    crate::atomic_writer::AtomicWriter::write_atomic(path, out.as_bytes()).await?;
    tracing::info!(
        "Migrated {} from V16 to V17 (legacy counters dropped)",
        path.display()
    );
    Ok(())
}

pub(crate) fn transform_v16_to_v17_value(envelope: &mut Value) -> PersistenceResult<bool> {
    let version = envelope.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version >= 17 {
        return Ok(false);
    }

    if let Some(boards) = envelope
        .get_mut("data")
        .and_then(|d| d.get_mut("boards"))
        .and_then(Value::as_array_mut)
    {
        for board in boards.iter_mut() {
            if let Some(obj) = board.as_object_mut() {
                obj.remove("card_counter");
                obj.remove("sprint_counters");
            }
        }
    }

    envelope["version"] = Value::Number(17.into());
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn envelope(version: u64) -> Value {
        json!({
            "version": version,
            "metadata": { "instance_id": "00000000-0000-0000-0000-000000000001",
                          "saved_at": "2024-01-01T00:00:00Z" },
            "data": { "boards": [{
                "id": "33333333-3333-3333-3333-333333333333",
                "name": "B", "card_prefix": "KAN",
                "card_counter": 42,
                "sprint_counters": {"KAN": 7},
                "next_sprint_number": 3,
                "sprint_names": ["Alpha"]
            }], "columns": [], "cards": [], "sprints": [], "archived_cards": [],
            "prefixes": [{"name": "kan", "card_counter": 41, "sprint_counter": 6}] }
        })
    }

    #[test]
    fn test_v17_removes_both_legacy_counter_keys() {
        let mut env = envelope(16);

        transform_v16_to_v17_value(&mut env).unwrap();

        let board = &env["data"]["boards"][0];
        assert!(board.get("card_counter").is_none(), "card_counter remains");
        assert!(
            board.get("sprint_counters").is_none(),
            "sprint_counters remains"
        );
    }

    /// The keys around them are unrelated to this epic and must survive. A
    /// blanket rewrite of the board object would take them too.
    #[test]
    fn test_v17_leaves_every_other_board_field_alone() {
        let mut env = envelope(16);

        transform_v16_to_v17_value(&mut env).unwrap();

        let board = &env["data"]["boards"][0];
        assert_eq!(board["name"], "B");
        assert_eq!(board["card_prefix"], "KAN");
        assert_eq!(board["next_sprint_number"], 3);
        assert_eq!(board["sprint_names"][0], "Alpha");
    }

    /// The prefix rows carry the numbering now; stripping the board keys must
    /// not disturb them or every namespace restarts at 1.
    #[test]
    fn test_v17_leaves_the_prefix_rows_intact() {
        let mut env = envelope(16);

        transform_v16_to_v17_value(&mut env).unwrap();

        assert_eq!(env["data"]["prefixes"][0]["card_counter"], 41);
        assert_eq!(env["data"]["prefixes"][0]["sprint_counter"], 6);
    }

    #[test]
    fn test_v17_is_a_noop_on_an_already_migrated_envelope() {
        let mut env = envelope(17);

        assert!(
            !transform_v16_to_v17_value(&mut env).unwrap(),
            "re-running must not rewrite an envelope already at 17"
        );
    }

    #[test]
    fn test_v17_sets_the_version() {
        let mut env = envelope(16);
        transform_v16_to_v17_value(&mut env).unwrap();
        assert_eq!(env["version"], 17);
    }
}
