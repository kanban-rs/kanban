//! All-entry-point coverage for the V13 -> V14 `default_status` derivation,
//! against a REAL V11 fixture (exercising the full upgrade chain) shaped
//! like a historical board with multiple columns and no explicit
//! `default_status` set on any of them.

use kanban_persistence::{FormatVersion, PersistenceStore};
use kanban_persistence_json::migration::Migrator;
use kanban_persistence_json::JsonFileStore;
use serde_json::Value;
use tempfile::tempdir;

const LAST_COLUMN: &str = "55555555-5555-5555-5555-555555555555";
const OTHER_COLUMN: &str = "22222222-2222-2222-2222-222222222222";

fn write_v11_fixture(path: &std::path::Path) {
    let fixture = include_str!("fixtures/v11_completion_column.json");
    std::fs::write(path, fixture).unwrap();
}

fn read_json(path: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[tokio::test]
async fn test_full_chain_v1_to_v14_round_trips() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("board.json");
    write_v11_fixture(&path);

    Migrator::migrate(FormatVersion::V11, FormatVersion::MAX, &path)
        .await
        .expect("V11 -> V14 must succeed");

    let after = read_json(&path);
    assert_eq!(after["version"], 17, "chain must reach the current format");

    let columns = after["data"]["columns"].as_array().unwrap();
    let completion_column = columns
        .iter()
        .find(|c| c["id"] == LAST_COLUMN)
        .expect("completion column present");
    assert_eq!(
        completion_column["default_status"], "Done",
        "the backfilled completion column must derive to Done"
    );
    let other_column = columns
        .iter()
        .find(|c| c["id"] == OTHER_COLUMN)
        .expect("other column present");
    assert_eq!(
        other_column["default_status"], "Todo",
        "a column outside completion_column_ids must derive to Todo"
    );

    let board = after["data"]["boards"][0]
        .as_object()
        .expect("board present");
    assert!(
        board.contains_key("completion_column_ids"),
        "completion_column_ids must still be present after V14; a later card removes it"
    );
}

#[tokio::test]
async fn test_sync_and_async_chains_produce_identical_v14_output() {
    let dir = tempdir().unwrap();
    let async_path = dir.path().join("async.json");
    let sync_path = dir.path().join("sync.json");
    write_v11_fixture(&async_path);
    write_v11_fixture(&sync_path);

    Migrator::migrate(FormatVersion::V11, FormatVersion::MAX, &async_path)
        .await
        .expect("async V11 -> V14 must succeed");

    let sync_store = JsonFileStore::new(&sync_path);
    let _ = sync_store.load_sync().unwrap().expect("file exists");

    let async_after = read_json(&async_path);
    let sync_after = read_json(&sync_path);

    assert_eq!(
        async_after, sync_after,
        "the async migration chain and the sync migration chain must produce identical output"
    );
}
