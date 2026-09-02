use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;
use kanban_domain::{ArchivedCard, Board, Card, Column, DataStore, Prefix};

#[test]
fn test_apply_snapshot_never_deletes_a_prefix_row_while_a_card_names_it() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("prefix_order.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        store
            .upsert_prefix(Prefix {
                name: "kan".to_string(),
                card_counter: 3,
                sprint_counter: 0,
            })
            .unwrap();

        let board = Board::new("Existing", Some("KAN"));
        let column = Column::new(board.id, "Todo", 0);
        let mut card = Card::new(board.id, column.id, "Existing card", 0);
        card.prefix = "KAN".to_string();
        card.card_number = 3;
        let mut second = Card::new(board.id, column.id, "Second card", 1);
        second.prefix = "KAN".to_string();
        second.card_number = 2;

        store.upsert_board(board.clone()).unwrap();
        store.upsert_column(column).unwrap();
        store.upsert_card(card).unwrap();
        store.upsert_card(second.clone()).unwrap();
        store
            .insert_archived_card(ArchivedCard::new(second.id, board.id))
            .unwrap();

        let snap = store.snapshot_async().await.unwrap();

        sqlx::query(
            "CREATE TRIGGER prefix_delete_restrict_probe \
             BEFORE DELETE ON prefixes FOR EACH ROW \
             WHEN EXISTS (SELECT 1 FROM cards WHERE cards.prefix = OLD.name COLLATE NOCASE) \
             BEGIN SELECT RAISE(ABORT, 'prefix still referenced by a card'); END;",
        )
        .execute(store.pool())
        .await
        .unwrap();

        sqlx::query(
            "CREATE TRIGGER card_prefix_restrict_probe \
             BEFORE INSERT ON cards FOR EACH ROW \
             WHEN NEW.prefix <> '' AND NOT EXISTS (SELECT 1 FROM prefixes WHERE name = NEW.prefix COLLATE NOCASE) \
             BEGIN SELECT RAISE(ABORT, 'card names an unbacked prefix'); END;",
        )
        .execute(store.pool())
        .await
        .unwrap();

        let result = store.apply_snapshot_async(snap).await;
        assert!(
            result.is_ok(),
            "apply_snapshot must not delete a prefix row while a card still \
             names it, nor insert a card before its prefix row exists: {result:?}"
        );

        let unbacked: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM cards c WHERE c.prefix <> '' AND NOT EXISTS \
             (SELECT 1 FROM prefixes p WHERE p.name = c.prefix COLLATE NOCASE)",
        )
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(unbacked, 0);

        let card_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM cards")
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(card_count, 2);
    });
}
