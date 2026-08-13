//! All-entry-point coverage for the V9 → V10 archival reference-marker collapse
//! (MIGRATION-M1). Exercises the async `Migrator::migrate`, the sync
//! `migrate_to_latest_sync` (via `JsonFileStore::load_sync`), and a full
//! multi-version chain, against a REAL V9 fixture that embeds an archived card
//! and an archived board.

use kanban_persistence::{FormatVersion, PersistenceStore};
use kanban_persistence_json::migration::Migrator;
use kanban_persistence_json::JsonFileStore;
use serde_json::Value;
use tempfile::tempdir;

const ARCHIVED_CARD: &str = "33333333-3333-3333-3333-333333333333";
const LIVE_CARD: &str = "44444444-4444-4444-4444-444444444444";
const ARCHIVED_BOARD: &str = "55555555-5555-5555-5555-555555555555";
const LIVE_BOARD: &str = "11111111-1111-1111-1111-111111111111";

/// The genuine V9 fixture: embedded archived card + embedded archived board +
/// live board/column/card. Copied into a temp file per test so migrations run
/// against a writable path.
fn write_v9_fixture(path: &std::path::Path) {
    let fixture = include_str!("fixtures/v9_with_archived_card_and_board.json");
    std::fs::write(path, fixture).unwrap();
}

fn read_json(path: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn ids_in(arr: &Value, key: &str) -> Vec<String> {
    arr.as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e.get(key).and_then(Value::as_str).map(str::to_string))
        .collect()
}

#[tokio::test]
async fn test_migrate_v9_to_max_lifts_embeds_writes_v10_and_removes_backup() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("board.json");
    write_v9_fixture(&path);

    Migrator::migrate(FormatVersion::V9, FormatVersion::MAX, &path)
        .await
        .expect("V9 -> V10 must succeed");

    let after = read_json(&path);
    assert_eq!(
        after["version"], 13,
        "file migrated to current (V13) on disk"
    );

    // The formerly-embedded archived card is now a LIVE row plus a pure marker.
    let card_ids = ids_in(&after["data"]["cards"], "id");
    assert!(
        card_ids.contains(&LIVE_CARD.to_string()),
        "live card preserved"
    );
    assert!(
        card_ids.contains(&ARCHIVED_CARD.to_string()),
        "archived card lifted into live cards"
    );
    let ac = &after["data"]["archived_cards"][0];
    let mut keys: Vec<&str> = ac.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["archived_at", "board_id", "entity_id"]);
    assert_eq!(ac["entity_id"].as_str(), Some(ARCHIVED_CARD));
    assert_eq!(ac["board_id"].as_str(), Some(LIVE_BOARD));

    // The formerly-embedded archived board is now a LIVE row plus a marker.
    let board_ids = ids_in(&after["data"]["boards"], "id");
    assert!(board_ids.contains(&LIVE_BOARD.to_string()));
    assert!(
        board_ids.contains(&ARCHIVED_BOARD.to_string()),
        "archived board lifted into live boards"
    );
    let ab = &after["data"]["archived_boards"][0];
    let mut bkeys: Vec<&str> = ab.as_object().unwrap().keys().map(String::as_str).collect();
    bkeys.sort_unstable();
    assert_eq!(
        bkeys,
        vec!["archived_at", "entity_id"],
        "board marker: NoContext"
    );
    assert_eq!(ab["entity_id"].as_str(), Some(ARCHIVED_BOARD));

    // The pre-chain backup is cleaned up on success.
    assert!(
        !path.with_extension("v9.backup").exists(),
        ".v9.backup removed after a successful V9 -> V10 migration"
    );
}

#[test]
fn test_load_sync_on_v9_fixture_returns_marker_snapshot() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("board.json");
    write_v9_fixture(&path);

    let store = JsonFileStore::new(&path);
    let (snapshot, _meta) = store.load_sync().unwrap().expect("file exists");

    // The Snapshot bytes deserialize into the collapsed shape: the formerly
    // archived card lives under `cards`, and `archived_cards` are pure markers.
    let data: Value = serde_json::from_slice(&snapshot.data).unwrap();
    let card_ids = ids_in(&data["cards"], "id");
    assert!(
        card_ids.contains(&ARCHIVED_CARD.to_string()),
        "archived card present in live cards after load_sync"
    );
    assert_eq!(
        data["archived_cards"][0]["entity_id"].as_str(),
        Some(ARCHIVED_CARD),
        "archived_cards are entity_id markers"
    );
    assert!(data["archived_cards"][0].get("card").is_none());
    assert!(data["archived_cards"][0].get("entity").is_none());
    // On-disk migrated to V10 too.
    assert_eq!(read_json(&path)["version"], 13);
}

#[tokio::test]
async fn test_v9_fixture_deserializes_into_collapsed_domain_snapshot() {
    // End-to-end: load through the store and deserialize into the real
    // `Snapshot`, proving the migrated bytes match the collapsed marker model.
    let dir = tempdir().unwrap();
    let path = dir.path().join("board.json");
    write_v9_fixture(&path);

    let store = JsonFileStore::new(&path);
    let (snapshot, _meta) = store.load().await.unwrap();
    let domain = kanban_persistence::snapshot_from_json_bytes(&snapshot.data)
        .expect("migrated V9 bytes must deserialize into the collapsed Snapshot");

    // cards carries BOTH the live and the formerly-archived card.
    assert!(domain.cards.iter().any(|c| c.id.to_string() == LIVE_CARD));
    assert!(domain
        .cards
        .iter()
        .any(|c| c.id.to_string() == ARCHIVED_CARD));
    // archived_cards is a marker referencing the archived card by entity_id.
    assert_eq!(domain.archived_cards.len(), 1);
    assert_eq!(
        domain.archived_cards[0].entity_id.to_string(),
        ARCHIVED_CARD
    );
    assert_eq!(
        domain.archived_cards[0].context.board_id.to_string(),
        LIVE_BOARD
    );
    // boards carries both the live and the formerly-archived board.
    assert!(domain.boards.iter().any(|b| b.id.to_string() == LIVE_BOARD));
    assert!(domain
        .boards
        .iter()
        .any(|b| b.id.to_string() == ARCHIVED_BOARD));
    assert_eq!(domain.archived_boards.len(), 1);
    assert_eq!(
        domain.archived_boards[0].entity_id.to_string(),
        ARCHIVED_BOARD
    );
}

fn write_v7_fixture(path: &std::path::Path) {
    let fixture = include_str!("fixtures/v7_with_embedded_archived_card.json");
    std::fs::write(path, fixture).unwrap();
}

fn write_v4_fixture(path: &std::path::Path) {
    let fixture = include_str!("fixtures/v4_no_archived_card.json");
    std::fs::write(path, fixture).unwrap();
}

#[tokio::test]
async fn test_load_v7_embedded_archived_card_full_chain_to_collapsed_snapshot() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("board.json");
    write_v7_fixture(&path);

    let store = JsonFileStore::new(&path);
    let (snapshot, _meta) = store.load().await.unwrap();

    // File must have been upgraded to V10 on disk.
    let on_disk: Value = read_json(&path);
    assert_eq!(
        on_disk["version"], 13,
        "file must be migrated to current (V13)"
    );

    // Deserialize into the collapsed Snapshot domain type.
    let domain = kanban_persistence::snapshot_from_json_bytes(&snapshot.data)
        .expect("V7 full-chain migrated bytes must deserialize into collapsed Snapshot");

    // The archived card (33..3) must be a LIVE row in cards[] (lifted by V9→V10).
    assert!(
        domain
            .cards
            .iter()
            .any(|c| c.id.to_string() == ARCHIVED_CARD),
        "archived card must be lifted into live cards[]"
    );
    // The live card (44..4) must still be present.
    assert!(
        domain.cards.iter().any(|c| c.id.to_string() == LIVE_CARD),
        "live card must be preserved in cards[]"
    );

    // archived_cards must be a single marker with entity_id=33..3 and
    // board_id=11..1 (backfilled by V7→V8 from original_column_id → column → board).
    assert_eq!(domain.archived_cards.len(), 1);
    assert_eq!(
        domain.archived_cards[0].entity_id.to_string(),
        ARCHIVED_CARD,
        "archived card marker must reference the archived card"
    );
    assert_eq!(
        domain.archived_cards[0].context.board_id.to_string(),
        LIVE_BOARD,
        "board_id must be backfilled (V7→V8 ran before V9→V10)"
    );

    // No residual card/entity embed in the on-disk archived_cards.
    let ac = &on_disk["data"]["archived_cards"][0];
    assert!(
        ac.get("card").is_none(),
        "no residual 'card' embed after V9→V10"
    );
    assert!(
        ac.get("entity").is_none(),
        "no residual 'entity' embed after V9→V10"
    );
}

#[test]
fn test_load_sync_v7_embedded_archived_card_full_chain_to_collapsed_snapshot() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("board.json");
    write_v7_fixture(&path);

    let store = JsonFileStore::new(&path);
    let (snapshot, _meta) = store.load_sync().unwrap().expect("file exists");

    let domain = kanban_persistence::snapshot_from_json_bytes(&snapshot.data)
        .expect("V7 sync load must deserialize into collapsed Snapshot");

    assert!(
        domain
            .cards
            .iter()
            .any(|c| c.id.to_string() == ARCHIVED_CARD),
        "archived card must be lifted into live cards[] (sync path)"
    );
    assert_eq!(domain.archived_cards.len(), 1);
    assert_eq!(
        domain.archived_cards[0].context.board_id.to_string(),
        LIVE_BOARD,
        "board_id must be backfilled in sync chain too"
    );
}

#[tokio::test]
async fn test_load_v4_full_chain_reaches_collapsed_snapshot() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("board.json");
    write_v4_fixture(&path);

    let store = JsonFileStore::new(&path);
    let (snapshot, _meta) = store.load().await.unwrap();

    // On disk must be current (V13).
    assert_eq!(
        read_json(&path)["version"],
        13,
        "V4 fixture must be migrated to current (V13) via the full chain"
    );

    // Must deserialize cleanly into the collapsed Snapshot.
    kanban_persistence::snapshot_from_json_bytes(&snapshot.data)
        .expect("V4 full-chain migrated bytes must deserialize into collapsed Snapshot");
}
