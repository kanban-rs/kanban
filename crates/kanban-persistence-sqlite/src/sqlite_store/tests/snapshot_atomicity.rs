use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;
use kanban_domain::{Board, Card, Column, DataStore};

#[test]
fn test_apply_snapshot_failure_leaves_existing_data_untouched() {
    // Regression guard: apply_snapshot_async wipes every table then re-inserts
    // the incoming snapshot inside a single transaction (snapshot.rs). Its
    // correctness rests entirely on that atomicity -- if the wipe and the
    // rewrite were ever split across transactions (or committed early), a
    // failure partway through the rewrite would leave the store wiped rather
    // than rolled back. `defer_foreign_keys = ON` means an FK violation is
    // exactly the kind of failure that only surfaces at COMMIT, after every
    // individual INSERT has already run -- a real stress case for atomicity,
    // not just an early-return one.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("atomic.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let mut board = Board::new("Existing", None::<String>);
        let column = Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, column.id, "Existing card", 0);
        let existing_card_id = card.id;
        store.upsert_board(board).unwrap();
        store.upsert_column(column).unwrap();
        store.upsert_card(card).unwrap();

        let mut bad_snapshot = store.snapshot().unwrap();
        // A card whose sprint_id points at a sprint that doesn't exist: the
        // per-row INSERT succeeds (FK checking is deferred), but COMMIT fails.
        let mut broken_card = bad_snapshot.cards[0].clone();
        broken_card.sprint_id = Some(Uuid::new_v4());
        bad_snapshot.cards = vec![broken_card];

        let result = store.apply_snapshot(bad_snapshot);
        assert!(
            result.is_err(),
            "a dangling sprint_id must fail at commit, not silently succeed"
        );

        assert!(
            store.get_card(existing_card_id).unwrap().is_some(),
            "a failed apply_snapshot must roll back the wipe too -- the \
             pre-existing card must still be there, not left deleted"
        );
        assert_eq!(
            store.list_boards().unwrap().len(),
            1,
            "the pre-existing board must survive a rolled-back apply_snapshot"
        );
    });
}
