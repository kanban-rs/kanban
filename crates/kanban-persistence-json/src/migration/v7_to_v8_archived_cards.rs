//! V8 archived-cards first-class retrofit: backfills `board_id` onto each
//! historical `archived_cards` entry that lacks it, reconstructing the value
//! from `original_column_id`→`columns[].board_id`. An archived card whose
//! `original_column_id` no longer resolves to a column keeps a nil `board_id`
//! (unrecoverable, acceptable under the first-class model). Also defensively
//! drops any `cards`-array entry that shadows an archived card by id (JSON
//! never duplicated archived cards in `cards`, so this is a no-op on
//! well-formed files, but it hardens the "archived cards are a discrete peer
//! collection, never duplicated in `cards`" invariant).
//!
//! V7 envelopes carry archived cards without a `board_id` key:
//!
//! ```json
//! "data": {
//!   "columns": [{ "id": "col-1", "board_id": "board-1" }],
//!   "archived_cards": [{ "card": {...}, "original_column_id": "col-1", ... }]
//! }
//! ```
//!
//! V8 envelopes carry the backfilled `board_id`:
//!
//! ```json
//! "archived_cards": [{ "card": {...}, "board_id": "board-1",
//!                      "original_column_id": "col-1", ... }]
//! ```

use kanban_persistence::PersistenceResult;
use serde_json::Value;

/// Pure transform on an already-parsed envelope.
///
/// STUB (red): does nothing. The real V7→V8 backfill is implemented in the
/// green step. Kept as a no-op so the failing unit tests compile and pin the
/// expected behavior.
pub(crate) fn transform_v7_to_v8_value(_envelope: &mut Value) -> PersistenceResult<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const BOARD: &str = "11111111-1111-1111-1111-111111111111";
    const COLUMN: &str = "22222222-2222-2222-2222-222222222222";
    const CARD: &str = "33333333-3333-3333-3333-333333333333";

    fn make_v7_envelope(archived: Value, cards: Value) -> Value {
        json!({
            "version": 7,
            "metadata": {
                "instance_id": "00000000-0000-0000-0000-000000000001",
                "saved_at": "2024-01-01T00:00:00Z"
            },
            "data": {
                "boards": [{ "id": BOARD, "name": "B" }],
                "columns": [{ "id": COLUMN, "board_id": BOARD }],
                "cards": cards,
                "archived_cards": archived,
                "sprints": [],
                "graph": {
                    "spawns": { "edges": [] },
                    "blocks": { "edges": [] },
                    "relates": { "edges": [] }
                }
            }
        })
    }

    fn archived_card(original_column_id: &str) -> Value {
        json!({
            "card": { "id": CARD, "title": "T" },
            "archived_at": "2024-01-01T00:00:00Z",
            "original_column_id": original_column_id,
            "original_position": 0
        })
    }

    #[test]
    fn test_transform_backfills_board_id_from_original_column() {
        let mut env = make_v7_envelope(json!([archived_card(COLUMN)]), json!([]));

        transform_v7_to_v8_value(&mut env).unwrap();

        assert_eq!(env["version"], 8);
        assert_eq!(
            env["data"]["archived_cards"][0]["board_id"]
                .as_str()
                .unwrap(),
            BOARD,
            "board_id must be backfilled from original_column_id -> column.board_id"
        );
    }

    #[test]
    fn test_transform_sets_nil_board_id_when_original_column_missing() {
        let missing = "99999999-9999-9999-9999-999999999999";
        let mut env = make_v7_envelope(json!([archived_card(missing)]), json!([]));

        transform_v7_to_v8_value(&mut env).unwrap();

        assert_eq!(env["version"], 8);
        assert_eq!(
            env["data"]["archived_cards"][0]["board_id"]
                .as_str()
                .unwrap(),
            uuid::Uuid::nil().to_string(),
            "a dangling original_column_id yields a nil board_id, not an error"
        );
    }

    #[test]
    fn test_transform_drops_cards_array_entry_shadowing_archived_card() {
        // An entry present in BOTH cards and archived_cards (by id) must be
        // removed from cards post-transform (defensive first-class invariant).
        let live = json!([{ "id": CARD, "title": "shadow" }, { "id": "keep", "title": "K" }]);
        let mut env = make_v7_envelope(json!([archived_card(COLUMN)]), live);

        transform_v7_to_v8_value(&mut env).unwrap();

        let cards = env["data"]["cards"].as_array().unwrap();
        assert_eq!(cards.len(), 1, "the shadowing live-cards entry is dropped");
        assert_eq!(cards[0]["id"], "keep");
    }

    #[test]
    fn test_transform_is_noop_on_v8_envelope() {
        let v8 = json!({
            "version": 8,
            "data": {
                "columns": [{ "id": COLUMN, "board_id": BOARD }],
                "archived_cards": [archived_card(COLUMN)],
                "cards": []
            }
        });
        let mut env = v8.clone();
        transform_v7_to_v8_value(&mut env).unwrap();
        assert_eq!(env, v8, "a version:8 envelope must be returned unchanged");
    }

    #[test]
    fn test_transform_bumps_version_when_no_archived_cards() {
        let mut env = make_v7_envelope(json!([]), json!([]));
        transform_v7_to_v8_value(&mut env).unwrap();
        assert_eq!(env["version"], 8);
    }

    #[test]
    fn test_transform_preserves_existing_board_id() {
        // If a board_id is already present (e.g. a card archived by a V8
        // binary before the migration ran), it must not be overwritten.
        let existing = "44444444-4444-4444-4444-444444444444";
        let mut ac = archived_card(COLUMN);
        ac.as_object_mut()
            .unwrap()
            .insert("board_id".to_string(), json!(existing));
        let mut env = make_v7_envelope(json!([ac]), json!([]));

        transform_v7_to_v8_value(&mut env).unwrap();

        assert_eq!(
            env["data"]["archived_cards"][0]["board_id"]
                .as_str()
                .unwrap(),
            existing,
            "an already-present board_id must be preserved, not clobbered"
        );
    }
}
