use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;
use kanban_domain::{Board, Card, Column, DataStore};

#[test]
fn test_apply_snapshot_failure_leaves_existing_data_untouched() {
    // apply_snapshot_async wipes every table then re-inserts the incoming
    // snapshot inside one transaction; a dangling sprint_id fails only at
    // COMMIT (FK checking is deferred), after every individual INSERT in the
    // rewrite has already run against the wiped tables.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("atomic.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let mut board = Board::new("Existing", None::<String>);
        let board_id = board.id;
        let column = Column::new(board.id, "Todo", 0);
        let existing_column_id = column.id;
        let card = Card::new(&mut board, column.id, "Existing card", 0);
        let existing_card_id = card.id;
        store.upsert_board(board).unwrap();
        store.upsert_column(column).unwrap();
        store.upsert_card(card).unwrap();

        let mut bad_snapshot = store.snapshot().unwrap();
        let mut broken_card = bad_snapshot.cards[0].clone();
        broken_card.sprint_id = Some(Uuid::new_v4());
        bad_snapshot.cards = vec![broken_card];

        let result = store.apply_snapshot(bad_snapshot);
        assert!(
            result.is_err(),
            "a dangling sprint_id must fail at commit, not silently succeed"
        );

        assert!(
            store.get_board(board_id).unwrap().is_some(),
            "a failed apply_snapshot must roll back the wipe too -- the \
             pre-existing board must still be there, not left deleted"
        );
        assert!(
            store.get_column(existing_column_id).unwrap().is_some(),
            "the pre-existing column must survive a rolled-back apply_snapshot"
        );
        assert!(
            store.get_card(existing_card_id).unwrap().is_some(),
            "the pre-existing card must survive a rolled-back apply_snapshot"
        );
    });
}
