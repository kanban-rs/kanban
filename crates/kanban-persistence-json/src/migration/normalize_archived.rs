//! Temporary F3b read-shim: lift EMBEDDED archived entities to the pure
//! reference-marker shape at load time.
//!
//! Historical on-disk files embed the archived entity inside each
//! `archived_cards[]` / `archived_boards[]` entry (under the key `entity`, with
//! the legacy alias `card` for cards / `board` for boards) and do NOT carry that
//! entity in the top-level `cards` / `boards` arrays. The collapsed `Snapshot`
//! deserializer (F3b) expects PURE MARKERS (`entity_id`) with every entity
//! living in `cards` / `boards`. This pass normalizes the former to the latter,
//! in place, at read time.
//!
//! It runs AFTER the version-migration chain and BEFORE the bytes are handed to
//! the `Snapshot` deserializer. It does NOT bump `FormatVersion` — this is a
//! transparent read-time normalize.
//!
//! TEMPORARY: MIGRATION-M1 (KAN-874) replaces this with a formal, version-bumped
//! V9 → V10 migration step (embed → reference) that rewrites the file on disk.
//! Until then, this shim keeps genuine old files loadable without a version bump.

use serde_json::Value;

/// Normalize an envelope's `data` object in place: lift any embedded archived
/// entity into the live collection and collapse the entry to a pure marker.
/// Idempotent — a file already in marker shape passes through unchanged.
/// Defensive — missing keys / non-array values are no-ops.
pub(crate) fn normalize_embedded_archived_to_references(data: &mut Value) {
    let Some(obj) = data.as_object_mut() else {
        return;
    };

    lift_embedded(
        obj,
        "archived_cards",
        "cards",
        &["entity", "card"],
        card_marker,
    );
    lift_embedded(
        obj,
        "archived_boards",
        "boards",
        &["entity", "board"],
        board_marker,
    );
}

/// For each entry in `archived_key`, if it embeds an entity under any of
/// `embed_keys`, move that entity into `live_key` (unless already present by id)
/// and replace the entry with the marker produced by `to_marker`.
fn lift_embedded(
    data: &mut serde_json::Map<String, Value>,
    archived_key: &str,
    live_key: &str,
    embed_keys: &[&str],
    to_marker: fn(&Value, &Value) -> Option<Value>,
) {
    // Pull the archived array out so we can also mutate the live array.
    let Some(mut archived) = data
        .get_mut(archived_key)
        .and_then(|v| v.as_array_mut())
        .map(std::mem::take)
    else {
        return;
    };

    // Snapshot the ids already present in the live collection.
    let mut live_ids: std::collections::HashSet<String> = data
        .get(live_key)
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|e| e.get("id").and_then(|v| v.as_str()).map(str::to_string))
        .collect();

    let mut lifted: Vec<Value> = Vec::new();

    for entry in archived.iter_mut() {
        // Already a marker (has entity_id, no embed) → leave unchanged.
        let embedded = embed_keys.iter().find_map(|k| entry.get(*k)).cloned();
        let Some(embedded) = embedded else {
            continue;
        };

        if let Some(marker) = to_marker(&embedded, entry) {
            // Lift the entity into the live collection (dedup by id).
            if let Some(id) = embedded.get("id").and_then(|v| v.as_str()) {
                if live_ids.insert(id.to_string()) {
                    lifted.push(embedded.clone());
                }
            }
            *entry = marker;
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
}

/// Build a card marker `{ entity_id, archived_at, board_id? }` from the embedded
/// card and the original archived-card entry (carries `archived_at` / `board_id`).
fn card_marker(embedded: &Value, entry: &Value) -> Option<Value> {
    let id = embedded.get("id")?.clone();
    let mut marker = serde_json::Map::new();
    marker.insert("entity_id".to_string(), id);
    if let Some(at) = entry.get("archived_at") {
        marker.insert("archived_at".to_string(), at.clone());
    }
    if let Some(board_id) = entry.get("board_id") {
        marker.insert("board_id".to_string(), board_id.clone());
    }
    Some(Value::Object(marker))
}

/// Build a board marker `{ entity_id, archived_at }` (NoContext — no extra
/// fields) from the embedded board and the original archived-board entry.
fn board_marker(embedded: &Value, entry: &Value) -> Option<Value> {
    let id = embedded.get("id")?.clone();
    let mut marker = serde_json::Map::new();
    marker.insert("entity_id".to_string(), id);
    if let Some(at) = entry.get("archived_at") {
        marker.insert("archived_at".to_string(), at.clone());
    }
    Some(Value::Object(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CARD: &str = "33333333-3333-3333-3333-333333333333";
    const BOARD: &str = "11111111-1111-1111-1111-111111111111";

    #[test]
    fn test_embedded_archived_card_is_lifted_to_reference() {
        // Embed shape: the archived card carries the full entity under `card`, is
        // NOT present in `cards`, and has board_id + original_column_id.
        let mut data = json!({
            "cards": [],
            "archived_cards": [{
                "card": { "id": CARD, "title": "T", "column_id": "col-1", "position": 3 },
                "archived_at": "2024-01-01T00:00:00Z",
                "board_id": BOARD,
                "original_column_id": "col-1",
                "original_position": 3
            }]
        });

        normalize_embedded_archived_to_references(&mut data);

        // The card was lifted into the live `cards` array.
        let cards = data["cards"].as_array().unwrap();
        assert_eq!(cards.len(), 1, "the embedded card is lifted into cards");
        assert_eq!(cards[0]["id"].as_str(), Some(CARD));
        assert_eq!(cards[0]["title"].as_str(), Some("T"));

        // The archived entry became a pure marker: entity_id + archived_at +
        // board_id, with NO embedded entity / original_* leftovers.
        let ac = &data["archived_cards"][0];
        assert_eq!(ac["entity_id"].as_str(), Some(CARD));
        assert_eq!(ac["archived_at"].as_str(), Some("2024-01-01T00:00:00Z"));
        assert_eq!(ac["board_id"].as_str(), Some(BOARD), "board_id preserved");
        assert!(ac.get("card").is_none(), "no embedded card key");
        assert!(ac.get("entity").is_none(), "no embedded entity key");
        assert!(ac.get("original_column_id").is_none());
        assert!(ac.get("original_position").is_none());
    }

    #[test]
    fn test_marker_shape_archived_card_passes_through_unchanged() {
        // Already a marker (entity_id, no embed): idempotent no-op, and the card
        // already in `cards` is not duplicated.
        let mut data = json!({
            "cards": [{ "id": CARD, "title": "T" }],
            "archived_cards": [{
                "entity_id": CARD,
                "archived_at": "2024-01-01T00:00:00Z",
                "board_id": BOARD
            }]
        });
        let before = data.clone();

        normalize_embedded_archived_to_references(&mut data);

        assert_eq!(data, before, "a marker-shape file is unchanged");
        assert_eq!(
            data["cards"].as_array().unwrap().len(),
            1,
            "the live card is not duplicated"
        );
    }

    #[test]
    fn test_embedded_archived_board_is_lifted_to_reference() {
        let mut data = json!({
            "boards": [],
            "archived_boards": [{
                "board": { "id": BOARD, "name": "B" },
                "archived_at": "2024-02-02T00:00:00Z"
            }]
        });

        normalize_embedded_archived_to_references(&mut data);

        let boards = data["boards"].as_array().unwrap();
        assert_eq!(boards.len(), 1, "the embedded board is lifted into boards");
        assert_eq!(boards[0]["id"].as_str(), Some(BOARD));

        let ab = &data["archived_boards"][0];
        assert_eq!(ab["entity_id"].as_str(), Some(BOARD));
        assert_eq!(ab["archived_at"].as_str(), Some("2024-02-02T00:00:00Z"));
        assert!(ab.get("board").is_none());
        assert!(ab.get("entity").is_none());
    }

    #[test]
    fn test_missing_keys_are_noop() {
        let mut data = json!({ "boards": [{ "id": BOARD }] });
        let before = data.clone();
        normalize_embedded_archived_to_references(&mut data);
        assert_eq!(data, before, "no archived arrays → nothing changes");
    }

    #[test]
    fn test_embed_not_duplicated_when_card_already_present() {
        // Defensive: if the embed's id already exists in `cards`, don't push a
        // second copy — just collapse the entry to a marker.
        let mut data = json!({
            "cards": [{ "id": CARD, "title": "live" }],
            "archived_cards": [{
                "card": { "id": CARD, "title": "embedded" },
                "archived_at": "2024-01-01T00:00:00Z",
                "board_id": BOARD
            }]
        });

        normalize_embedded_archived_to_references(&mut data);

        let cards = data["cards"].as_array().unwrap();
        assert_eq!(cards.len(), 1, "no duplicate card row");
        assert_eq!(
            cards[0]["title"].as_str(),
            Some("live"),
            "existing row kept"
        );
        assert_eq!(data["archived_cards"][0]["entity_id"].as_str(), Some(CARD));
    }
}
