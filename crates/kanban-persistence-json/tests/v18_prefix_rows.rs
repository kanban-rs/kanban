//! The V17 -> V18 prefix-row repair through a real file, both entry points.
//!
//! Mirrors `tests/v17_drop_legacy_counters.rs`: unit tests next to the
//! transform prove the pure function, this file proves the file on disk
//! actually reaches V18, that the sync and async orchestrators agree, and
//! that the pre-chain backup is cleaned up on success.

use kanban_persistence::PersistenceStore;
use kanban_persistence_json::JsonFileStore;
use serde_json::{json, Value};
use tempfile::TempDir;

fn base_envelope(cards: Value, prefixes: Value) -> Value {
    json!({
        "version": 17,
        "metadata": {
            "instance_id": "550e8400-e29b-41d4-a716-446655440000",
            "saved_at": "2024-01-01T00:00:00Z"
        },
        "data": {
            "boards": [],
            "columns": [],
            "cards": cards,
            "sprints": [],
            "archived_cards": [],
            "archived_boards": [],
            "graph": { "blocks": { "edges": [] },
                       "relates": { "edges": [] },
                       "spawns": { "edges": [] } },
            "prefixes": prefixes
        }
    })
}

fn base_envelope_with_sprints(
    boards: Value,
    sprints: Value,
    cards: Value,
    prefixes: Value,
) -> Value {
    json!({
        "version": 17,
        "metadata": {
            "instance_id": "550e8400-e29b-41d4-a716-446655440000",
            "saved_at": "2024-01-01T00:00:00Z"
        },
        "data": {
            "boards": boards,
            "columns": [],
            "cards": cards,
            "sprints": sprints,
            "archived_cards": [],
            "archived_boards": [],
            "graph": { "blocks": { "edges": [] },
                       "relates": { "edges": [] },
                       "spawns": { "edges": [] } },
            "prefixes": prefixes
        }
    })
}

fn write(dir: &TempDir, envelope: &Value) -> std::path::PathBuf {
    let path = dir.path().join("board.json");
    std::fs::write(&path, serde_json::to_string_pretty(envelope).unwrap()).unwrap();
    path
}

fn read(path: &std::path::Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn prefix_row<'a>(env: &'a Value, name: &str) -> Option<&'a Value> {
    env["data"]["prefixes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == name)
}

#[tokio::test]
async fn test_v18_inserts_a_row_for_an_unbacked_namespace() {
    let dir = TempDir::new().unwrap();
    let envelope = base_envelope(
        json!([{ "id": "11111111-1111-1111-1111-111111111111", "prefix": "OPS", "card_number": 4 }]),
        json!([]),
    );
    let path = write(&dir, &envelope);

    let store = JsonFileStore::new(&path);
    store.load().await.unwrap();

    let on_disk = read(&path);
    assert_eq!(on_disk["version"], 18);
    let rows = on_disk["data"]["prefixes"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row["name"], "ops");
    assert_eq!(row["card_counter"], 4);
    assert_eq!(row["sprint_counter"], 0);
}

#[tokio::test]
async fn test_v18_uses_the_high_water_mark_across_cards() {
    let dir = TempDir::new().unwrap();
    let envelope = base_envelope(
        json!([
            { "id": "11111111-1111-1111-1111-111111111111", "prefix": "OPS", "card_number": 9 },
            { "id": "22222222-2222-2222-2222-222222222222", "prefix": "OPS", "card_number": 2 }
        ]),
        json!([]),
    );
    let path = write(&dir, &envelope);

    let store = JsonFileStore::new(&path);
    store.load().await.unwrap();

    let on_disk = read(&path);
    let row = prefix_row(&on_disk, "ops").expect("ops row must exist");
    assert_eq!(row["card_counter"], 9);
}

#[tokio::test]
async fn test_v18_raises_a_backed_row_that_lags_its_cards() {
    let dir = TempDir::new().unwrap();
    let envelope = base_envelope(
        json!([{ "id": "11111111-1111-1111-1111-111111111111", "prefix": "KAN", "card_number": 7 }]),
        json!([{ "name": "kan", "card_counter": 3, "sprint_counter": 0 }]),
    );
    let path = write(&dir, &envelope);

    let store = JsonFileStore::new(&path);
    store.load().await.unwrap();

    let on_disk = read(&path);
    let rows = on_disk["data"]["prefixes"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "the row must be raised, not duplicated");
    let row = &rows[0];
    assert_eq!(row["name"], "kan");
    assert_eq!(row["card_counter"], 7);
}

#[tokio::test]
async fn test_v18_never_lowers_a_row_that_already_leads_its_cards() {
    let dir = TempDir::new().unwrap();
    let envelope = base_envelope(
        json!([{ "id": "11111111-1111-1111-1111-111111111111", "prefix": "KAN", "card_number": 2 }]),
        json!([{ "name": "kan", "card_counter": 12, "sprint_counter": 5 }]),
    );
    let path = write(&dir, &envelope);

    let store = JsonFileStore::new(&path);
    store.load().await.unwrap();

    let on_disk = read(&path);
    assert_eq!(on_disk["version"], 18);
    let row = prefix_row(&on_disk, "kan").unwrap();
    assert_eq!(row["card_counter"], 12);
    assert_eq!(row["sprint_counter"], 5);
}

#[tokio::test]
async fn test_v18_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let envelope = base_envelope(
        json!([{ "id": "11111111-1111-1111-1111-111111111111", "prefix": "OPS", "card_number": 4 }]),
        json!([]),
    );
    let path = write(&dir, &envelope);

    let store = JsonFileStore::new(&path);
    store.load().await.unwrap();
    let first = read(&path);
    assert_eq!(first["version"], 18);
    assert!(prefix_row(&first, "ops").is_some());
    let bytes_after_first = std::fs::read(&path).unwrap();

    store.load().await.unwrap();
    let bytes_after_second = std::fs::read(&path).unwrap();

    assert_eq!(bytes_after_first, bytes_after_second);
}

#[tokio::test]
async fn test_v18_leaves_no_backup_behind_on_success() {
    let dir = TempDir::new().unwrap();
    let envelope = base_envelope(
        json!([{ "id": "11111111-1111-1111-1111-111111111111", "prefix": "OPS", "card_number": 4 }]),
        json!([]),
    );
    let path = write(&dir, &envelope);

    let store = JsonFileStore::new(&path);
    store.load().await.unwrap();

    let on_disk = read(&path);
    assert_eq!(on_disk["version"], 18);

    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.contains("backup"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "a verified migration must clean up its backup, found {leftovers:?}"
    );
}

#[tokio::test]
async fn test_v18_handles_empty_prefix_cards_the_way_sqlite_does() {
    let dir = TempDir::new().unwrap();
    let board_id = "33333333-3333-3333-3333-333333333333";
    let column_id = "22222222-2222-2222-2222-222222222222";
    let mut envelope = base_envelope(
        json!([{ "id": "11111111-1111-1111-1111-111111111111", "column_id": column_id,
                 "board_id": board_id, "prefix": "", "card_number": 5 }]),
        json!([]),
    );
    envelope["data"]["boards"] = json!([{
        "id": board_id, "name": "Empty prefix board", "card_prefix": ""
    }]);
    envelope["data"]["columns"] = json!([{ "id": column_id, "board_id": board_id }]);
    let path = write(&dir, &envelope);

    let store = JsonFileStore::new(&path);
    store.load().await.unwrap();

    let on_disk = read(&path);
    assert_eq!(on_disk["version"], 18);
    assert_eq!(on_disk["data"]["cards"][0]["prefix"], "task");
    let row = prefix_row(&on_disk, "task").expect("task row must exist");
    assert_eq!(row["card_counter"], 5);
}

#[tokio::test]
async fn test_v18_returns_an_error_for_an_envelope_with_no_data() {
    let dir = TempDir::new().unwrap();
    let envelope = json!({
        "version": 17,
        "metadata": {
            "instance_id": "550e8400-e29b-41d4-a716-446655440000",
            "saved_at": "2024-01-01T00:00:00Z"
        }
    });
    let path = write(&dir, &envelope);

    let store = JsonFileStore::new(&path);
    let err = store
        .load()
        .await
        .expect_err("missing data must error, not panic");

    assert!(
        matches!(err, kanban_persistence::PersistenceError::Serialization(_)),
        "expected a Serialization error, got {err:?}"
    );
}

#[test]
fn test_load_sync_also_repairs_the_prefix_rows() {
    let dir = TempDir::new().unwrap();
    let envelope = base_envelope(
        json!([{ "id": "11111111-1111-1111-1111-111111111111", "prefix": "OPS", "card_number": 4 }]),
        json!([]),
    );
    let path = write(&dir, &envelope);

    let store = JsonFileStore::new(&path);
    store.load_sync().unwrap();

    let on_disk = read(&path);
    assert_eq!(on_disk["version"], 18);
    let row = prefix_row(&on_disk, "ops").expect("ops row must exist");
    assert_eq!(row["card_counter"], 4);
}

#[tokio::test]
async fn test_v18_preserves_a_sprint_namespace_counter_the_board_no_longer_names() {
    let dir = TempDir::new().unwrap();
    let envelope = base_envelope_with_sprints(
        json!([{ "id": "b1111111-1111-1111-1111-111111111111", "sprint_prefix": null }]),
        json!([
            { "id": "e1111111-1111-1111-1111-111111111111", "board_id": "b1111111-1111-1111-1111-111111111111", "prefix": "QTR", "sprint_number": 2 },
            { "id": "e2222222-2222-2222-2222-222222222222", "board_id": "b1111111-1111-1111-1111-111111111111", "prefix": "QTR", "sprint_number": 1 },
            { "id": "e3333333-3333-3333-3333-333333333333", "board_id": "b1111111-1111-1111-1111-111111111111", "prefix": "sprint", "sprint_number": 1 }
        ]),
        json!([]),
        json!([]),
    );
    let path = write(&dir, &envelope);

    let store = JsonFileStore::new(&path);
    store.load().await.unwrap();

    let on_disk = read(&path);
    assert_eq!(on_disk["version"], 18);
    let qtr = prefix_row(&on_disk, "qtr").expect("qtr row must exist");
    assert_eq!(qtr["sprint_counter"], 2);
    let sprint = prefix_row(&on_disk, "sprint").expect("sprint row must exist");
    assert_eq!(sprint["sprint_counter"], 1);
}

#[tokio::test]
async fn test_v18_reads_the_legacy_prefix_override_key_on_a_sprint() {
    let dir = TempDir::new().unwrap();
    let envelope = base_envelope_with_sprints(
        json!([{ "id": "b1111111-1111-1111-1111-111111111111", "sprint_prefix": null }]),
        json!([
            { "id": "e1111111-1111-1111-1111-111111111111", "board_id": "b1111111-1111-1111-1111-111111111111", "prefix_override": "QTR", "sprint_number": 2 },
            { "id": "e2222222-2222-2222-2222-222222222222", "board_id": "b1111111-1111-1111-1111-111111111111", "prefix_override": "QTR", "sprint_number": 1 }
        ]),
        json!([]),
        json!([]),
    );
    let path = write(&dir, &envelope);

    let store = JsonFileStore::new(&path);
    store.load().await.unwrap();

    let on_disk = read(&path);
    let qtr = prefix_row(&on_disk, "qtr").expect("qtr row must exist");
    assert_eq!(qtr["sprint_counter"], 2);
}
