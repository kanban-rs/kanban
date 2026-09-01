use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;

#[test]
fn test_sqlite_store_instance_id_is_stable_across_calls() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let first = store.instance_id();
        let second = store.instance_id();
        assert_eq!(first, second);
        assert_ne!(first, Uuid::nil());
    });
}
