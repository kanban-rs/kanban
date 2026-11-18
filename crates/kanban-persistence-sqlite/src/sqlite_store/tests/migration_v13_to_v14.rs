//! Schema 13 -> 14: `cards.labels TEXT` added. The column is nullable so
//! pre-migration rows that never had `labels` read back as `labels == []`
//! (see `row_to_card` in `conversions.rs`).

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;

use super::super::{SqliteStore, SUPPORTED_SCHEMA_VERSION};
use super::make_rt;
use kanban_domain::DataStore;

/// Open the current-v14 DB, then `DROP COLUMN labels` and stamp
/// `schema_version = 13` to simulate a pre-migration state. Reopening with
/// `SqliteStore::open` must trigger `migrate_v13_to_v14_tags`, which adds
/// the `labels` column back. The pre-existing card should then read back
/// with `labels == []`.
#[test]
fn test_sqlite_migration_v13_to_v14_adds_tags_empty() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        // Step 1: seed a real DB (board, column, card with a known label)
        let card_id = {
            let store = SqliteStore::open(&path).await.unwrap();
            let board = kanban_domain::Board::new("B", Some("BBB"));
            let col = kanban_domain::Column::new(board.id, "Col", 0);
            let mut card = kanban_domain::Card::new(board.id, col.id, "Card", 0);
            card.labels = vec!["seed".to_string()];
            let card_id = card.id;
            store.upsert_board(board).unwrap();
            store.upsert_column(col).unwrap();
            store.upsert_card(card).unwrap();
            store.close().await;
            card_id
        };

        // Step 2: drop the labels column and stamp schema_version = 13
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
            sqlx::query("ALTER TABLE cards DROP COLUMN labels")
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

        // Step 4: binary constant must be 15 (labels migration added v14 -> v15)
        assert_eq!(SUPPORTED_SCHEMA_VERSION, 15);

        // Step 5: schema_version in the DB is now 14
        let version: u32 = sqlx::query_scalar("SELECT schema_version FROM metadata WHERE id = 1")
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert_eq!(
            version, SUPPORTED_SCHEMA_VERSION,
            "v13 -> v14 migration must bump the schema marker"
        );

        // Step 6: the pre-migration card reads back with an empty label list
        let card = store
            .get_card(card_id)
            .unwrap()
            .expect("card should still be present after migration");
        assert_eq!(
            card.labels,
            Vec::<String>::new(),
            "a pre-migration card must read back with labels == []"
        );
    });
}
