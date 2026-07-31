use kanban_backend_memory::InMemoryStore;
use kanban_domain::{ArchivedCard, Board, Card, DataStore};
use kanban_persistence::snapshot_to_json_bytes;
use uuid::Uuid;

/// F2: a `Snapshot` produced by `InMemoryStore::snapshot()` with one live and
/// one archived card serializes the archived entity under `archived_cards`
/// ONLY. Guards the reference-model exclusion at the serde seam: no card id may
/// appear in both on-disk collections.
///
/// Not inline in `snapshot_serde.rs`: an inline test links a second, distinct
/// instance of this crate through `kanban-backend-memory`.
#[test]
fn test_snapshot_serde_carries_archived_card_as_live_plus_marker() {
    // Reference-marker model (F3b): EVERY card — live and archived — is the
    // single source of truth in `cards`. `archived_cards` holds a pure marker
    // (`entity_id` references the card in `cards`); nothing is embedded.
    let store = InMemoryStore::new();
    let mut board = Board::new("B", None::<String>);
    let col_id = Uuid::new_v4();
    let live = Card::new(&mut board, col_id, "Live", 0);
    let archived = Card::new(&mut board, col_id, "Archived", 1);
    let archived_id = archived.id;
    store.upsert_card(live).unwrap();
    store.upsert_card(archived.clone()).unwrap();
    store
        .insert_archived_card(ArchivedCard::new(archived_id, board.id))
        .unwrap();

    let snapshot = store.snapshot().unwrap();
    let bytes = snapshot_to_json_bytes(&snapshot).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(
        value["cards"].as_array().unwrap().len(),
        2,
        "both the live and the archived card are serialized under cards"
    );
    assert_eq!(
        value["archived_cards"].as_array().unwrap().len(),
        1,
        "the archived card's marker is serialized under archived_cards"
    );
    let id_str = archived_id.to_string();
    assert!(
        value["cards"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"].as_str() == Some(id_str.as_str())),
        "the archived card's row is present under cards (source of truth)"
    );
    assert_eq!(
        value["archived_cards"][0]["entity_id"].as_str(),
        Some(id_str.as_str()),
        "the marker references the live card by entity_id, never embeds it"
    );
    assert!(
        value["archived_cards"][0]["entity"].is_null(),
        "no embedded entity under the marker"
    );
}
