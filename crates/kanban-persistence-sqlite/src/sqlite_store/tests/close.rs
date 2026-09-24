use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;

#[test]
fn test_close_shuts_the_pool_without_the_persistence_store_trait_in_scope() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("close.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        store.close().await;
        assert!(store.pool.is_closed());
    });
}
