use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tempfile::TempDir;

use super::super::{SqliteStore, SUPPORTED_SCHEMA_VERSION};
use super::make_rt;
use kanban_domain::Board;

#[test]
fn test_checkpoint_executes_without_error() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        store.checkpoint().await.unwrap();
    });
}

#[test]
fn test_read_metadata_sync_reflects_actual_schema_version_from_db_row() {
    // Contract: PersistenceMetadata.format_version is the format the DB
    // is currently at, not whatever the binary's SUPPORTED happens to be.
    // After open(), the two coincide (migrate normalises). We manually
    // UPDATE schema_version below to a value that differs from SUPPORTED
    // and confirm the read reflects the DB, not the const.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("drift.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        sqlx::query("UPDATE metadata SET schema_version = 1 WHERE id = 1")
            .execute(&store.pool)
            .await
            .unwrap();

        let meta = store
            .read_metadata_sync()
            .unwrap()
            .expect("metadata row should exist");
        assert_eq!(
            meta.format_version,
            Some(1),
            "format_version must reflect the DB row (1), not the binary's SUPPORTED ({SUPPORTED_SCHEMA_VERSION})"
        );
    });
}

#[test]
fn test_stamp_writer_updates_metadata_row_and_returns_timestamp() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("stamped.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        // Wipe what migrate-on-open already wrote so the assertion is clean.
        sqlx::query("UPDATE metadata SET writer_version = NULL, writer_commit = NULL WHERE id = 1")
            .execute(&store.pool)
            .await
            .unwrap();

        let before = Utc::now();
        let stamped_at = store.stamp_writer().await.unwrap();
        let after = Utc::now();

        assert!(
            stamped_at >= before && stamped_at <= after,
            "returned timestamp must be in [before, after]: {stamped_at:?}"
        );

        let (saved_at_str, wv, wc): (String, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT saved_at, writer_version, writer_commit FROM metadata WHERE id = 1",
        )
        .fetch_one(&store.pool)
        .await
        .unwrap();
        assert_eq!(wv.as_deref(), Some(kanban_core::KANBAN_VERSION));
        assert_eq!(wc.as_deref(), Some(kanban_core::KANBAN_COMMIT));
        assert_eq!(saved_at_str, stamped_at.to_rfc3339());
    });
}

#[test]
fn test_stamp_writer_waits_for_ambient_transaction_instead_of_erroring_locked() {
    // stamp_writer/checkpoint go through the raw pool, bypassing active_tx,
    // so a concurrent writer that collides with an in-flight ambient
    // transaction must wait (busy_timeout) rather than immediately fail
    // with "database is locked". The ambient transaction here is held for
    // 6s, past sqlx-sqlite's own 5s default busy_timeout, so this only
    // passes once SqliteStore configures a longer one itself.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("busy.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = Arc::new(SqliteStore::open(&path).await.unwrap());

        let mut board = Board::new("B", None::<String>);
        board.id = uuid::Uuid::new_v4();
        let board2 = board.clone();

        store.begin_ambient_transaction().await.unwrap();
        store
            .db_conn(|conn| {
                Box::pin(async move { SqliteStore::write_board_with_conn(conn, &board2).await })
            })
            .await
            .unwrap();

        let store_clone = Arc::clone(&store);
        let handle = tokio::spawn(async move { store_clone.stamp_writer().await });

        tokio::time::sleep(Duration::from_secs(6)).await;
        store.finish_ambient_transaction(true).await.unwrap();

        let result = handle.await.unwrap();
        assert!(
            result.is_ok(),
            "stamp_writer must wait for the in-flight ambient transaction to \
             commit instead of immediately erroring with 'database is locked': {result:?}"
        );
    });
}

#[test]
fn test_checkpoint_alone_does_not_stamp_writer_fields() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ckpt_only.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        sqlx::query("UPDATE metadata SET writer_version = NULL, writer_commit = NULL WHERE id = 1")
            .execute(&store.pool)
            .await
            .unwrap();

        store.checkpoint().await.unwrap();

        let (wv, wc): (Option<String>, Option<String>) =
            sqlx::query_as("SELECT writer_version, writer_commit FROM metadata WHERE id = 1")
                .fetch_one(&store.pool)
                .await
                .unwrap();
        assert!(
            wv.is_none() && wc.is_none(),
            "checkpoint() must be WAL-only post-split; got wv={wv:?} wc={wc:?}"
        );
    });
}
