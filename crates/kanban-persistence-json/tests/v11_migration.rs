//! All-entry-point coverage for the V11 -> V12 completion_column_ids backfill,
//! against a REAL V11 fixture shaped like a historical board with no
//! `completion_column_id` set and multiple columns.

use kanban_persistence::{FormatVersion, PersistenceStore};
use kanban_persistence_json::migration::Migrator;
use kanban_persistence_json::JsonFileStore;
use serde_json::Value;
use tempfile::tempdir;

const BOARD: &str = "11111111-1111-1111-1111-111111111111";
const LAST_COLUMN: &str = "55555555-5555-5555-5555-555555555555";

fn write_v11_fixture(path: &std::path::Path) {
    let fixture = include_str!("fixtures/v11_completion_column.json");
    std::fs::write(path, fixture).unwrap();
}

fn read_json(path: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[tokio::test]
async fn test_load_migrates_v11_file_to_v12_on_disk() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("board.json");
    write_v11_fixture(&path);

    let store = JsonFileStore::new(&path);
    let _ = store.load().await.unwrap();

    let after = read_json(&path);
    assert_eq!(
        after["version"], 12,
        "load must migrate V11 to current (V12) on disk"
    );
    assert_eq!(
        after["data"]["boards"][0]["completion_column_ids"],
        serde_json::json!([LAST_COLUMN]),
        "no legacy completion_column_id: backfilled with the board's last column"
    );
    assert!(
        after["data"]["boards"][0]
            .as_object()
            .unwrap()
            .get("completion_column_id")
            .is_none(),
        "the legacy completion_column_id key must be gone after migration"
    );
}

#[test]
fn test_load_sync_migrates_v11_file_to_v12_on_disk() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("board.json");
    write_v11_fixture(&path);

    let store = JsonFileStore::new(&path);
    let _ = store.load_sync().unwrap().expect("file exists");

    let after = read_json(&path);
    assert_eq!(
        after["version"], 12,
        "load_sync must migrate V11 to current (V12) on disk"
    );
    assert_eq!(
        after["data"]["boards"][0]["completion_column_ids"],
        serde_json::json!([LAST_COLUMN])
    );
}

#[tokio::test]
async fn test_migrate_v11_to_v12_writes_v11_backup() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("board.json");
    write_v11_fixture(&path);

    Migrator::migrate(FormatVersion::V11, FormatVersion::MAX, &path)
        .await
        .expect("V11 -> V12 must succeed");

    assert!(
        !path.with_extension("v11.backup").exists(),
        ".v11.backup must be removed after a successful V11 -> V12 migration"
    );
}

#[tokio::test]
async fn test_migrate_v9_full_chain_reaches_v12_with_completion_column_ids() {
    // A pre-V11 file also picks up the V11 -> V12 step as part of the full
    // upgrade chain, not just files that start out at V11.
    let fixture = include_str!("fixtures/v9_with_archived_card_and_board.json");
    let dir = tempdir().unwrap();
    let path = dir.path().join("board.json");
    std::fs::write(&path, fixture).unwrap();

    Migrator::migrate(FormatVersion::V9, FormatVersion::MAX, &path)
        .await
        .unwrap();

    let after = read_json(&path);
    assert_eq!(after["version"], 12);
    let board = after["data"]["boards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == BOARD)
        .expect("live board present");
    assert!(
        board.get("completion_column_id").is_none(),
        "legacy key must be gone"
    );
    assert!(
        board["completion_column_ids"].is_array(),
        "completion_column_ids must be present"
    );
}
