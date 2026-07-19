//! V10 collapses the archival wrapper into a PURE MARKER (F3b).
//!
//! A V9 (and earlier) file EMBEDS the archived entity inside each
//! `archived_cards[]` / `archived_boards[]` entry (under the key `entity`, with
//! the legacy alias `card` for cards / `board` for boards) and does NOT carry
//! that entity in the top-level `cards` / `boards` arrays. The collapsed
//! `Snapshot` (F3b) expects PURE MARKERS with every entity living in the live
//! collections. V10 performs that lift-and-collapse ON DISK:
//!
//! - each embedded entity is MOVED into the live `cards` / `boards` array
//!   (id-collision guard: if the id is already live, the live copy wins and the
//!   embed is dropped — no duplication);
//! - each wrapper is rewritten as the reference marker the domain now
//!   serializes — card `{ "entity_id", "archived_at", "board_id" }`, board
//!   `{ "entity_id", "archived_at" }` — dropping the retired
//!   `original_column_id` / `original_position` restore context.
//!
//! An entry already in marker shape (`entity_id`, no embed) passes through
//! unchanged. This formalizes — and replaces — the temporary F3b read-shim.

use std::path::Path;

use kanban_persistence::{PersistenceError, PersistenceResult};
use serde_json::Value;

/// Apply the V9→V10 archival-reference collapse to a JSON file in-place (atomic
/// write). Skips the write when the file is already at version ≥ 10.
pub(crate) async fn migrate_v9_to_v10(path: &Path) -> PersistenceResult<()> {
    let content = tokio::fs::read_to_string(path).await?;
    let mut envelope: Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v9_to_v10_value(&mut envelope)? {
        return Ok(());
    }
    let out = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    crate::atomic_writer::AtomicWriter::write_atomic(path, out.as_bytes()).await?;
    tracing::info!(
        "Migrated {} from V9 to V10 (archival reference-marker collapse)",
        path.display()
    );
    Ok(())
}

/// Pure transform on an already-parsed envelope. Returns `true` if the envelope
/// was changed (needs writing back). Idempotent: a file already at version ≥ 10
/// is left untouched (returns `false`).
pub(crate) fn transform_v9_to_v10_value(envelope: &mut Value) -> PersistenceResult<bool> {
    let version = envelope.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version >= 10 {
        return Ok(false);
    }

    if let Some(data) = envelope.get_mut("data").and_then(Value::as_object_mut) {
        lift_archived_cards(data)?;
        lift_archived_boards(data)?;
    }

    envelope["version"] = Value::Number(10.into());
    Ok(true)
}

/// Lift embedded cards out of `archived_cards[]` into `cards[]` and rewrite each
/// wrapper as a `{ entity_id, archived_at, board_id }` marker.
fn lift_archived_cards(data: &mut serde_json::Map<String, Value>) -> PersistenceResult<()> {
    lift(
        data,
        "archived_cards",
        "cards",
        &["entity", "card"],
        |embedded, entry| {
            let id = embedded
                .get("id")
                .cloned()
                .ok_or("archived card entity has no `id`")?;
            let mut marker = serde_json::Map::new();
            marker.insert("entity_id".to_string(), id);
            if let Some(at) = entry.get("archived_at") {
                marker.insert("archived_at".to_string(), at.clone());
            }
            if let Some(board_id) = entry.get("board_id") {
                marker.insert("board_id".to_string(), board_id.clone());
            }
            Ok(Value::Object(marker))
        },
    )
}

/// Lift embedded boards out of `archived_boards[]` into `boards[]` and rewrite
/// each wrapper as a `{ entity_id, archived_at }` marker (NoContext).
fn lift_archived_boards(data: &mut serde_json::Map<String, Value>) -> PersistenceResult<()> {
    lift(
        data,
        "archived_boards",
        "boards",
        &["entity", "board"],
        |embedded, entry| {
            let id = embedded
                .get("id")
                .cloned()
                .ok_or("archived board entity has no `id`")?;
            let mut marker = serde_json::Map::new();
            marker.insert("entity_id".to_string(), id);
            if let Some(at) = entry.get("archived_at") {
                marker.insert("archived_at".to_string(), at.clone());
            }
            Ok(Value::Object(marker))
        },
    )
}

/// Generic lift: for each entry in `archived_key`, if it embeds an entity under
/// any `embed_keys`, MOVE the entity into `live_key` (dedup by id — live copy
/// wins) and replace the entry with the marker built by `to_marker`. Entries
/// already in marker shape (no embed) require an `entity_id` and pass through.
fn lift(
    data: &mut serde_json::Map<String, Value>,
    archived_key: &str,
    live_key: &str,
    embed_keys: &[&str],
    to_marker: impl Fn(&Value, &Value) -> Result<Value, &'static str>,
) -> PersistenceResult<()> {
    let Some(mut archived) = data
        .get_mut(archived_key)
        .and_then(Value::as_array_mut)
        .map(std::mem::take)
    else {
        return Ok(());
    };

    let mut live_ids: std::collections::HashSet<String> = data
        .get(live_key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|e| e.get("id").and_then(Value::as_str).map(str::to_string))
        .collect();

    let mut lifted: Vec<Value> = Vec::new();

    for entry in archived.iter_mut() {
        match embed_keys.iter().find_map(|k| entry.get(*k)).cloned() {
            Some(embedded) => {
                let marker = to_marker(&embedded, entry).map_err(|e| {
                    PersistenceError::Serialization(format!("V9→V10 migration: {e}"))
                })?;
                if let Some(id) = embedded.get("id").and_then(Value::as_str) {
                    if live_ids.insert(id.to_string()) {
                        lifted.push(embedded.clone());
                    }
                }
                *entry = marker;
            }
            None => {
                // Already a marker: require entity_id (a malformed entry with
                // neither an embed nor an entity_id is a corrupt file).
                if entry.get("entity_id").is_none() {
                    return Err(PersistenceError::Serialization(format!(
                        "V9→V10 migration: {archived_key} entry has neither an embedded \
                         entity nor an `entity_id`"
                    )));
                }
            }
        }
    }

    if !lifted.is_empty() {
        let live = data
            .entry(live_key.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(arr) = live.as_array_mut() {
            arr.extend(lifted);
        }
    }

    data.insert(archived_key.to_string(), Value::Array(archived));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CARD: &str = "33333333-3333-3333-3333-333333333333";
    const BOARD: &str = "11111111-1111-1111-1111-111111111111";

    fn v9_env(data: Value) -> Value {
        json!({ "version": 9, "data": data })
    }

    #[test]
    fn test_lift_card_into_cards_and_marker_has_exact_keys() {
        let mut env = v9_env(json!({
            "cards": [],
            "archived_cards": [{
                "card": { "id": CARD, "title": "T", "column_id": "col-1", "position": 3 },
                "archived_at": "2024-01-01T00:00:00Z",
                "board_id": BOARD,
                "original_column_id": "col-1",
                "original_position": 3
            }]
        }));

        assert!(transform_v9_to_v10_value(&mut env).unwrap());
        assert_eq!(env["version"], 10, "version bumped to 10");

        // The card is now a live row.
        let cards = env["data"]["cards"].as_array().unwrap();
        assert_eq!(cards.len(), 1, "the embedded card is lifted into cards");
        assert_eq!(cards[0]["id"].as_str(), Some(CARD));
        assert_eq!(cards[0]["title"].as_str(), Some("T"));

        // The marker keys are EXACTLY {entity_id, board_id, archived_at}.
        let ac = env["data"]["archived_cards"][0].as_object().unwrap();
        let mut keys: Vec<&str> = ac.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["archived_at", "board_id", "entity_id"]);
        assert_eq!(ac["entity_id"].as_str(), Some(CARD));
        assert_eq!(ac["board_id"].as_str(), Some(BOARD));
        assert_eq!(ac["archived_at"].as_str(), Some("2024-01-01T00:00:00Z"));
        // No embed / restore-context leftovers.
        assert!(!ac.contains_key("entity"));
        assert!(!ac.contains_key("card"));
        assert!(!ac.contains_key("original_column_id"));
        assert!(!ac.contains_key("original_position"));
    }

    #[test]
    fn test_lift_board_into_boards_and_marker_has_exact_keys() {
        let mut env = v9_env(json!({
            "boards": [],
            "archived_boards": [{
                "board": { "id": BOARD, "name": "B" },
                "archived_at": "2024-02-02T00:00:00Z"
            }]
        }));

        assert!(transform_v9_to_v10_value(&mut env).unwrap());

        let boards = env["data"]["boards"].as_array().unwrap();
        assert_eq!(boards.len(), 1, "the embedded board is lifted into boards");
        assert_eq!(boards[0]["id"].as_str(), Some(BOARD));

        let ab = env["data"]["archived_boards"][0].as_object().unwrap();
        let mut keys: Vec<&str> = ab.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["archived_at", "entity_id"],
            "NoContext: no extra fields"
        );
        assert_eq!(ab["entity_id"].as_str(), Some(BOARD));
        assert_eq!(ab["archived_at"].as_str(), Some("2024-02-02T00:00:00Z"));
        assert!(!ab.contains_key("board"));
        assert!(!ab.contains_key("entity"));
    }

    #[test]
    fn test_entity_alias_key_is_lifted_too() {
        // The current serializer used `entity`; make sure the alias path works.
        let mut env = v9_env(json!({
            "cards": [],
            "archived_cards": [{
                "entity": { "id": CARD, "title": "T" },
                "archived_at": "2024-01-01T00:00:00Z",
                "board_id": BOARD
            }]
        }));
        assert!(transform_v9_to_v10_value(&mut env).unwrap());
        assert_eq!(env["data"]["cards"].as_array().unwrap().len(), 1);
        assert_eq!(
            env["data"]["archived_cards"][0]["entity_id"].as_str(),
            Some(CARD)
        );
        assert!(env["data"]["archived_cards"][0].get("entity").is_none());
    }

    #[test]
    fn test_id_collision_keeps_single_live_card() {
        // The card is ALREADY in `cards`; the embed must not duplicate it.
        let mut env = v9_env(json!({
            "cards": [{ "id": CARD, "title": "live" }],
            "archived_cards": [{
                "card": { "id": CARD, "title": "embedded" },
                "archived_at": "2024-01-01T00:00:00Z",
                "board_id": BOARD
            }]
        }));
        assert!(transform_v9_to_v10_value(&mut env).unwrap());
        let cards = env["data"]["cards"].as_array().unwrap();
        assert_eq!(cards.len(), 1, "no duplicate card row");
        assert_eq!(cards[0]["title"].as_str(), Some("live"), "live copy wins");
        assert_eq!(
            env["data"]["archived_cards"][0]["entity_id"].as_str(),
            Some(CARD)
        );
    }

    #[test]
    fn test_nil_and_absent_board_id_preserved() {
        // nil board_id preserved verbatim.
        let nil = uuid::Uuid::nil().to_string();
        let mut env = v9_env(json!({
            "cards": [],
            "archived_cards": [{
                "card": { "id": CARD },
                "archived_at": "2024-01-01T00:00:00Z",
                "board_id": nil
            }]
        }));
        transform_v9_to_v10_value(&mut env).unwrap();
        assert_eq!(
            env["data"]["archived_cards"][0]["board_id"].as_str(),
            Some(nil.as_str())
        );

        // absent board_id → marker simply omits the key (domain defaults it).
        let mut env2 = v9_env(json!({
            "cards": [],
            "archived_cards": [{
                "card": { "id": CARD },
                "archived_at": "2024-01-01T00:00:00Z"
            }]
        }));
        transform_v9_to_v10_value(&mut env2).unwrap();
        assert!(env2["data"]["archived_cards"][0].get("board_id").is_none());
    }

    #[test]
    fn test_idempotent_on_v10_marker_envelope_returns_false_and_byte_equal() {
        let env0 = json!({
            "version": 10,
            "data": {
                "cards": [{ "id": CARD, "title": "T" }],
                "archived_cards": [{
                    "entity_id": CARD, "archived_at": "2024-01-01T00:00:00Z", "board_id": BOARD
                }],
                "boards": [{ "id": BOARD, "name": "B" }],
                "archived_boards": [{ "entity_id": BOARD, "archived_at": "2024-02-02T00:00:00Z" }]
            }
        });
        let mut env = env0.clone();
        assert!(
            !transform_v9_to_v10_value(&mut env).unwrap(),
            "v10 → no change"
        );
        assert_eq!(env, env0, "byte-equal on an already-v10 envelope");
    }

    #[test]
    fn test_errors_on_entry_with_neither_embed_nor_entity_id() {
        let mut env = v9_env(json!({
            "cards": [],
            "archived_cards": [{ "archived_at": "2024-01-01T00:00:00Z", "board_id": BOARD }]
        }));
        let err = transform_v9_to_v10_value(&mut env).unwrap_err();
        assert!(
            matches!(err, PersistenceError::Serialization(_)),
            "a corrupt entry (no embed, no entity_id) errors"
        );
    }

    #[test]
    fn test_bumps_version_when_no_archived_arrays() {
        let mut env = v9_env(json!({ "boards": [], "cards": [] }));
        assert!(transform_v9_to_v10_value(&mut env).unwrap());
        assert_eq!(env["version"], 10);
    }
}
