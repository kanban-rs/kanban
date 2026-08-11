use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;

#[test]
fn test_append_load_shift_and_truncate_command_log_round_trip() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        store.append_command_batch(0, "[\"a\"]").await.unwrap();
        store.append_command_batch(1, "[\"b\"]").await.unwrap();
        store.append_command_batch(2, "[\"c\"]").await.unwrap();

        assert_eq!(
            store.load_all_command_batches().await.unwrap(),
            vec![
                "[\"a\"]".to_string(),
                "[\"b\"]".to_string(),
                "[\"c\"]".to_string()
            ]
        );

        store.shift_command_log(1).await.unwrap();
        assert_eq!(
            store.load_all_command_batches().await.unwrap(),
            vec!["[\"b\"]".to_string(), "[\"c\"]".to_string()]
        );

        store.truncate_command_log_after(1).await.unwrap();
        assert_eq!(
            store.load_all_command_batches().await.unwrap(),
            vec!["[\"b\"]".to_string()]
        );
    });
}
