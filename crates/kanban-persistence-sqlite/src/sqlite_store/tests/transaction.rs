use tempfile::TempDir;

use super::super::SqliteStore;
use super::make_rt;
use kanban_domain::{ArchivedCard, Board, Card, Column, DataStore, KanbanResult, Sprint};

fn open(path: &std::path::Path) -> SqliteStore {
    let rt = make_rt();
    rt.block_on(async { SqliteStore::open(path).await.unwrap() })
}

#[test]
fn test_db_conn_without_ambient_tx_commits_multi_statement_write() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let mut board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, column.id, "Card", 0);
        let card_id = card.id;

        let card2 = card.clone();
        store
            .db_conn(|conn| {
                Box::pin(async move { SqliteStore::write_card_with_conn(conn, &card2).await })
            })
            .await
            .unwrap();

        // write_card_with_conn alone doesn't satisfy FKs (board/column not
        // written), so read it back through a raw query instead of get_card.
        let row: (String,) = sqlx::query_as("SELECT id FROM cards WHERE id = ?")
            .bind(card_id.to_string())
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert_eq!(row.0, card_id.to_string());
    });
}

#[test]
fn test_db_conn_without_ambient_tx_rolls_back_failing_write() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let mut board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, column.id, "Card", 0);
        let card_id = card.id;

        let card2 = card.clone();
        let result: KanbanResult<()> = store
            .db_conn(|conn| {
                Box::pin(async move {
                    SqliteStore::write_card_with_conn(conn, &card2).await?;
                    Err(kanban_domain::KanbanError::Internal("boom".into()))
                })
            })
            .await;
        assert!(result.is_err());

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cards WHERE id = ?")
            .bind(card_id.to_string())
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert_eq!(
            count.0, 0,
            "failing db_conn closure must roll back its write"
        );
    });
}

#[test]
fn test_begin_ambient_transaction_writes_invisible_to_a_fresh_connection_until_commit() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");

    // Open the second store FIRST: SqliteStore::open() re-runs idempotent
    // schema/migration writes on every open, which contend for SQLite's
    // single writer lock against a held ambient transaction. No
    // busy_timeout is configured, so opening second fails with
    // SQLITE_BUSY if this store is opened after the ambient tx starts.
    let store1 = open(&path);
    let store2 = open(&path);

    let rt = make_rt();
    rt.block_on(async {
        let mut board = Board::new("B", None::<String>);
        board.id = uuid::Uuid::new_v4();
        let board2 = board.clone();

        store1.begin_ambient_transaction().await.unwrap();
        store1
            .db_conn(|conn| {
                Box::pin(async move { SqliteStore::write_board_with_conn(conn, &board2).await })
            })
            .await
            .unwrap();

        assert!(
            store2.get_board(board.id).unwrap().is_none(),
            "an uncommitted ambient write must be invisible to a fresh connection"
        );

        store1.finish_ambient_transaction(true).await.unwrap();

        assert!(
            store2.get_board(board.id).unwrap().is_some(),
            "the board must be visible once the ambient transaction commits"
        );
    });
}

#[test]
fn test_read_after_write_within_ambient_transaction_sees_the_write() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let mut board = Board::new("B", None::<String>);
        let column = Column::new(board.id, "Todo", 0);
        store.upsert_board(board.clone()).unwrap();
        store.upsert_column(column.clone()).unwrap();
        let card = Card::new(&mut board, column.id, "Card", 0);
        let card_id = card.id;

        store.begin_ambient_transaction().await.unwrap();
        let card2 = card.clone();
        store
            .db_conn(|conn| {
                Box::pin(async move { SqliteStore::write_card_with_conn(conn, &card2).await })
            })
            .await
            .unwrap();

        let read = store.get_card(card_id).unwrap();
        store.finish_ambient_transaction(true).await.unwrap();

        assert!(
            read.is_some(),
            "a read within the same ambient transaction must see the prior write"
        );
    });
}

#[test]
fn test_read_after_write_within_ambient_transaction_covers_every_routed_read() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let mut board = Board::new("B", None::<String>);
        let board_id = board.id;
        let column = Column::new(board.id, "Todo", 0);
        let column_id = column.id;
        let card = Card::new(&mut board, column.id, "Card", 0);
        let card_id = card.id;
        // A second, non-archived card so the live-scoped list/count reads
        // below have something to observe (`card_id` is archived further
        // down, which excludes it from live-scoped queries).
        let live_card = Card::new(&mut board, column.id, "Live card", 1);
        let sprint = Sprint::new(board.id, 1, None, Some("SP"));
        let sprint_id = sprint.id;
        let archived_card = ArchivedCard::new(card_id, board_id);
        let other_card_id = uuid::Uuid::new_v4();

        store.begin_ambient_transaction().await.unwrap();

        let b2 = board.clone();
        store
            .db_conn(|conn| {
                Box::pin(async move { SqliteStore::write_board_with_conn(conn, &b2).await })
            })
            .await
            .unwrap();
        let c2 = column.clone();
        store
            .db_conn(|conn| {
                Box::pin(async move { SqliteStore::write_column_with_conn(conn, &c2).await })
            })
            .await
            .unwrap();
        let card2 = card.clone();
        store
            .db_conn(|conn| {
                Box::pin(async move { SqliteStore::write_card_with_conn(conn, &card2).await })
            })
            .await
            .unwrap();
        let live_card2 = live_card.clone();
        store
            .db_conn(|conn| {
                Box::pin(async move { SqliteStore::write_card_with_conn(conn, &live_card2).await })
            })
            .await
            .unwrap();
        let s2 = sprint.clone();
        store
            .db_conn(|conn| {
                Box::pin(async move { SqliteStore::write_sprint_with_conn(conn, &s2).await })
            })
            .await
            .unwrap();
        let ac2 = archived_card;
        store
            .db_conn(|conn| {
                Box::pin(
                    async move { SqliteStore::write_archived_card_with_conn(conn, &ac2).await },
                )
            })
            .await
            .unwrap();

        store
            .modify_graph_async(Box::new(move |graph| {
                graph.set_block(other_card_id, card_id)
            }))
            .await
            .unwrap();

        assert!(store.get_board(board_id).unwrap().is_some(), "get_board");
        assert!(store.get_column(column_id).unwrap().is_some(), "get_column");
        assert_eq!(
            store.list_columns_by_board(board_id).unwrap().len(),
            1,
            "list_columns_by_board"
        );
        assert!(store.get_card(card_id).unwrap().is_some(), "get_card");
        assert_eq!(
            store.list_cards_by_column(column_id).unwrap().len(),
            1,
            "list_cards_by_column"
        );
        assert_eq!(
            store.count_cards_in_column(column_id).unwrap(),
            1,
            "count_cards_in_column"
        );
        assert!(store.get_sprint(sprint_id).unwrap().is_some(), "get_sprint");
        assert_eq!(
            store.list_sprints_by_board(board_id).unwrap().len(),
            1,
            "list_sprints_by_board"
        );
        assert!(
            store.get_archived_card(card_id).unwrap().is_some(),
            "get_archived_card"
        );
        assert_eq!(
            store.list_archived_cards_by_board(board_id).unwrap().len(),
            1,
            "list_archived_cards_by_board"
        );
        assert_eq!(store.list_boards().unwrap().len(), 1, "list_boards");
        assert_eq!(
            store.list_all_columns().unwrap().len(),
            1,
            "list_all_columns"
        );
        assert_eq!(
            store.list_all_sprints().unwrap().len(),
            1,
            "list_all_sprints"
        );
        assert_eq!(
            store.list_archived_cards().unwrap().len(),
            1,
            "list_archived_cards"
        );
        assert!(store.get_graph().is_ok(), "get_graph");

        store.finish_ambient_transaction(true).await.unwrap();
    });
}

#[test]
fn test_finish_ambient_transaction_false_rolls_back_full_graph() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let mut board = Board::new("B", None::<String>);
        let board_id = board.id;
        let column = Column::new(board.id, "Todo", 0);
        let column_id = column.id;
        let card = Card::new(&mut board, column.id, "Card", 0);
        let card_id = card.id;
        let sprint = Sprint::new(board.id, 1, None, Some("SP"));
        let sprint_id = sprint.id;
        let archived_card = ArchivedCard::new(card_id, board_id);

        store.begin_ambient_transaction().await.unwrap();

        let b2 = board.clone();
        store
            .db_conn(|conn| {
                Box::pin(async move { SqliteStore::write_board_with_conn(conn, &b2).await })
            })
            .await
            .unwrap();
        let c2 = column.clone();
        store
            .db_conn(|conn| {
                Box::pin(async move { SqliteStore::write_column_with_conn(conn, &c2).await })
            })
            .await
            .unwrap();
        let card2 = card.clone();
        store
            .db_conn(|conn| {
                Box::pin(async move { SqliteStore::write_card_with_conn(conn, &card2).await })
            })
            .await
            .unwrap();
        let s2 = sprint.clone();
        store
            .db_conn(|conn| {
                Box::pin(async move { SqliteStore::write_sprint_with_conn(conn, &s2).await })
            })
            .await
            .unwrap();
        let ac2 = archived_card;
        store
            .db_conn(|conn| {
                Box::pin(
                    async move { SqliteStore::write_archived_card_with_conn(conn, &ac2).await },
                )
            })
            .await
            .unwrap();

        store.finish_ambient_transaction(false).await.unwrap();

        let fresh = SqliteStore::open(&path).await.unwrap();
        assert!(
            fresh.get_board(board_id).unwrap().is_none(),
            "board rolled back"
        );
        assert!(
            fresh.get_column(column_id).unwrap().is_none(),
            "column rolled back"
        );
        assert!(
            fresh.get_card(card_id).unwrap().is_none(),
            "card rolled back"
        );
        assert!(
            fresh.get_sprint(sprint_id).unwrap().is_none(),
            "sprint rolled back"
        );
        assert!(
            fresh.get_archived_card(card_id).unwrap().is_none(),
            "archived card marker rolled back"
        );
    });
}

#[test]
fn test_begin_ambient_transaction_twice_returns_err_and_preserves_first_transaction() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("t.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let mut board = Board::new("B", None::<String>);
        board.id = uuid::Uuid::new_v4();
        let board2 = board.clone();

        store.begin_ambient_transaction().await.unwrap();
        store
            .db_conn(|conn| {
                Box::pin(async move { SqliteStore::write_board_with_conn(conn, &board2).await })
            })
            .await
            .unwrap();

        let second = store.begin_ambient_transaction().await;
        assert!(
            second.is_err(),
            "a nested begin_ambient_transaction call must return an error, not panic"
        );

        store.finish_ambient_transaction(true).await.unwrap();

        assert!(
            store.get_board(board.id).unwrap().is_some(),
            "the first transaction's write must still be intact and committable after the nested begin was rejected"
        );
    });
}
