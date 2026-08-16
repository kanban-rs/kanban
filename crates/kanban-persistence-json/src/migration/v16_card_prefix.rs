//! V16 stamps every existing card with the prefix it is addressed by TODAY,
//! freezing its identifier.
//!
//! Before this, a card's prefix was resolved dynamically through its board, so
//! renaming a board's prefix retroactively renamed every card it had already
//! minted. The stored value ends that, but only if the value written here is
//! byte-identical to what the reader resolves — otherwise this migration
//! itself changes the identifiers it exists to preserve.
//!
//! So it does not reimplement the rule. It projects the ids the rule reads off
//! the raw `Value` -- it cannot build domain structs, which now require fields
//! these files predate -- and calls `resolve_card_prefix_by_ids`, which the
//! identifier reader also routes through. Two implementations of one rule is
//! how the JSON and SQLite prefix backfills came to disagree earlier in this
//! epic; the SQLite card-prefix backfill calls the same function.

use std::path::Path;

use kanban_domain::{resolve_card_prefix_by_ids, DEFAULT_CARD_PREFIX};
use kanban_persistence::{PersistenceError, PersistenceResult};
use serde_json::Value;

pub(crate) async fn migrate_v15_to_v16(path: &Path) -> PersistenceResult<()> {
    let content = tokio::fs::read_to_string(path).await?;
    let mut envelope: Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v15_to_v16_value(&mut envelope)? {
        return Ok(());
    }
    let out = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    crate::atomic_writer::AtomicWriter::write_atomic(path, out.as_bytes()).await?;
    tracing::info!(
        "Migrated {} from V15 to V16 (card prefix backfill)",
        path.display()
    );
    Ok(())
}

pub(crate) fn transform_v15_to_v16_value(envelope: &mut Value) -> PersistenceResult<bool> {
    let version = envelope.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version >= 16 {
        return Ok(false);
    }

    // Projected, not deserialized: this runs against files written before
    // `Card`/`Board`/`Sprint` had fields those structs now require.
    let str_of = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);

    let data = envelope.get("data").cloned().unwrap_or(Value::Null);
    let arr = |k: &str| -> Vec<Value> {
        data.get(k)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };

    let uuid_of = |v: &Value, k: &str| {
        v.get(k)
            .and_then(Value::as_str)
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
    };

    // column -> board, board -> prefix, sprint -> prefix override
    let columns: Vec<(uuid::Uuid, uuid::Uuid)> = arr("columns")
        .iter()
        .filter_map(|c| Some((uuid_of(c, "id")?, uuid_of(c, "board_id")?)))
        .collect();
    let boards: Vec<(uuid::Uuid, Option<String>)> = arr("boards")
        .iter()
        .filter_map(|b| Some((uuid_of(b, "id")?, str_of(b, "card_prefix"))))
        .collect();
    let sprints: Vec<(uuid::Uuid, Option<String>)> = arr("sprints")
        .iter()
        .filter_map(|s| Some((uuid_of(s, "id")?, str_of(s, "card_prefix"))))
        .collect();

    let resolve = |card: &Value| -> String {
        let Some(column_id) = uuid_of(card, "column_id") else {
            return DEFAULT_CARD_PREFIX.to_string();
        };
        resolve_card_prefix_by_ids(
            column_id,
            uuid_of(card, "sprint_id"),
            &columns,
            &boards,
            &sprints,
            DEFAULT_CARD_PREFIX,
        )
    };

    if let Some(cards) = envelope
        .get_mut("data")
        .and_then(|d| d.get_mut("cards"))
        .and_then(Value::as_array_mut)
    {
        for card in cards.iter_mut() {
            let resolved = resolve(card);
            if let Some(obj) = card.as_object_mut() {
                obj.insert("prefix".to_string(), Value::String(resolved));
            }
        }
    }

    envelope["version"] = Value::Number(16.into());
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Ids are real UUIDs: the shared resolver takes `Uuid`, so a placeholder
    /// like "b1" parses to nothing and every case would fall through to the
    /// default and pass for the wrong reason.
    ///
    /// Branch-level, not fixture-level. The real-binary fixture in
    /// `tests/v16_card_prefix.rs` proves identity end to end, but it only
    /// exercises an explicit board prefix and the default fallback -- it
    /// contains no card under a sprint override, and no card whose column
    /// disagrees with its `board_id`. Those two branches are the easiest to
    /// get wrong and are covered here.
    fn envelope(cards: Value, columns: Value, boards: Value, sprints: Value) -> Value {
        json!({
            "version": 15,
            "metadata": { "instance_id": "00000000-0000-0000-0000-000000000001",
                          "saved_at": "2024-01-01T00:00:00Z" },
            "data": { "boards": boards, "columns": columns, "cards": cards,
                      "archived_cards": [], "sprints": sprints,
                      "graph": { "spawns": {"edges": []}, "blocks": {"edges": []},
                                 "relates": {"edges": []} } }
        })
    }

    fn prefix_of(env: &Value, idx: usize) -> String {
        env["data"]["cards"][idx]["prefix"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn test_v16_prefers_a_sprint_override_over_the_boards_prefix() {
        let mut env = envelope(
            json!([{ "id": "11111111-1111-1111-1111-111111111111", "column_id": "22222222-2222-2222-2222-222222222222", "board_id": "33333333-3333-3333-3333-333333333333",
                     "sprint_id": "55555555-5555-5555-5555-555555555555", "card_number": 3 }]),
            json!([{ "id": "22222222-2222-2222-2222-222222222222", "board_id": "33333333-3333-3333-3333-333333333333" }]),
            json!([{ "id": "33333333-3333-3333-3333-333333333333", "card_prefix": "KAN" }]),
            json!([{ "id": "55555555-5555-5555-5555-555555555555", "card_prefix": "AUTH" }]),
        );

        transform_v15_to_v16_value(&mut env).unwrap();

        assert_eq!(
            prefix_of(&env, 0),
            "AUTH",
            "a card in an overriding sprint is addressed AUTH-3 today and must \
             be frozen as such, casing included, not as its board's KAN"
        );
    }

    #[test]
    fn test_v16_ignores_a_sprint_without_its_own_prefix() {
        let mut env = envelope(
            json!([{ "id": "11111111-1111-1111-1111-111111111111", "column_id": "22222222-2222-2222-2222-222222222222", "board_id": "33333333-3333-3333-3333-333333333333",
                     "sprint_id": "55555555-5555-5555-5555-555555555555", "card_number": 3 }]),
            json!([{ "id": "22222222-2222-2222-2222-222222222222", "board_id": "33333333-3333-3333-3333-333333333333" }]),
            json!([{ "id": "33333333-3333-3333-3333-333333333333", "card_prefix": "KAN" }]),
            json!([{ "id": "55555555-5555-5555-5555-555555555555", "card_prefix": null }]),
        );

        transform_v15_to_v16_value(&mut env).unwrap();

        assert_eq!(
            prefix_of(&env, 0),
            "KAN",
            "no override means the board wins"
        );
    }

    #[test]
    fn test_v16_follows_the_column_not_the_cards_board_id() {
        let mut env = envelope(
            json!([{ "id": "11111111-1111-1111-1111-111111111111", "column_id": "22222222-2222-2222-2222-222222222222", "board_id": "44444444-4444-4444-4444-444444444444",
                     "sprint_id": null, "card_number": 1 }]),
            json!([{ "id": "22222222-2222-2222-2222-222222222222", "board_id": "33333333-3333-3333-3333-333333333333" }]),
            json!([{ "id": "33333333-3333-3333-3333-333333333333", "card_prefix": "COL" },
                   { "id": "44444444-4444-4444-4444-444444444444", "card_prefix": "FIELD" }]),
            json!([]),
        );

        transform_v15_to_v16_value(&mut env).unwrap();

        assert_eq!(
            prefix_of(&env, 0),
            "COL",
            "the reader resolves through the column's board, so the freeze must \
             too -- following card.board_id would rename this card"
        );
    }

    #[test]
    fn test_v16_falls_back_to_the_default_when_nothing_resolves() {
        let mut env = envelope(
            json!([{ "id": "11111111-1111-1111-1111-111111111111", "column_id": "66666666-6666-6666-6666-666666666666", "board_id": "33333333-3333-3333-3333-333333333333",
                     "sprint_id": null, "card_number": 1 }]),
            json!([]),
            json!([]),
            json!([]),
        );

        transform_v15_to_v16_value(&mut env).unwrap();

        assert_eq!(prefix_of(&env, 0), "task");
    }

    #[test]
    fn test_v16_is_a_noop_on_an_already_migrated_envelope() {
        let mut env = envelope(json!([]), json!([]), json!([]), json!([]));
        env["version"] = json!(16);

        assert!(
            !transform_v15_to_v16_value(&mut env).unwrap(),
            "re-running must not rewrite prefixes a later edit may have changed"
        );
    }
}
