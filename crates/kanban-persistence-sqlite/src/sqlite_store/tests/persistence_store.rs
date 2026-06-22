use kanban_domain::Snapshot;
use kanban_persistence::{PersistenceMetadata, PersistenceStore, StoreSnapshot};
use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;

#[test]
fn test_sqlitestore_path_is_preserved() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let rt = make_rt();
    let store = rt.block_on(SqliteStore::open(&path)).unwrap();
    assert_eq!(store.path(), path.as_path());
}

#[test]
fn test_sqlitestore_persistence_save_load_roundtrip() {
    use kanban_domain::{Board, DependencyGraph};
    use kanban_persistence::snapshot_to_json_bytes;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let rt = make_rt();

    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("Test Board", None::<String>);
        let snapshot = Snapshot::from_data(
            vec![board],
            vec![],
            vec![],
            vec![],
            vec![],
            DependencyGraph::new(),
        );
        let data = snapshot_to_json_bytes(&snapshot).unwrap();
        let meta = PersistenceMetadata::new(store.instance_id());
        let store_snap = StoreSnapshot {
            data,
            metadata: meta,
        };

        PersistenceStore::save(&store, store_snap).await.unwrap();

        let (loaded_snap, _meta) = PersistenceStore::load(&store).await.unwrap();
        let loaded: Snapshot = serde_json::from_slice(&loaded_snap.data).unwrap();
        assert_eq!(loaded.boards.len(), 1);
        assert_eq!(loaded.boards[0].name, "Test Board");
    });
}

#[test]
fn test_sqlitestore_exists_returns_true_after_open() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        assert!(PersistenceStore::exists(&store).await);
    });
}

#[test]
fn test_save_checkpoints_wal_file_stays_minimal() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let (snapshot, _) = PersistenceStore::load(&store).await.unwrap();
        PersistenceStore::save(&store, snapshot).await.unwrap();
        let wal_path = path.with_extension("sqlite3-wal");
        if wal_path.exists() {
            assert!(
                wal_path.metadata().unwrap().len() < 32 * 1024,
                "WAL file should be minimal after save+checkpoint"
            );
        }
    });
}

#[test]
fn test_load_reports_actual_schema_version_in_metadata() {
    use kanban_domain::DependencyGraph;
    use kanban_persistence::snapshot_to_json_bytes;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("load_drift.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        // Seed the row so load() has something to read.
        let snapshot = Snapshot::from_data(
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            DependencyGraph::new(),
        );
        let data = snapshot_to_json_bytes(&snapshot).unwrap();
        PersistenceStore::save(
            &store,
            StoreSnapshot {
                data,
                metadata: PersistenceMetadata::new(store.instance_id()),
            },
        )
        .await
        .unwrap();
        sqlx::query("UPDATE metadata SET schema_version = 1 WHERE id = 1")
            .execute(&store.pool)
            .await
            .unwrap();

        let (_, meta) = PersistenceStore::load(&store).await.unwrap();
        assert_eq!(
            meta.format_version,
            Some(1),
            "load() must report the DB's actual schema_version, not the const"
        );
    });
}

#[test]
fn test_save_stamps_writer_version_and_commit_into_metadata_row() {
    use kanban_domain::DependencyGraph;
    use kanban_persistence::snapshot_to_json_bytes;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("stamped.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let snapshot = Snapshot::from_data(
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            DependencyGraph::new(),
        );
        let data = snapshot_to_json_bytes(&snapshot).unwrap();
        let store_snap = StoreSnapshot {
            data,
            metadata: PersistenceMetadata::new(store.instance_id()),
        };
        let returned = PersistenceStore::save(&store, store_snap).await.unwrap();

        assert_eq!(
            returned.writer_version.as_deref(),
            Some(kanban_core::KANBAN_VERSION),
        );
        assert_eq!(
            returned.writer_commit.as_deref(),
            Some(kanban_core::KANBAN_COMMIT),
        );

        let row: (Option<String>, Option<String>) =
            sqlx::query_as("SELECT writer_version, writer_commit FROM metadata WHERE id = 1")
                .fetch_one(&store.pool)
                .await
                .unwrap();
        assert_eq!(row.0.as_deref(), Some(kanban_core::KANBAN_VERSION));
        assert_eq!(row.1.as_deref(), Some(kanban_core::KANBAN_COMMIT));
    });
}

#[test]
fn test_load_returns_writer_stamp_from_metadata_row() {
    use kanban_domain::DependencyGraph;
    use kanban_persistence::snapshot_to_json_bytes;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("loaded.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let snapshot = Snapshot::from_data(
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            DependencyGraph::new(),
        );
        let data = snapshot_to_json_bytes(&snapshot).unwrap();
        PersistenceStore::save(
            &store,
            StoreSnapshot {
                data,
                metadata: PersistenceMetadata::new(store.instance_id()),
            },
        )
        .await
        .unwrap();

        let (_, meta) = PersistenceStore::load(&store).await.unwrap();
        assert_eq!(
            meta.writer_version.as_deref(),
            Some(kanban_core::KANBAN_VERSION),
        );
        assert_eq!(
            meta.writer_commit.as_deref(),
            Some(kanban_core::KANBAN_COMMIT),
        );
    });
}
