use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;
use kanban_domain::{Board, Card, Column, DataStore, DomainError, KanbanError, Prefix, Snapshot};

#[test]
fn test_a_replacing_write_that_drops_a_referenced_namespace_is_rejected() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();

    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let board = Board::new("B", Some("KAN"));
        let column = Column::new(board.id, "Todo", 0);
        let mut card = Card::new(board.id, column.id, "one", 0);
        card.prefix = "KAN".to_string();
        card.card_number = 7;

        let mut seed = Snapshot::new();
        seed.boards = vec![board];
        seed.columns = vec![column];
        seed.cards = vec![card];
        seed.prefixes = vec![Prefix {
            name: "kan".to_string(),
            card_counter: 7,
            sprint_counter: 0,
        }];

        store.apply_snapshot_async(seed.clone()).await.unwrap();

        let mut without_kan = seed.clone();
        without_kan.prefixes.clear();

        let err = store.apply_snapshot_async(without_kan).await.unwrap_err();
        assert!(
            matches!(
                &err,
                KanbanError::Domain(DomainError::PrefixNotBacked {
                    card_number: 7,
                    prefix,
                }) if prefix == "KAN"
            ),
            "expected PrefixNotBacked for card 7 / KAN, got {err:?}"
        );
        drop(store);

        let reopened = SqliteStore::open(&path).await.unwrap();
        let prefix = reopened
            .get_prefix("kan")
            .unwrap()
            .expect("the kan row must survive a rejected apply_snapshot, across a reopen");
        assert_eq!(prefix.card_counter, 7);
        let cards = reopened.list_all_cards().unwrap();
        let card = cards
            .into_iter()
            .find(|c| c.card_number == 7)
            .expect("card 7 must still be present after the reopen");
        assert_eq!(card.prefix, "KAN");
    });
}
