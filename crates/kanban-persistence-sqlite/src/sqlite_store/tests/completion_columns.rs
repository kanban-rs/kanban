use kanban_domain::data_store::DataStore;
use kanban_domain::{Board, Column};
use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::{completion_rows, make_rt};

fn seed_board_with_columns(store: &SqliteStore) -> (Board, Column, Column) {
    let board = Board::new("B".to_string(), None::<String>);
    let col1 = Column::new(board.id, "One".to_string(), 0);
    let col2 = Column::new(board.id, "Two".to_string(), 1);
    store.upsert_board(board.clone()).unwrap();
    store.upsert_column(col1.clone()).unwrap();
    store.upsert_column(col2.clone()).unwrap();
    (board, col1, col2)
}

#[test]
fn test_sqlite_completion_columns_round_trip_preserves_order() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let (mut board, col1, col2) = seed_board_with_columns(&store);

        // Deliberately NOT column-position order: [col2, col1].
        board.update_completion_column_ids(vec![col2.id, col1.id]);
        store.upsert_board(board.clone()).unwrap();

        let loaded = store.get_board(board.id).unwrap().expect("board loads");
        assert_eq!(
            loaded.completion_column_ids,
            vec![col2.id, col1.id],
            "list order must survive the round trip, not be re-sorted by column position"
        );

        let listed = store.list_boards().unwrap();
        assert_eq!(
            listed[0].completion_column_ids,
            vec![col2.id, col1.id],
            "the list read path must side-load the same ordered ids"
        );
    });
}

#[test]
fn test_sqlite_board_with_empty_completion_columns_round_trips_empty() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let (board, _col1, _col2) = seed_board_with_columns(&store);

        let loaded = store.get_board(board.id).unwrap().expect("board loads");
        assert_eq!(loaded.completion_column_ids, Vec::<Uuid>::new());
    });
}

#[test]
fn test_sqlite_upsert_replaces_completion_columns_not_appends() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let (mut board, col1, col2) = seed_board_with_columns(&store);

        board.update_completion_column_ids(vec![col1.id, col2.id]);
        store.upsert_board(board.clone()).unwrap();
        board.update_completion_column_ids(vec![col2.id]);
        store.upsert_board(board.clone()).unwrap();

        let loaded = store.get_board(board.id).unwrap().expect("board loads");
        assert_eq!(
            loaded.completion_column_ids,
            vec![col2.id],
            "an upsert must replace the stored list, not append to it"
        );
    });
}

#[test]
fn test_sqlite_deleting_column_removes_it_from_completion_set() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let (mut board, col1, col2) = seed_board_with_columns(&store);

        board.update_completion_column_ids(vec![col1.id, col2.id]);
        store.upsert_board(board.clone()).unwrap();

        store.delete_column(col1.id).unwrap();

        let loaded = store.get_board(board.id).unwrap().expect("board loads");
        assert_eq!(
            loaded.completion_column_ids,
            vec![col2.id],
            "deleting a column must remove exactly that entry from the completion set"
        );
    });
}

#[test]
fn test_sqlite_deleting_board_removes_its_completion_rows() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let (mut board, col1, _col2) = seed_board_with_columns(&store);

        board.update_completion_column_ids(vec![col1.id]);
        store.upsert_board(board.clone()).unwrap();

        store.delete_board(board.id).unwrap();

        assert_eq!(
            completion_rows(&store, board.id).await,
            Vec::<String>::new(),
            "deleting a board must cascade away all of its completion rows"
        );
    });
}
