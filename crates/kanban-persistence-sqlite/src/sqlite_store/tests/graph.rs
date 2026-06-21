use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;

/// Per-kind tables hard-reject metadata outside their respective
/// CHECK constraints. Pin the constraint via a direct insert
/// attempt so any future schema relaxation has to choose
/// whether to drop or update this test.
#[test]
fn test_blocks_edges_rejects_unknown_severity() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("check.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let insert = sqlx::query(
            "INSERT INTO blocks_edges
                (source_id, target_id, severity, created_at, archived_at)
             VALUES (?, ?, 'Catastrophic', ?, NULL)",
        )
        .bind(uuid::Uuid::nil().to_string())
        .bind(uuid::Uuid::from_u128(0x42).to_string())
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(store.pool())
        .await;
        assert!(
            insert.is_err(),
            "CHECK on severity must reject 'Catastrophic'; got {:?}",
            insert
        );
    });
}

#[test]
fn test_relates_edges_rejects_unknown_kind() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("check_relates.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let insert = sqlx::query(
            "INSERT INTO relates_edges
                (source_id, target_id, kind, created_at, archived_at)
             VALUES (?, ?, 'Unknown', ?, NULL)",
        )
        .bind(uuid::Uuid::nil().to_string())
        .bind(uuid::Uuid::from_u128(0x42).to_string())
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(store.pool())
        .await;
        assert!(insert.is_err());
    });
}
