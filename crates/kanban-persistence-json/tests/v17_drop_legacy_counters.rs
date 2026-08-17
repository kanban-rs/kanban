//! The V16 -> V17 step through a real file, both entry points.
//!
//! The unit tests next to the transform prove the pure function. They cannot
//! prove the file actually reaches disk stripped, that the sync and async
//! orchestrators agree, or that the pre-chain backup is taken and cleaned up.
//! A silent divergence between the two paths would surface only as a user's
//! file keeping keys the other path removes.

use kanban_persistence::PersistenceStore;
use kanban_persistence_json::JsonFileStore;
use serde_json::Value;
use tempfile::TempDir;

fn v16_file(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("board.json");
    let envelope = serde_json::json!({
        "version": 16,
        "metadata": {
            "instance_id": "550e8400-e29b-41d4-a716-446655440000",
            "saved_at": "2024-01-01T00:00:00Z"
        },
        "data": {
            "boards": [{
                "id": "550e8400-e29b-41d4-a716-446655440001",
                "name": "Legacy",
                "card_prefix": "KAN",
                "card_counter": 42,
                "sprint_counters": { "KAN": 7 },
                "next_sprint_number": 3,
                "sprint_names": [],
                "sprint_name_used_count": 0,
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
            }],
            "columns": [], "cards": [], "sprints": [],
            "archived_cards": [], "archived_boards": [],
            "graph": { "blocks": { "edges": [] },
                       "relates": { "edges": [] },
                       "spawns": { "edges": [] } },
            "prefixes": [{ "name": "kan", "card_counter": 41, "sprint_counter": 6 }]
        }
    });
    std::fs::write(&path, serde_json::to_string_pretty(&envelope).unwrap()).unwrap();
    path
}

fn read(path: &std::path::Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn assert_stripped_and_counters_intact(on_disk: &Value, via: &str) {
    assert_eq!(on_disk["version"], 17, "{via}: file must reach V17 on disk");

    let board = &on_disk["data"]["boards"][0];
    assert!(
        board.get("card_counter").is_none(),
        "{via}: card_counter must be gone from the file"
    );
    assert!(
        board.get("sprint_counters").is_none(),
        "{via}: sprint_counters must be gone from the file"
    );

    assert_eq!(
        board["name"], "Legacy",
        "{via}: unrelated fields must survive"
    );
    assert_eq!(board["next_sprint_number"], 3);

    let prefix = &on_disk["data"]["prefixes"][0];
    assert_eq!(
        (
            prefix["card_counter"].as_u64(),
            prefix["sprint_counter"].as_u64()
        ),
        (Some(41), Some(6)),
        "{via}: the rows that now carry the numbering must be untouched"
    );
}

#[tokio::test]
async fn test_load_migrates_a_v16_file_to_v17_and_strips_the_counters() {
    let dir = TempDir::new().unwrap();
    let path = v16_file(&dir);

    let store = JsonFileStore::new(&path);
    store.load().await.unwrap();

    assert_stripped_and_counters_intact(&read(&path), "async load");
}

/// The sync orchestrator is a separate chain that must not drift from the
/// async one. Only its own end state proves it ran the step.
#[test]
fn test_load_sync_migrates_a_v16_file_to_v17_and_strips_the_counters() {
    let dir = TempDir::new().unwrap();
    let path = v16_file(&dir);

    let store = JsonFileStore::new(&path);
    store.load_sync().unwrap();

    assert_stripped_and_counters_intact(&read(&path), "sync load");
}

/// A V16 source had no backup path at all before this branch, so the
/// destructive tail of the chain ran with no recovery point. The backup is
/// taken before the chain and removed once it verifies, so a successful
/// migration leaves no stray file behind.
#[tokio::test]
async fn test_a_v16_migration_leaves_no_backup_behind_on_success() {
    let dir = TempDir::new().unwrap();
    let path = v16_file(&dir);

    let store = JsonFileStore::new(&path);
    store.load().await.unwrap();

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

/// Re-loading a file already at V17 must not rewrite it.
#[tokio::test]
async fn test_reloading_a_v17_file_is_a_noop() {
    let dir = TempDir::new().unwrap();
    let path = v16_file(&dir);

    let store = JsonFileStore::new(&path);
    store.load().await.unwrap();
    let after_first = std::fs::read(&path).unwrap();

    store.load().await.unwrap();

    assert_eq!(
        std::fs::read(&path).unwrap(),
        after_first,
        "a file already at V17 must not be rewritten by a second load"
    );
}
