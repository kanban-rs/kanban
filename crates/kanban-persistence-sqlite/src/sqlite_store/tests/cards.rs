use kanban_domain::data_store::DataStore;
use kanban_domain::{
    Board, Card, CardPriority, CardRecord, CardStatus, Column, ColumnRecord, SprintLog,
};
use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;

/// A fully-populated card whose `column_id` points at a real column and whose
/// `sprint_id` is `None`, so the round-trip satisfies the cards table's
/// foreign-key constraints (column_id -> columns, sprint_id -> sprints).
fn fully_populated_card(column_id: Uuid) -> Card {
    let sprint_id = Uuid::new_v4();
    let record = CardRecord {
        id: Uuid::new_v4(),
        column_id,
        title: "Done card".to_string(),
        description: Some("finished".to_string()),
        priority: CardPriority::High,
        status: CardStatus::Done,
        position: 7,
        due_date: Some("2024-05-05T00:00:00Z".parse().unwrap()),
        points: Some(3),
        card_number: 42,
        sprint_id: None,
        created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
        updated_at: "2024-02-02T00:00:00Z".parse().unwrap(),
        completed_at: Some("2024-03-03T00:00:00Z".parse().unwrap()),
        sprint_logs: vec![
            SprintLog {
                sprint_id,
                sprint_number: 1,
                sprint_name: Some("Sprint 1".to_string()),
                started_at: "2024-01-10T00:00:00Z".parse().unwrap(),
                ended_at: Some("2024-01-20T00:00:00Z".parse().unwrap()),
                status: "Completed".to_string(),
            },
            SprintLog {
                sprint_id,
                sprint_number: 2,
                sprint_name: None,
                started_at: "2024-02-01T00:00:00Z".parse().unwrap(),
                ended_at: None,
                status: "Active".to_string(),
            },
        ],
    };
    Card::reconstitute(record).unwrap()
}

fn seed_column(store: &SqliteStore) -> Uuid {
    let board = Board::new("B", None::<String>);
    let board_id = board.id;
    store.upsert_board(board).unwrap();

    let column = Column::reconstitute(ColumnRecord {
        id: Uuid::new_v4(),
        board_id,
        name: "In Progress".to_string(),
        position: 0,
        wip_limit: None,
        created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
        updated_at: "2024-01-01T00:00:00Z".parse().unwrap(),
    })
    .unwrap();
    let column_id = column.id;
    store.upsert_column(column).unwrap();
    column_id
}

#[test]
fn test_list_cards_by_column_ties_break_by_id_when_position_and_created_at_equal() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let column_id = seed_column(&store);

        let mut a = fully_populated_card(column_id);
        a.title = "A".to_string();
        let mut b = fully_populated_card(column_id);
        b.title = "B".to_string();

        // Insert in one order, then re-derive the same two cards and insert
        // in the opposite order into a fresh store; the result must be
        // identical regardless of insertion order.
        store.upsert_card(a.clone()).unwrap();
        store.upsert_card(b.clone()).unwrap();
        let first_order = store
            .list_cards_by_column(column_id)
            .unwrap()
            .iter()
            .map(|c| c.title.clone())
            .collect::<Vec<_>>();

        let path2 = dir.path().join("test2.sqlite3");
        let store2 = SqliteStore::open(&path2).await.unwrap();
        let column_id2 = seed_column(&store2);
        a.column_id = column_id2;
        b.column_id = column_id2;
        store2.upsert_card(b).unwrap();
        store2.upsert_card(a).unwrap();
        let second_order = store2
            .list_cards_by_column(column_id2)
            .unwrap()
            .iter()
            .map(|c| c.title.clone())
            .collect::<Vec<_>>();

        assert_eq!(first_order, second_order);
    });
}

#[test]
fn test_sqlite_card_round_trip_preserves_all_fields() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let column_id = seed_column(&store);

        let card = fully_populated_card(column_id);
        let id = card.id;
        store.upsert_card(card.clone()).unwrap();

        let loaded = store.get_card(id).unwrap().expect("card should load");
        assert_eq!(loaded, card);
    });
}

#[test]
fn test_sqlite_card_round_trip_restores_sprint_logs_from_separate_table() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let column_id = seed_column(&store);

        let card = fully_populated_card(column_id);
        let id = card.id;
        store.upsert_card(card.clone()).unwrap();

        let loaded = store.get_card(id).unwrap().expect("card should load");
        // The two logs are reassembled from the separate sprint_logs table,
        // ordered, and funnelled through reconstitute verbatim.
        assert_eq!(loaded.sprint_logs.len(), 2);
        assert_eq!(loaded.sprint_logs, card.sprint_logs);
        assert_eq!(loaded.sprint_logs[0].sprint_number, 1);
        assert_eq!(loaded.sprint_logs[0].ended_at, card.sprint_logs[0].ended_at);
        assert_eq!(loaded.sprint_logs[1].sprint_name, None);
        assert_eq!(loaded.sprint_logs[1].ended_at, None);
    });
}

#[test]
fn test_sqlite_card_reconstitute_rejects_malformed_row() {
    // A raw card row with an unparseable status enum surfaces a KanbanError
    // through reconstitute's read path, not a panic.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let column_id = seed_column(&store);
        let id = Uuid::new_v4();

        sqlx::query(
            "INSERT INTO cards (id, column_id, title, description, priority, status, position,
                due_date, points, card_number, sprint_id, created_at, updated_at, completed_at)
             VALUES (?, ?, 'Bad', NULL, 'Medium', 'NotAStatus', 0, NULL, NULL, 1, NULL,
                '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z', NULL)",
        )
        .bind(id.to_string())
        .bind(column_id.to_string())
        .execute(store.pool())
        .await
        .unwrap();

        let result = store.get_card(id);
        assert!(result.is_err());
    });
}
