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
            version, 6,
            "main DB must still migrate to the current schema even when the backup is skipped"
        );
    });
}

/// KAN-845 review fix: `backup_path_for`'s result may embed characters that
/// need escaping in the `VACUUM INTO '<literal>'` SQL string. Puts the quote
/// in the *parent directory* name (not just the DB filename) so it actually
/// lands in the scratch file's path too - the scratch path is a
/// `tempfile::NamedTempFile`-generated random name inside that same parent
/// directory, so escaping must hold for the directory component, not just a
/// name we happen to control.
#[test]
fn test_open_schema2_db_in_dir_containing_quote_escapes_correctly() {
    let dir = TempDir::new().unwrap();
    let quoted_dir = dir.path().join("board's data");
    std::fs::create_dir(&quoted_dir).unwrap();
    let path = quoted_dir.join("v2.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v2_db(&path, Uuid::nil()).await;

        SqliteStore::open(&path).await.unwrap();

        let backup = SqliteStore::backup_path_for(&path, 2);
        assert!(
            backup.exists(),
            "a parent directory containing a single quote must still produce a VACUUM INTO \
             backup, proving the literal-escaping in write_pre_migration_backup is correct"
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
/// create its target in a read-only directory) and asserts it aborts rather
/// than returning a partial/corrupt result. The pool is connected (which
/// itself needs to create `-wal`/`-shm` sidecars) BEFORE the directory is
/// made read-only, so the induced failure is isolated to the backup step
/// itself rather than an earlier, unrelated failure in WAL-mode setup.
/// Unix-only: relies on directory permission bits to induce the failure.
#[cfg(unix)]
#[test]
fn test_backup_failure_aborts_before_migration() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("v2.db");
    let rt = make_rt();
    rt.block_on(async {
        seed_v2_db(&path, Uuid::nil()).await;

        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true)
            .pragma("journal_mode", "wal");
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(options)
            .await
            .unwrap();

        let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
        let writable = perms.clone();
        perms.set_mode(0o555);
        std::fs::set_permissions(dir.path(), perms).unwrap();

        let result = SqliteStore::write_pre_migration_backup(&pool, &path, 2).await;

        // Restore write permission before TempDir's Drop cleans up.
        std::fs::set_permissions(dir.path(), writable).unwrap();

        assert!(
            result.is_err(),
            "write_pre_migration_backup must fail when it cannot create its scratch file"
        );

        let backup = SqliteStore::backup_path_for(&path, 2);
        assert!(
            !backup.exists(),
            "a failed backup attempt must not leave anything at the final backup path"
        );

        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            version, 2,
            "the source DB itself must be untouched by a failed backup attempt"
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
