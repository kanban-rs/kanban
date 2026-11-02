//! Schema 13 -> 14: `cards.tags TEXT` added. The column is nullable so
//! pre-migration rows that never had `tags` read back as `tags == []`
//! (see `row_to_card` in `conversions.rs`).

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;

use super::super::{SqliteStore, SUPPORTED_SCHEMA_VERSION};
use super::make_rt;
use kanban_domain::DataStore;

/// Open the current-v14 DB, then `DROP COLUMN tags` and stamp
/// `schema_version = 13` to simulate a pre-migration state. Reopening with
/// `SqliteStore::open` must trigger `migrate_v13_to_v14_tags`, which adds
/// the `tags` column back. The pre-existing card should then read back
/// with `tags == []`.
#[test]
fn test_sqlite_migration_v13_to_v14_adds_tags_empty() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        // Step 1: seed a real DB (board, column, card with a known tag)
        let card_id = {
            let store = SqliteStore::open(&path).await.unwrap();
            let board = kanban_domain::Board::new("B", Some("BBB"));
            let col = kanban_domain::Column::new(board.id, "Col", 0);
            let mut card = kanban_domain::Card::new(board.id, col.id, "Card", 0);
            card.tags = vec!["seed".to_string()];
            let card_id = card.id;
            store.upsert_board(board).unwrap();
            store.upsert_column(col).unwrap();
            store.upsert_card(card).unwrap();
            store.close().await;
            card_id
        };

        // Step 2: drop the tags column and stamp schema_version = 13
        {
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(
                    SqliteConnectOptions::new()
                        .filename(&path)
                        .foreign_keys(false),
                )
                .await
                .unwrap();
            sqlx::query("ALTER TABLE cards DROP COLUMN tags")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("UPDATE metadata SET schema_version = 13 WHERE id = 1")
                .execute(&pool)
                .await
                .unwrap();
            pool.close().await;
        }

        // Step 3: reopening triggers the 13 -> 14 migration
        let store = SqliteStore::open(&path).await.unwrap();

        // Step 4: binary constant must be 14
        assert_eq!(SUPPORTED_SCHEMA_VERSION, 14);

        // Step 5: schema_version in the DB is now 14
        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert_eq!(
            version, SUPPORTED_SCHEMA_VERSION,
            "v13 -> v14 migration must bump the schema marker"
        );

        // Step 6: the pre-migration card reads back with an empty tag list
        let card = store
            .get_card(card_id)
            .unwrap()
            .expect("card should still be present after migration");
        assert_eq!(
            card.tags,
            Vec::<String>::new(),
            "a pre-migration card must read back with tags == []"
        );
    });
}
