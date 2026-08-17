//! All-entry-point coverage for the V12 -> V13 `default_status` backfill,
//! against a REAL V11 fixture (exercising the full upgrade chain) shaped
//! like a historical board with a column named "Doing".

use kanban_persistence::{FormatVersion, PersistenceStore};
use kanban_persistence_json::migration::Migrator;
use kanban_persistence_json::JsonFileStore;
use serde_json::Value;
use tempfile::tempdir;

fn write_v11_fixture(path: &std::path::Path) {
    let fixture = include_str!("fixtures/v11_completion_column.json");
    std::fs::write(path, fixture).unwrap();
}

#[tokio::test]
async fn test_migrating_a_v11_file_writes_exactly_one_v11_backup_and_no_v12_backup() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("board.json");
    write_v11_fixture(&path);

    Migrator::migrate(FormatVersion::V11, FormatVersion::MAX, &path)
        .await
        .expect("V11 -> V14 must succeed");

    assert!(
        !path.with_extension("v11.backup").exists(),
        ".v11.backup must be removed after a successful V11 -> V14 migration"
    );
    assert!(
        !path.with_extension("v12.backup").exists(),
        "the outer pre-latest backup is keyed to the file's OWN starting version (v11), \
         so the internal v12->v13 step must not also leave a stray .v12.backup behind"
    );
}

#[tokio::test]
async fn test_round_trip_preserves_default_status_through_save_and_load() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("board.json");
    write_v11_fixture(&path);

    let store = JsonFileStore::new(&path);
    let (snapshot, metadata) = store.load().await.unwrap();

    let domain = kanban_persistence::snapshot_from_json_bytes(&snapshot.data)
        .expect("migrated V11 bytes must deserialize with default_status present");
    assert!(
        domain.columns.iter().all(|c| c.default_status.is_some()),
        "the full chain now reaches V14, which derives a non-null default_status \
         for every column from completion_column_ids"
    );

    let mut domain = domain;
    domain.columns[1].default_status = Some(kanban_domain::CardStatus::InProgress);
    let data = kanban_persistence::snapshot_to_json_bytes(&domain).unwrap();

    store
        .save(kanban_persistence::StoreSnapshot { data, metadata })
        .await
        .unwrap();

    let (reloaded, _) = store.load().await.unwrap();
    let reloaded_domain = kanban_persistence::snapshot_from_json_bytes(&reloaded.data).unwrap();
    let reloaded_column = reloaded_domain
        .columns
        .iter()
        .find(|c| c.id == domain.columns[1].id)
        .expect("column survives save->reload");
    assert_eq!(
        reloaded_column.default_status,
        Some(kanban_domain::CardStatus::InProgress),
        "an explicitly-set default_status must round-trip through save and load"
    );

    let on_disk: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(on_disk["version"], 17);
}
