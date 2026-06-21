use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;

#[test]
fn test_delete_archived_card_orphaned_cards_row_is_still_cleaned_up() {
    use kanban_domain::data_store::DataStore;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let mut board = kanban_domain::Board::new("B", None::<String>);
        let column = kanban_domain::Column::new(board.id, "Col", 0);
        let card = kanban_domain::Card::new(&mut board, column.id, "Task", 0);
        let card_id = card.id;
        let column_id = column.id;
        store.upsert_board(board).unwrap();
        store.upsert_column(column).unwrap();
        store.upsert_card(card.clone()).unwrap();

        // Insert into archived_cards WITHOUT calling delete_card first,
        // leaving an orphaned row in the cards table.
        let archived = kanban_domain::ArchivedCard::new(card, column_id, 0);
        store.insert_archived_card(archived).unwrap();

        store.delete_archived_card(card_id).unwrap();

        assert!(
            store.list_archived_cards().unwrap().is_empty(),
            "card should be gone from archived_cards"
        );
        assert!(
            store.list_all_cards().unwrap().is_empty(),
            "orphaned cards row should also be removed by delete_archived_card"
        );
    });
}

#[test]
fn test_delete_archived_card_removes_from_cards_table() {
    use kanban_domain::data_store::DataStore;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let mut board = kanban_domain::Board::new("B", None::<String>);
        let column = kanban_domain::Column::new(board.id, "Col", 0);
        let card = kanban_domain::Card::new(&mut board, column.id, "Task", 0);
        let card_id = card.id;
        let column_id = column.id;
        store.upsert_board(board).unwrap();
        store.upsert_column(column).unwrap();
        store.upsert_card(card.clone()).unwrap();

        let archived = kanban_domain::ArchivedCard::new(card, column_id, 0);
        store.insert_archived_card(archived).unwrap();
        store.delete_card(card_id).unwrap();

        assert_eq!(store.list_archived_cards().unwrap().len(), 1);

        store.delete_archived_card(card_id).unwrap();

        assert!(
            store.list_archived_cards().unwrap().is_empty(),
            "card should be gone from archived_cards"
        );
        assert!(
            store.list_all_cards().unwrap().is_empty(),
            "card should also be gone from cards table, not restored as active"
        );
        assert!(
            store.get_card(card_id).unwrap().is_none(),
            "get_card should return None for permanently deleted card"
        );
    });
}

#[test]
fn test_empty_sprint_log_status_returns_error() {
    use kanban_domain::data_store::DataStore;
    use kanban_domain::{Board, Card, Column, SprintLog};

    let dir = TempDir::new().unwrap();
    let path = dir.path().join("validation.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let mut board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Col", 0);
        let mut card = Card::new(&mut board, column.id, "Task", 0);
        store.upsert_board(board).unwrap();
        store.upsert_column(column).unwrap();

        let log = SprintLog::new(uuid::Uuid::new_v4(), 1, None::<String>, "");
        card.sprint_logs.push(log);

        let result = store.upsert_card(card);
        assert!(
            result.is_err(),
            "upsert_card must reject a SprintLog with empty status"
        );
    });
}

#[test]
fn test_empty_board_name_returns_error() {
    use kanban_domain::data_store::DataStore;
    use kanban_domain::Board;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("validation.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("", None::<String>);
        let result = store.upsert_board(board);
        assert!(
            result.is_err(),
            "upsert_board must reject a Board with empty name"
        );
    });
}

#[test]
fn test_empty_column_name_returns_error() {
    use kanban_domain::data_store::DataStore;
    use kanban_domain::{Board, Column};
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("validation.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("B", None::<String>);
        let board_id = board.id;
        store.upsert_board(board).unwrap();
        let col = Column::new(board_id, "", 0);
        let result = store.upsert_column(col);
        assert!(
            result.is_err(),
            "upsert_column must reject a Column with empty name"
        );
    });
}

#[test]
fn test_empty_card_title_returns_error() {
    use kanban_domain::data_store::DataStore;
    use kanban_domain::{Board, Card, Column};
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("validation.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let mut board = Board::new("B", None::<String>);
        let col = Column::new(board.id, "Col", 0);
        let col_id = col.id;
        // Card::new borrows &mut board -- call it before upsert_board moves board
        let card = Card::new(&mut board, col_id, "", 0);
        store.upsert_board(board).unwrap();
        store.upsert_column(col).unwrap();
        let result = store.upsert_card(card);
        assert!(
            result.is_err(),
            "upsert_card must reject a Card with empty title"
        );
    });
}
