use std::path::{Path, PathBuf};

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;
use uuid::Uuid;

use super::super::{SqliteStore, SUPPORTED_SCHEMA_VERSION};
use super::make_rt;
use super::migration_v2_to_v3::seed_v2_db;

fn backup_path(db_path: &Path, from_version: u32) -> PathBuf {
    let mut backup = db_path.as_os_str().to_owned();
    backup.push(format!(".schema{from_version}.backup"));
    PathBuf::from(backup)
}

#[test]
fn test_open_schema2_db_writes_durable_backup() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v2.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v2_db(&path, Uuid::nil()).await;

        SqliteStore::open(&path).await.unwrap();

        assert!(
            backup_path(&path, 2).exists(),
            "expected a <path>.schema2.backup file after opening a schema-2 DB"
        );
    });
}

#[test]
fn test_backup_reflects_pre_migration_state() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v2.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v2_db(&path, Uuid::nil()).await;

        SqliteStore::open(&path).await.unwrap();

        let backup = backup_path(&path, 2);
        let backup_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(&backup))
            .await
            .unwrap();

        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(&backup_pool)
            .await
            .unwrap();
        assert_eq!(version, 2, "backup must be a pre-migration schema_version=2 snapshot");

        let has_board_id: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('archived_cards') WHERE name = 'board_id'",
        )
        .fetch_one(&backup_pool)
        .await
        .unwrap();
        assert!(
            !has_board_id,
            "backup must predate the archived_cards.board_id migration"
        );

        backup_pool.close().await;
    });
}

#[test]
fn test_open_schema3_db_writes_no_backup() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v3.db");
    let rt = make_rt();
    rt.block_on(async {
        // First open creates a fresh (already-current) schema-3 DB.
        SqliteStore::open(&path).await.unwrap();

        // Second open sees pre_migrate_version == SUPPORTED_SCHEMA_VERSION.
        SqliteStore::open(&path).await.unwrap();

        assert!(
            !backup_path(&path, 3).exists(),
            "an already-current DB must not get a backup written"
        );
    });
}

#[test]
fn test_open_fresh_db_writes_no_backup() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("fresh.db");
    let rt = make_rt();
    rt.block_on(async {
        SqliteStore::open(&path).await.unwrap();

        for from_version in 0..=SUPPORTED_SCHEMA_VERSION {
            assert!(
                !backup_path(&path, from_version).exists(),
                "a fresh DB (no prior metadata table) must not get a backup written"
            );
        }
    });
}

#[test]
fn test_existing_backup_not_clobbered() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v2.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v2_db(&path, Uuid::nil()).await;

        let backup = backup_path(&path, 2);
        let sentinel = b"not a real sqlite file, must survive untouched";
        tokio::fs::write(&backup, sentinel).await.unwrap();

        let store = SqliteStore::open(&path).await.unwrap();

        let on_disk = tokio::fs::read(&backup).await.unwrap();
        assert_eq!(
            on_disk, sentinel,
            "a pre-existing backup file must never be overwritten"
        );

        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(
            version, 3,
            "main DB must still migrate to the current schema even when the backup is skipped"
        );
    });
}
