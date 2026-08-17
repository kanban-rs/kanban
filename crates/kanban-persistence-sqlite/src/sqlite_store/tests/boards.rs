use kanban_domain::data_store::DataStore;
use kanban_domain::{Board, BoardRecord, SortField, SortOrder};
use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;

/// A fully-populated board with null FK columns (active_sprint_id /
/// completion_column_id) so the round-trip does not trip the boards table's
/// foreign-key constraints.
fn fully_populated_board() -> Board {
    let record = BoardRecord {
        id: Uuid::new_v4(),
        name: "Populated".to_string(),
        description: Some("d".to_string()),
        sprint_prefix: Some("SPR".to_string()),
        card_prefix: Some("KAN".to_string()),
        task_sort_field: SortField::Priority,
        task_sort_order: SortOrder::Descending,
        sprint_duration_days: Some(14),
        sprint_names: vec!["Alpha".to_string(), "Beta".to_string()],
        sprint_name_used_count: 1,
        next_sprint_number: 12,
        active_sprint_id: None,
        task_list_view: kanban_domain::task_list_view::TaskListView::GroupedByColumn,
        position: 5,
        created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
        updated_at: "2024-02-02T00:00:00Z".parse().unwrap(),
    };
    Board::reconstitute(record).unwrap()
}

#[test]
fn test_sqlite_board_round_trip_through_record_is_identity() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = fully_populated_board();
        let id = board.id;
        store.upsert_board(board.clone()).unwrap();

        let loaded = store.get_board(id).unwrap().expect("board should load");
        assert_eq!(loaded, board);
    });
}

#[test]
fn test_sqlite_side_tables_sourced_from_record() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let board = fully_populated_board();
        let id = board.id;
        store.upsert_board(board.clone()).unwrap();

        let loaded = store.get_board(id).unwrap().expect("board should load");
        assert_eq!(loaded.sprint_names, board.sprint_names);
    });
}

#[test]
fn test_sqlite_row_to_board_goes_through_reconstitute() {
    // Insert a raw board row with a blank name (bypassing the write-side
    // required_str guard) and assert the read path goes through reconstitute,
    // which coerces a legacy blank name to "Untitled" rather than rejecting it
    // (rejecting would brick the board). A direct Board literal would keep the
    // blank, so the coercion proves the row routes through reconstitute.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO boards (id, name, description, sprint_prefix, card_prefix,
                task_sort_field, task_sort_order, sprint_duration_days,
                sprint_name_used_count, next_sprint_number, active_sprint_id,
                task_list_view, card_counter, position,
                created_at, updated_at)
             VALUES (?, '   ', NULL, NULL, NULL, 'Default', 'Ascending', NULL,
                0, 1, NULL, 'Flat', 1, 0,
                '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
        )
        .bind(id.to_string())
        .execute(store.pool())
        .await
        .unwrap();

        let loaded = store.get_board(id).unwrap().expect("board should load");
        assert_eq!(loaded.name, "Untitled");
    });
}

#[test]
fn test_list_boards_ties_break_by_created_at_then_id() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();

        let mut newer = fully_populated_board();
        newer.name = "Newer".to_string();
        newer.created_at = "2024-06-01T00:00:00Z".parse().unwrap();
        let mut older = fully_populated_board();
        older.name = "Older".to_string();
        older.created_at = "2024-01-01T00:00:00Z".parse().unwrap();

        // Same position on both boards; insert the "newer" row first so raw
        // insertion order alone would not already produce the correct
        // (created_at-ascending) result.
        store.upsert_board(newer).unwrap();
        store.upsert_board(older).unwrap();

        let loaded = store.list_boards().unwrap();
        assert_eq!(
            loaded.iter().map(|b| b.name.clone()).collect::<Vec<_>>(),
            vec!["Older", "Newer"]
        );
    });
}
