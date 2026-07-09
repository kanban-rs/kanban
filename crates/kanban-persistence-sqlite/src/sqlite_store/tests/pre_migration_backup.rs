use std::path::PathBuf;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;
use uuid::Uuid;

use super::super::{SqliteStore, SUPPORTED_SCHEMA_VERSION};
use super::make_rt;
use super::migration_v2_to_v3::seed_v2_db;

#[test]
fn test_open_schema2_db_writes_durable_backup() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v2.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v2_db(&path, Uuid::nil()).await;

        SqliteStore::open(&path).await.unwrap();

        assert!(
            SqliteStore::backup_path_for(&path, 2).exists(),
            "expected a <path>.v2.backup file after opening a schema-2 DB"
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

        let backup = SqliteStore::backup_path_for(&path, 2);
        let backup_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(&backup))
            .await
            .unwrap();

        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(&backup_pool)
            .await
            .unwrap();
        assert_eq!(
            version, 2,
            "backup must be a pre-migration schema_version=2 snapshot"
        );

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
            !SqliteStore::backup_path_for(&path, 3).exists(),
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
                !SqliteStore::backup_path_for(&path, from_version).exists(),
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

        let backup = SqliteStore::backup_path_for(&path, 2);
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

/// KAN-845 review fix: a `.tmp.<pid>` scratch file left behind by a crashed
/// prior backup attempt must not block a fresh backup from being written
/// (the old check-then-VACUUM-INTO-directly implementation would have
/// failed here, since `VACUUM INTO` refuses to overwrite an existing file
/// — except the old code wrote straight to the *final* path, so this
/// specific collision could only happen on the final path, silently
/// trusting the stale partial file forever. The fix routes through a
/// scratch path first, so staleness only ever affects the throwaway
/// scratch file, never the trusted final one).
#[test]
fn test_stale_tmp_backup_file_does_not_block_a_fresh_backup() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v2.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v2_db(&path, Uuid::nil()).await;

        let backup = SqliteStore::backup_path_for(&path, 2);
        let tmp = SqliteStore::tmp_backup_path_for(&backup);
        tokio::fs::write(&tmp, b"stale partial copy from a crashed prior attempt")
            .await
            .unwrap();

        SqliteStore::open(&path).await.unwrap();

        assert!(
            backup.exists(),
            "a stale .tmp scratch file must not block writing a fresh, complete backup"
        );
        assert!(
            !tmp.exists(),
            "the scratch .tmp file must not survive a successful backup"
        );

        let backup_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(&backup))
            .await
            .unwrap();
        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(&backup_pool)
            .await
            .unwrap();
        assert_eq!(
            version, 2,
            "the fresh backup written past the stale scratch file must be a real, valid snapshot"
        );
        backup_pool.close().await;
    });
}

#[test]
fn test_open_schema2_db_at_path_containing_quote_escapes_correctly() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("kanban's board.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v2_db(&path, Uuid::nil()).await;

        SqliteStore::open(&path).await.unwrap();

        let backup = SqliteStore::backup_path_for(&path, 2);
        assert!(
            backup.exists(),
            "a path containing a single quote must still produce a VACUUM INTO backup, \
             proving the literal-escaping in write_pre_migration_backup is correct"
        );

        let backup_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(&backup))
            .await
            .unwrap();
        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(&backup_pool)
            .await
            .unwrap();
        assert_eq!(version, 2);
        backup_pool.close().await;
    });
}

/// Forces a genuine `write_pre_migration_backup` failure (VACUUM INTO cannot
/// create its target in a read-only directory) and asserts `open()` aborts
/// rather than proceeding to the irreversible migration. Unix-only: relies
/// on directory permission bits to induce the failure.
#[cfg(unix)]
#[test]
fn test_backup_failure_aborts_open_before_migration() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v2.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v2_db(&path, Uuid::nil()).await;

        let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
        let writable = perms.clone();
        perms.set_mode(0o555);
        std::fs::set_permissions(dir.path(), perms).unwrap();

        let result = SqliteStore::open(&path).await;

        // Restore write permission before TempDir's Drop cleans up.
        std::fs::set_permissions(dir.path(), writable).unwrap();

        assert!(
            result.is_err(),
            "open() must abort when the backup step fails, not silently continue"
        );

        let backup = SqliteStore::backup_path_for(&path, 2);
        let tmp = SqliteStore::tmp_backup_path_for(&backup);
        assert!(
            !backup.exists(),
            "a failed backup attempt must not leave anything at the final backup path"
        );
        assert!(
            !tmp.exists(),
            "a failed backup attempt must clean up its own scratch .tmp file"
        );

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(&path))
            .await
            .unwrap();
        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            version, 2,
            "irreversible migration must not have run when the backup failed"
        );
    });
}

#[test]
fn test_backup_path_for_uses_v_prefix_matching_json_backend_convention() {
    let path = PathBuf::from("/tmp/kanban.db");
    assert_eq!(
        SqliteStore::backup_path_for(&path, 2),
        PathBuf::from("/tmp/kanban.db.v2.backup"),
        "naming must match the JSON backend's .v{{N}}.backup convention \
         (kanban-persistence-json::migration::backup::pre_latest_backup_path_for)"
    );
}
