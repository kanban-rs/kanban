use kanban_domain::data_store::DataStore;
use kanban_domain::{Board, CardStatus, Column, ColumnRecord, NewColumn};
use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;

/// A column whose `board_id` points at `board_id`, so the round-trip satisfies
/// the columns table's foreign-key constraint (board_id -> boards).
fn record_for(board_id: Uuid, wip_limit: Option<i32>) -> ColumnRecord {
    ColumnRecord {
        id: Uuid::new_v4(),
        board_id,
        name: "In Progress".to_string(),
        position: 3,
        wip_limit,
        default_status: None,
        created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
        updated_at: "2024-02-02T00:00:00Z".parse().unwrap(),
    }
}

#[test]
fn test_sqlite_column_round_trip_preserves_all_fields() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("B", None::<String>);
        let board_id = board.id;
        store.upsert_board(board).unwrap();

        let column = Column::reconstitute(record_for(board_id, Some(7))).unwrap();
        let id = column.id;
        store.upsert_column(column.clone()).unwrap();

        let loaded = store.get_column(id).unwrap().expect("column should load");
        assert_eq!(loaded, column);
    });
}

#[test]
fn test_sqlite_column_round_trip_none_wip_limit() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("B", None::<String>);
        let board_id = board.id;
        store.upsert_board(board).unwrap();

        let column = Column::reconstitute(record_for(board_id, None)).unwrap();
        let id = column.id;
        store.upsert_column(column.clone()).unwrap();

        let loaded = store.get_column(id).unwrap().expect("column should load");
        assert_eq!(loaded.wip_limit, None);
        assert_eq!(loaded, column);
    });
}

#[test]
fn test_row_to_column_funnels_through_reconstitute() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("B", None::<String>);
        let board_id = board.id;
        store.upsert_board(board).unwrap();

        let record = record_for(board_id, Some(2));
        let expected = Column::reconstitute(record.clone()).unwrap();
        store.upsert_column(expected.clone()).unwrap();

        let loaded = store.get_column(record.id).unwrap().expect("column loads");
        // The funnel is observable: the loaded column equals the column produced
        // by reconstitute(record), field-for-field including position.
        assert_eq!(loaded, Column::reconstitute(record).unwrap());
        assert_eq!(loaded.position, expected.position);
    });
}

#[test]
fn test_column_create_store_load_equal_sqlite() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("B", None::<String>);
        let board_id = board.id;
        store.upsert_board(board).unwrap();

        let now = "2024-03-03T00:00:00Z".parse().unwrap();
        let column = Column::create(
            NewColumn {
                board_id,
                name: "Done".to_string(),
                wip_limit: Some(5),
                default_status: None,
            },
            Uuid::new_v4(),
            1,
            now,
        )
        .unwrap();
        let id = column.id;
        store.upsert_column(column.clone()).unwrap();

        let loaded = store.get_column(id).unwrap().expect("column should load");
        assert_eq!(loaded, column);
    });
}

#[test]
fn test_column_default_status_round_trips_through_sqlite() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("B", None::<String>);
        let board_id = board.id;
        store.upsert_board(board).unwrap();

        let mut record = record_for(board_id, Some(7));
        record.default_status = Some(CardStatus::InProgress);
        let column = Column::reconstitute(record).unwrap();
        let id = column.id;
        store.upsert_column(column.clone()).unwrap();
        drop(store);

        let reopened = SqliteStore::open(&path).await.unwrap();
        let loaded = reopened
            .get_column(id)
            .unwrap()
            .expect("column should load");
        assert_eq!(loaded.default_status, Some(CardStatus::InProgress));
        assert_eq!(loaded, column);
    });
}

#[test]
fn test_column_null_default_status_round_trips_as_none() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("B", None::<String>);
        let board_id = board.id;
        store.upsert_board(board).unwrap();

        let column = Column::reconstitute(record_for(board_id, Some(7))).unwrap();
        let id = column.id;
        store.upsert_column(column.clone()).unwrap();
        drop(store);

        let reopened = SqliteStore::open(&path).await.unwrap();
        let loaded = reopened
            .get_column(id)
            .unwrap()
            .expect("column should load");
        assert_eq!(loaded.default_status, None);
        assert_eq!(loaded, column);
    });
}

#[test]
fn test_column_default_status_is_stored_as_its_serde_wire_name_not_debug_output() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("B", None::<String>);
        let board_id = board.id;
        store.upsert_board(board).unwrap();

        let mut record = record_for(board_id, None);
        record.default_status = Some(CardStatus::Done);
        let column = Column::reconstitute(record).unwrap();
        let id = column.id;
        store.upsert_column(column).unwrap();

        let raw: String = sqlx::query_scalar("SELECT default_status FROM columns WHERE id = ?")
            .bind(id.to_string())
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(
            raw, "Done",
            "the raw stored text must be the serde wire name, never Debug output"
        );
    });
}

#[test]
fn test_list_columns_by_board_ties_break_by_created_at_then_id() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = Board::new("B", None::<String>);
        let board_id = board.id;
        store.upsert_board(board).unwrap();

        let mut newer = record_for(board_id, None);
        newer.name = "Newer".to_string();
        newer.created_at = "2024-06-01T00:00:00Z".parse().unwrap();
        let mut older = record_for(board_id, None);
        older.name = "Older".to_string();
        older.created_at = "2024-01-01T00:00:00Z".parse().unwrap();

        // Same position on both boards; insert the "newer" row first so raw
        // insertion order alone would not already produce the correct
        // (created_at-ascending) result.
        store
            .upsert_column(Column::reconstitute(newer).unwrap())
            .unwrap();
        store
            .upsert_column(Column::reconstitute(older).unwrap())
            .unwrap();

        let loaded = store.list_columns_by_board(board_id).unwrap();
        assert_eq!(
            loaded.iter().map(|c| c.name.clone()).collect::<Vec<_>>(),
            vec!["Older", "Newer"]
        );
    });
}
