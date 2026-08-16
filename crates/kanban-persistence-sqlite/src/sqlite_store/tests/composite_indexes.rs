use kanban_domain::data_store::DataStore;
use kanban_domain::{Board, Card, Column, ColumnRecord, Sprint};
use sqlx::{Pool, Row, Sqlite};
use tempfile::TempDir;
use uuid::Uuid;

use super::super::SqliteStore;
use super::make_rt;

async fn explain_query_plan(
    pool: &Pool<Sqlite>,
    sql: &str,
    bind_id: &str,
    bind_number: i64,
) -> String {
    let full_sql = format!("EXPLAIN QUERY PLAN {sql}");
    let rows = sqlx::query(&full_sql)
        .bind(bind_id)
        .bind(bind_number)
        .fetch_all(pool)
        .await
        .unwrap();
    rows.iter()
        .map(|row| row.get::<String, _>("detail"))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn seed_column(store: &SqliteStore, board_id: Uuid) -> Uuid {
    let column = Column::reconstitute(ColumnRecord {
        id: Uuid::new_v4(),
        board_id,
        name: "In Progress".to_string(),
        position: 0,
        wip_limit: None,
        default_status: None,
        created_at: "2024-01-01T00:00:00Z".parse().unwrap(),
        updated_at: "2024-01-01T00:00:00Z".parse().unwrap(),
    })
    .unwrap();
    let column_id = column.id;
    store.upsert_column(column).unwrap();
    column_id
}

async fn seed_cards_across_two_boards(store: &SqliteStore) -> (Uuid, Uuid) {
    let mut board_a = Board::new("A", None::<String>);
    let board_a_id = board_a.id;
    store.upsert_board(board_a.clone()).unwrap();
    let column_a = seed_column(store, board_a_id);

    let mut board_b = Board::new("B", None::<String>);
    let board_b_id = board_b.id;
    store.upsert_board(board_b.clone()).unwrap();
    let column_b = seed_column(store, board_b_id);

    for _ in 0..3 {
        let card = Card::new(&mut board_a, column_a, "card a", 0);
        store.upsert_card(card).unwrap();
    }
    for _ in 0..3 {
        let card = Card::new(&mut board_b, column_b, "card b", 0);
        store.upsert_card(card).unwrap();
    }

    (board_a_id, board_b_id)
}

async fn seed_cards_across_two_sprints(store: &SqliteStore) -> (Uuid, Uuid) {
    let mut board = Board::new("S", None::<String>);
    let board_id = board.id;
    store.upsert_board(board.clone()).unwrap();
    let column_id = seed_column(store, board_id);

    let sprint_a = Sprint::new(board_id, 1, None, None::<String>);
    let sprint_a_id = sprint_a.id;
    store.upsert_sprint(sprint_a).unwrap();

    let sprint_b = Sprint::new(board_id, 2, None, None::<String>);
    let sprint_b_id = sprint_b.id;
    store.upsert_sprint(sprint_b).unwrap();

    for sprint_id in [sprint_a_id, sprint_b_id] {
        for _ in 0..3 {
            let mut card = Card::new(&mut board, column_id, "card", 0);
            card.sprint_id = Some(sprint_id);
            store.upsert_card(card).unwrap();
        }
    }

    (sprint_a_id, sprint_b_id)
}

#[test]
fn test_board_number_lookup_uses_composite_index() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let (board_a_id, _board_b_id) = seed_cards_across_two_boards(&store).await;

        let plan = explain_query_plan(
            store.pool(),
            "SELECT * FROM cards WHERE board_id = ?1 AND card_number = ?2",
            &board_a_id.to_string(),
            1,
        )
        .await;

        assert!(
            plan.contains("USING INDEX idx_cards_board_number"),
            "expected the composite index in the plan, got: {plan}"
        );
    });
}

#[test]
fn test_sprint_number_lookup_uses_composite_index() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let (sprint_a_id, _sprint_b_id) = seed_cards_across_two_sprints(&store).await;

        let plan = explain_query_plan(
            store.pool(),
            "SELECT * FROM cards WHERE sprint_id = ?1 AND card_number = ?2",
            &sprint_a_id.to_string(),
            1,
        )
        .await;

        assert!(
            plan.contains("USING INDEX idx_cards_sprint_number"),
            "expected the composite index in the plan, got: {plan}"
        );
    });
}

#[test]
fn test_existing_database_gets_new_indexes_on_open() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        {
            let store = SqliteStore::open(&path).await.unwrap();
            sqlx::raw_sql("DROP INDEX idx_cards_board_number; DROP INDEX idx_cards_sprint_number;")
                .execute(store.pool())
                .await
                .unwrap();
        }

        let reopened = SqliteStore::open(&path).await.unwrap();
        let index_names: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'index' AND name LIKE 'idx_cards_%_number'",
        )
        .fetch_all(reopened.pool())
        .await
        .unwrap();

        assert!(index_names.contains(&"idx_cards_board_number".to_string()));
        assert!(index_names.contains(&"idx_cards_sprint_number".to_string()));
    });
}

#[test]
fn test_identifier_resolution_returns_identical_results_with_and_without_index() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let rt = make_rt();
    rt.block_on(async {
        let store = SqliteStore::open(&path).await.unwrap();
        let (board_a_id, _board_b_id) = seed_cards_across_two_boards(&store).await;

        let query = "SELECT id FROM cards WHERE board_id = ?1 AND card_number = ?2 ORDER BY id";

        let with_index: Vec<String> = sqlx::query_scalar(query)
            .bind(board_a_id.to_string())
            .bind(2_i64)
            .fetch_all(store.pool())
            .await
            .unwrap();

        sqlx::raw_sql("DROP INDEX idx_cards_board_number")
            .execute(store.pool())
            .await
            .unwrap();

        let without_index: Vec<String> = sqlx::query_scalar(query)
            .bind(board_a_id.to_string())
            .bind(2_i64)
            .fetch_all(store.pool())
            .await
            .unwrap();

        assert_eq!(with_index, without_index);
        assert!(!with_index.is_empty());
    });
}
