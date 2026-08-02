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
        let board_id = board.id;
        let column = kanban_domain::Column::new(board.id, "Col", 0);
        let card = kanban_domain::Card::new(&mut board, column.id, "Task", 0);
        let card_id = card.id;
        store.upsert_board(board).unwrap();
        store.upsert_column(column).unwrap();
        store.upsert_card(card.clone()).unwrap();

        // Mark the (live-upserted) card as archived. The card row stays live
        // behind the marker; delete_archived_card must clean up both.
        let archived = kanban_domain::ArchivedCard::new(card_id, board_id);
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
        let board_id = board.id;
        let column = kanban_domain::Column::new(board.id, "Col", 0);
        let card = kanban_domain::Card::new(&mut board, column.id, "Task", 0);
        let card_id = card.id;
        store.upsert_board(board).unwrap();
        store.upsert_column(column).unwrap();
        store.upsert_card(card.clone()).unwrap();

        let archived = kanban_domain::ArchivedCard::new(card_id, board_id);
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

/// A completed-lifecycle sprint on `board_id` built through the factory, with
/// every nullable column populated. The `sprints` table carries an FK to
/// `boards`, so callers must persist a matching board first.
fn fully_populated_sprint(board_id: uuid::Uuid) -> kanban_domain::Sprint {
    use kanban_domain::{SprintRecord, SprintStatus};
    use uuid::Uuid;

    let record = SprintRecord {
        id: Uuid::new_v4(),
        board_id,
        sprint_number: 9,
        name_index: Some(4),
        prefix: Some("SPR".to_string()),
        card_prefix: Some("KAN".to_string()),
        status: SprintStatus::Completed,
        start_date: Some("2024-02-01T00:00:00Z".parse().unwrap()),
        end_date: Some("2024-02-14T00:00:00Z".parse().unwrap()),
        created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
        updated_at: "2024-02-15T00:00:00Z".parse().unwrap(),
    };
    kanban_domain::Sprint::reconstitute(record).unwrap()
}

#[test]
fn test_sprint_write_then_read_round_trips_all_fields() {
    use kanban_domain::data_store::DataStore;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("sprint_roundtrip.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = kanban_domain::Board::new("B", None::<String>);
        let board_id = board.id;
        store.upsert_board(board).unwrap();

        let original = fully_populated_sprint(board_id);
        let id = original.id;
        store.upsert_sprint(original.clone()).unwrap();

        let loaded = store.get_sprint(id).unwrap().expect("sprint must load");
        assert_eq!(loaded, original);
    });
}

#[test]
fn test_sprint_reconstitute_restores_completed_lifecycle_from_row() {
    use kanban_domain::data_store::DataStore;
    use kanban_domain::SprintStatus;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("sprint_lifecycle.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = kanban_domain::Board::new("B", None::<String>);
        let board_id = board.id;
        store.upsert_board(board).unwrap();

        let original = fully_populated_sprint(board_id);
        let id = original.id;
        store.upsert_sprint(original.clone()).unwrap();

        let loaded = store.get_sprint(id).unwrap().expect("sprint must load");
        assert_eq!(loaded.status, SprintStatus::Completed);
        assert_eq!(loaded.start_date, original.start_date);
        assert_eq!(loaded.end_date, original.end_date);
    });
}

#[test]
fn test_list_sprints_by_board_round_trips_through_record() {
    use kanban_domain::data_store::DataStore;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("sprint_list.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board_a = kanban_domain::Board::new("A", None::<String>);
        let board_b = kanban_domain::Board::new("B", None::<String>);
        let board_a_id = board_a.id;
        let board_b_id = board_b.id;
        store.upsert_board(board_a).unwrap();
        store.upsert_board(board_b).unwrap();

        let s1 = fully_populated_sprint(board_a_id);
        let mut s2 = fully_populated_sprint(board_a_id);
        s2.sprint_number = 10;
        let other = fully_populated_sprint(board_b_id);
        store.upsert_sprint(s1.clone()).unwrap();
        store.upsert_sprint(s2.clone()).unwrap();
        store.upsert_sprint(other).unwrap();

        let loaded = store.list_sprints_by_board(board_a_id).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0], s1);
        assert_eq!(loaded[1], s2);
    });
}

#[test]
fn test_sqlite_row_to_sprint_goes_through_reconstitute() {
    use kanban_domain::data_store::DataStore;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("sprint_funnel.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = kanban_domain::Board::new("B", None::<String>);
        let board_id = board.id;
        store.upsert_board(board).unwrap();

        let sprint = fully_populated_sprint(board_id);
        let id = sprint.id;
        store.upsert_sprint(sprint).unwrap();

        // Corrupt the stored status to an unparseable value directly, bypassing
        // write-side encoding, so the read funnel is the only thing left to
        // catch it via p_enum inside row_to_sprint -> SprintRecord.
        sqlx::query("UPDATE sprints SET status = 'NotAStatus' WHERE id = ?")
            .bind(id.to_string())
            .execute(&store.pool)
            .await
            .unwrap();

        let result = store.get_sprint(id);
        assert!(
            result.is_err(),
            "row_to_sprint must surface a decode error for an unparseable status"
        );
    });
}

#[test]
fn test_sprint_json_and_sqlite_produce_equal_entity() {
    use kanban_domain::data_store::DataStore;
    use kanban_domain::Snapshot;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("sprint_cross.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = kanban_domain::Board::new("B", None::<String>);
        let board_id = board.id;
        store.upsert_board(board).unwrap();

        let original = fully_populated_sprint(board_id);
        let id = original.id;

        // SQLite round-trip.
        store.upsert_sprint(original.clone()).unwrap();
        let from_sqlite = store.get_sprint(id).unwrap().expect("sprint must load");

        // JSON round-trip through the same SprintRecord shim.
        let snapshot = Snapshot::from_data(
            vec![],
            vec![],
            vec![],
            vec![],
            vec![original.clone()],
            kanban_domain::DependencyGraph::new(),
        );
        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: Snapshot = serde_json::from_str(&json).unwrap();
        let from_json = restored.sprints[0].clone();

        assert_eq!(from_sqlite, from_json);
        assert_eq!(from_sqlite, original);
    });
}
