//! V18 repairs the `prefixes` array against the cards that name it.
//!
//! Runs the same two phases `SqliteStore` runs on every open: first any card
//! still carrying no prefix is stamped with the one it is addressed by
//! today, then a row is inserted or raised for every namespace a card names,
//! never lowering or renaming an existing row. Both phases route through the
//! shared `kanban_domain` functions the SQLite backend uses
//! (`resolve_card_prefix_by_ids`, `counters_implied_by`,
//! `merge_counter_rows`), so the two backends cannot derive different rows
//! for identical logical content.

use std::path::Path;

use chrono::{DateTime, Utc};
use kanban_domain::{
    resolve_card_prefix_by_ids, Card, CardPriority, CardRecord, CardStatus, Prefix,
    DEFAULT_CARD_PREFIX,
};
use kanban_persistence::{PersistenceError, PersistenceResult};
use serde_json::Value;
use uuid::Uuid;

pub(crate) async fn migrate_v17_to_v18(path: &Path) -> PersistenceResult<()> {
    let content = tokio::fs::read_to_string(path).await?;
    let mut envelope: Value = serde_json::from_str(&content)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    if !transform_v17_to_v18_value(&mut envelope)? {
        return Ok(());
    }
    let out = serde_json::to_string_pretty(&envelope)
        .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
    crate::atomic_writer::AtomicWriter::write_atomic(path, out.as_bytes()).await?;
    tracing::info!(
        "Migrated {} from V17 to V18 (prefix rows repaired)",
        path.display()
    );
    Ok(())
}

pub(crate) fn transform_v17_to_v18_value(envelope: &mut Value) -> PersistenceResult<bool> {
    let version = envelope.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version >= 18 {
        return Ok(false);
    }

    stamp_empty_card_prefixes(envelope);
    repair_unbacked_card_namespaces(envelope);

    envelope["version"] = Value::Number(18.into());
    Ok(true)
}

fn str_of(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}

fn uuid_of(v: &Value, k: &str) -> Option<Uuid> {
    v.get(k)
        .and_then(Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok())
}

fn array_of(data: &Value, k: &str) -> Vec<Value> {
    data.get(k)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn stamp_empty_card_prefixes(envelope: &mut Value) {
    let data = envelope.get("data").cloned().unwrap_or(Value::Null);
    let columns: Vec<(Uuid, Uuid)> = array_of(&data, "columns")
        .iter()
        .filter_map(|c| Some((uuid_of(c, "id")?, uuid_of(c, "board_id")?)))
        .collect();
    let boards: Vec<(Uuid, Option<String>)> = array_of(&data, "boards")
        .iter()
        .filter_map(|b| Some((uuid_of(b, "id")?, str_of(b, "card_prefix"))))
        .collect();
    let sprints: Vec<(Uuid, Option<String>)> = array_of(&data, "sprints")
        .iter()
        .filter_map(|s| Some((uuid_of(s, "id")?, str_of(s, "card_prefix"))))
        .collect();

    let Some(cards) = envelope
        .get_mut("data")
        .and_then(|d| d.get_mut("cards"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    for card in cards.iter_mut() {
        let already_has_prefix = card
            .get("prefix")
            .and_then(Value::as_str)
            .is_some_and(|p| !p.is_empty());
        if already_has_prefix {
            continue;
        }

        let resolved = match uuid_of(card, "column_id") {
            Some(column_id) => resolve_card_prefix_by_ids(
                column_id,
                uuid_of(card, "sprint_id"),
                &columns,
                &boards,
                &sprints,
                None,
            ),
            None => DEFAULT_CARD_PREFIX.to_string(),
        };
        let resolved = if resolved.is_empty() {
            DEFAULT_CARD_PREFIX.to_string()
        } else {
            resolved
        };

        if let Some(obj) = card.as_object_mut() {
            obj.insert("prefix".to_string(), Value::String(resolved));
        }
    }
}

fn stamped_cards(envelope: &Value) -> Vec<Card> {
    let data = envelope.get("data").cloned().unwrap_or(Value::Null);
    array_of(&data, "cards")
        .iter()
        .filter_map(|card| {
            let prefix = str_of(card, "prefix").unwrap_or_default();
            let card_number = card.get("card_number").and_then(Value::as_u64).unwrap_or(0) as u32;
            Card::reconstitute(CardRecord {
                id: Uuid::nil(),
                column_id: Uuid::nil(),
                board_id: Uuid::nil(),
                title: String::new(),
                description: None,
                priority: CardPriority::Medium,
                status: CardStatus::Todo,
                position: 0,
                due_date: None,
                points: None,
                card_number,
                prefix,
                sprint_id: None,
                created_at: DateTime::<Utc>::UNIX_EPOCH,
                updated_at: DateTime::<Utc>::UNIX_EPOCH,
                completed_at: None,
                sprint_logs: Vec::new(),
            })
            .ok()
        })
        .collect()
}

fn existing_prefixes(envelope: &Value) -> Vec<Prefix> {
    let data = envelope.get("data").cloned().unwrap_or(Value::Null);
    array_of(&data, "prefixes")
        .iter()
        .filter_map(|row| {
            let name = str_of(row, "name")?;
            let card_counter = row.get("card_counter").and_then(Value::as_u64).unwrap_or(0) as u32;
            let sprint_counter = row
                .get("sprint_counter")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32;
            Some(Prefix {
                name,
                card_counter,
                sprint_counter,
            })
        })
        .collect()
}

fn repair_unbacked_card_namespaces(envelope: &mut Value) {
    let cards = stamped_cards(envelope);
    let existing = existing_prefixes(envelope);

    let mut target = kanban_domain::counters_implied_by(&cards, &[], &[], &[], None);
    kanban_domain::merge_counter_rows(&mut target, &existing);

    let prefixes = envelope.get_mut("data").and_then(|d| d.get_mut("prefixes"));
    let prefixes = match prefixes {
        Some(p) if p.is_array() => p,
        _ => {
            if let Some(data) = envelope.get_mut("data").and_then(Value::as_object_mut) {
                data.insert("prefixes".to_string(), Value::Array(Vec::new()));
            }
            envelope
                .get_mut("data")
                .and_then(|d| d.get_mut("prefixes"))
                .expect("prefixes was just inserted")
        }
    };
    let rows = prefixes.as_array_mut().expect("prefixes is an array");

    for row in &target {
        let existing_row = rows
            .iter_mut()
            .find(|r| r.get("name").and_then(Value::as_str) == Some(row.name.as_str()));
        match existing_row {
            Some(existing_row) => {
                let current_card = existing_row
                    .get("card_counter")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32;
                let current_sprint = existing_row
                    .get("sprint_counter")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as u32;
                if let Some(obj) = existing_row.as_object_mut() {
                    obj.insert(
                        "card_counter".to_string(),
                        Value::Number(current_card.max(row.card_counter).into()),
                    );
                    obj.insert(
                        "sprint_counter".to_string(),
                        Value::Number(current_sprint.max(row.sprint_counter).into()),
                    );
                }
            }
            None => {
                rows.push(serde_json::json!({
                    "name": row.name,
                    "card_counter": row.card_counter,
                    "sprint_counter": row.sprint_counter,
                }));
            }
        }
    }
}
