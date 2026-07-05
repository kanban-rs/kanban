use kanban_domain::data_store::DataStore;
use kanban_domain::*;
use kanban_persistence_sqlite::SqliteStore;
use tempfile::TempDir;
use uuid::Uuid;

async fn make_store() -> (SqliteStore, TempDir) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.db");
    let store = SqliteStore::open(&path).await.unwrap();
    (store, dir)
}

fn make_board(name: &str) -> Board {
    Board::new(name.to_string(), None::<String>)
}

fn make_column(board_id: Uuid, name: &str, pos: i32) -> Column {
    Column::new(board_id, name.to_string(), pos)
}

fn make_card(board: &mut Board, column_id: Uuid, title: &str, pos: i32) -> Card {
    Card::new(board, column_id, title.to_string(), pos)
}

// --- Board CRUD ---

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_upsert_and_get_board() {
    let (store, _dir) = make_store().await;
    let mut board = make_board("Test Board");
    board.sprint_names = vec!["Alpha".to_string(), "Beta".to_string()];
    board.sprint_counters.insert("SP".to_string(), 5);
    let id = board.id;
    store.upsert_board(board).unwrap();

    let fetched = store.get_board(id).unwrap().unwrap();
    assert_eq!(fetched.name, "Test Board");
    assert_eq!(fetched.sprint_names, vec!["Alpha", "Beta"]);
    assert_eq!(fetched.sprint_counters.get("SP"), Some(&5));
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_list_boards_empty() {
    let (store, _dir) = make_store().await;
    assert!(store.list_boards().unwrap().is_empty());
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_delete_board_removes_it() {
    let (store, _dir) = make_store().await;
    let board = make_board("To Delete");
    let id = board.id;
    store.upsert_board(board).unwrap();
    store.delete_board(id).unwrap();
    assert!(store.get_board(id).unwrap().is_none());
}

// --- Column CRUD ---

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_upsert_and_get_column() {
    let (store, _dir) = make_store().await;
    let board = make_board("B");
    store.upsert_board(board.clone()).unwrap();
    let col = make_column(board.id, "Col", 0);
    let col_id = col.id;
    store.upsert_column(col).unwrap();

    let fetched = store.get_column(col_id).unwrap().unwrap();
    assert_eq!(fetched.name, "Col");
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_list_columns_by_board_filters_correctly() {
    let (store, _dir) = make_store().await;
    let board1 = make_board("B1");
    let board2 = make_board("B2");
    store.upsert_board(board1.clone()).unwrap();
    store.upsert_board(board2.clone()).unwrap();

    store
        .upsert_column(make_column(board1.id, "C1", 0))
        .unwrap();
    store
        .upsert_column(make_column(board1.id, "C2", 1))
        .unwrap();
    store
        .upsert_column(make_column(board2.id, "C3", 0))
        .unwrap();

    let cols = store.list_columns_by_board(board1.id).unwrap();
    assert_eq!(cols.len(), 2);
    assert!(cols.iter().all(|c| c.board_id == board1.id));
}

// --- Card CRUD ---

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_upsert_and_get_card() {
    let (store, _dir) = make_store().await;
    let mut board = make_board("B");
    let col = make_column(board.id, "Col", 0);
    store.upsert_board(board.clone()).unwrap();
    store.upsert_column(col.clone()).unwrap();

    let card = make_card(&mut board, col.id, "Card", 0);
    let card_id = card.id;
    store.upsert_card(card).unwrap();

    let fetched = store.get_card(card_id).unwrap().unwrap();
    assert_eq!(fetched.title, "Card");
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_list_cards_by_column() {
    let (store, _dir) = make_store().await;
    let mut board = make_board("B");
    let col1 = make_column(board.id, "C1", 0);
    let col2 = make_column(board.id, "C2", 1);
    store.upsert_board(board.clone()).unwrap();
    store.upsert_column(col1.clone()).unwrap();
    store.upsert_column(col2.clone()).unwrap();

    store
        .upsert_card(make_card(&mut board, col1.id, "Card1", 0))
        .unwrap();
    store
        .upsert_card(make_card(&mut board, col1.id, "Card2", 1))
        .unwrap();
    store
        .upsert_card(make_card(&mut board, col2.id, "Card3", 0))
        .unwrap();

    let cards = store.list_cards_by_column(col1.id).unwrap();
    assert_eq!(cards.len(), 2);
    assert!(cards.iter().all(|c| c.column_id == col1.id));
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_list_cards_by_sprint() {
    let (store, _dir) = make_store().await;
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let sprint = Sprint::new(board.id, 1, None, None::<String>);
    store.upsert_board(board.clone()).unwrap();
    store.upsert_column(col.clone()).unwrap();
    store.upsert_sprint(sprint.clone()).unwrap();

    let mut card1 = make_card(&mut board, col.id, "Card1", 0);
    card1.sprint_id = Some(sprint.id);
    let card2 = make_card(&mut board, col.id, "Card2", 1);
    store.upsert_card(card1).unwrap();
    store.upsert_card(card2).unwrap();

    let cards = store.list_cards_by_sprint(sprint.id).unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].sprint_id, Some(sprint.id));
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_count_cards_in_column() {
    let (store, _dir) = make_store().await;
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    store.upsert_board(board.clone()).unwrap();
    store.upsert_column(col.clone()).unwrap();

    store
        .upsert_card(make_card(&mut board, col.id, "C1", 0))
        .unwrap();
    store
        .upsert_card(make_card(&mut board, col.id, "C2", 1))
        .unwrap();

    assert_eq!(store.count_cards_in_column(col.id).unwrap(), 2);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_count_cards_in_column_excluding() {
    let (store, _dir) = make_store().await;
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    store.upsert_board(board.clone()).unwrap();
    store.upsert_column(col.clone()).unwrap();

    let card1 = make_card(&mut board, col.id, "C1", 0);
    let card1_id = card1.id;
    store.upsert_card(card1).unwrap();
    store
        .upsert_card(make_card(&mut board, col.id, "C2", 1))
        .unwrap();

    let count = store
        .count_cards_in_column_excluding(col.id, &[card1_id])
        .unwrap();
    assert_eq!(count, 1);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_clear_sprint_from_cards() {
    let (store, _dir) = make_store().await;
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let sprint = Sprint::new(board.id, 1, None, None::<String>);
    store.upsert_board(board.clone()).unwrap();
    store.upsert_column(col.clone()).unwrap();
    store.upsert_sprint(sprint.clone()).unwrap();

    let mut card1 = make_card(&mut board, col.id, "C1", 0);
    card1.sprint_id = Some(sprint.id);
    let card1_id = card1.id;
    store.upsert_card(card1).unwrap();

    store
        .clear_sprint_from_cards(sprint.id, chrono::Utc::now())
        .unwrap();
    assert!(store
        .get_card(card1_id)
        .unwrap()
        .unwrap()
        .sprint_id
        .is_none());
}

// --- Sprint CRUD ---

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_upsert_and_get_sprint() {
    let (store, _dir) = make_store().await;
    let board = make_board("B");
    store.upsert_board(board.clone()).unwrap();
    let sprint = Sprint::new(board.id, 1, None, None::<String>);
    let sprint_id = sprint.id;
    store.upsert_sprint(sprint).unwrap();

    let fetched = store.get_sprint(sprint_id).unwrap().unwrap();
    assert_eq!(fetched.sprint_number, 1);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_list_sprints_by_board() {
    let (store, _dir) = make_store().await;
    let board1 = make_board("B1");
    let board2 = make_board("B2");
    store.upsert_board(board1.clone()).unwrap();
    store.upsert_board(board2.clone()).unwrap();

    store
        .upsert_sprint(Sprint::new(board1.id, 1, None, None::<String>))
        .unwrap();
    store
        .upsert_sprint(Sprint::new(board1.id, 2, None, None::<String>))
        .unwrap();
    store
        .upsert_sprint(Sprint::new(board2.id, 1, None, None::<String>))
        .unwrap();

    let sprints = store.list_sprints_by_board(board1.id).unwrap();
    assert_eq!(sprints.len(), 2);
}

// --- Archived card ---

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_insert_and_get_archived_card() {
    let (store, _dir) = make_store().await;
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    store.upsert_board(board.clone()).unwrap();
    store.upsert_column(col.clone()).unwrap();

    let card = make_card(&mut board, col.id, "Card", 0);
    let card_id = card.id;
    let ac = ArchivedCard::new(card, uuid::Uuid::nil(), col.id, 0);
    store.insert_archived_card(ac).unwrap();

    let fetched = store.get_archived_card(card_id).unwrap().unwrap();
    assert_eq!(fetched.card.id, card_id);
    assert_eq!(fetched.original_column_id, col.id);

    // Archived card should NOT appear in active card queries
    assert!(store.get_card(card_id).unwrap().is_none());
    assert!(store.list_all_cards().unwrap().is_empty());
}

// --- Graph ---

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_set_and_get_graph() {
    let (store, _dir) = make_store().await;
    let graph = DependencyGraph::new();
    store.set_graph(graph.clone()).unwrap();
    let fetched = store.get_graph().unwrap();
    assert_eq!(fetched, graph);
}

// --- Snapshot ---

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_snapshot_roundtrip() {
    let (store, _dir) = make_store().await;
    let mut board = make_board("B");
    let col = make_column(board.id, "C", 0);
    let sprint = Sprint::new(board.id, 1, None, None::<String>);
    store.upsert_board(board.clone()).unwrap();
    store.upsert_column(col.clone()).unwrap();
    store.upsert_sprint(sprint.clone()).unwrap();
    let card = make_card(&mut board, col.id, "Card", 0);
    store.upsert_card(card).unwrap();

    let snap = store.snapshot().unwrap();
    assert_eq!(snap.boards.len(), 1);
    assert_eq!(snap.columns.len(), 1);
    assert_eq!(snap.cards.len(), 1);
    assert_eq!(snap.sprints.len(), 1);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_apply_snapshot_replaces_existing_data() {
    let (store, _dir) = make_store().await;
    let board_old = make_board("Old");
    store.upsert_board(board_old).unwrap();

    let board_new = make_board("New");
    let snap = Snapshot::from_data(
        vec![board_new],
        vec![],
        vec![],
        vec![],
        vec![],
        DependencyGraph::new(),
    );
    store.apply_snapshot(snap).unwrap();

    let boards = store.list_boards().unwrap();
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "New");
}

// --- KAN-191 command_log migration ---

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_open_recreates_legacy_command_log_with_new_schema() {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("legacy.sqlite");

    // Seed the file with the pre-405 legacy command_log schema (idx/cmd_json
    // columns) and the legacy undo_state table.
    {
        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::raw_sql(
            "CREATE TABLE command_log (idx INTEGER PRIMARY KEY, cmd_json TEXT NOT NULL); \
             INSERT INTO command_log (cmd_json) VALUES ('[]'); \
             CREATE TABLE undo_state (id INTEGER PRIMARY KEY, cursor INTEGER NOT NULL); \
             INSERT INTO undo_state (id, cursor) VALUES (1, 0);",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;
    }

    let store = SqliteStore::open(&db_path).await.unwrap();

    let has_command_log: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='command_log'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    let has_undo_state: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='undo_state'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    let has_new_col: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('command_log') WHERE name = 'batch_index'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();

    assert!(
        has_command_log,
        "KAN-191 brings the command_log table back — must exist after open"
    );
    assert!(
        has_new_col,
        "legacy schema (idx/cmd_json) must be replaced by the KAN-191 schema (batch_index/...)"
    );
    assert!(
        !has_undo_state,
        "undo_state (the cursor table) is still dropped — cursor stays in-session"
    );

    // Confirm the rest of the schema is intact and the DB is usable.
    let board = make_board("Survived migrate");
    let id = board.id;
    store.upsert_board(board).unwrap();
    assert_eq!(
        store.get_board(id).unwrap().unwrap().name,
        "Survived migrate"
    );
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_open_creates_command_log_on_fresh_database() {
    let (store, _dir) = make_store().await;
    let has_command_log: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='command_log'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(
        has_command_log,
        "fresh database must contain a command_log table for the audit log"
    );
}

#[tokio::test(flavor = "current_thread")]
#[should_panic(expected = "SqliteStore requires a multi-threaded Tokio runtime")]
async fn test_sqlite_on_current_thread_runtime_gives_clear_error() {
    let (store, _dir) = make_store().await;
    let board = make_board("CurrentThread");
    store.upsert_board(board).unwrap();
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_concurrent_reads_and_writes_no_panic() {
    use std::sync::Arc;

    let (store, _dir) = make_store().await;
    let store = Arc::new(store);
    let mut handles = vec![];

    for i in 0..10 {
        let s = Arc::clone(&store);
        handles.push(tokio::spawn(async move {
            let board = Board::new(format!("Board-{i}"), None::<String>);
            s.upsert_board(board.clone()).unwrap();
            let col = Column::new(board.id, format!("Col-{i}"), i);
            s.upsert_column(col).unwrap();
        }));
    }

    for _ in 0..10 {
        let s = Arc::clone(&store);
        handles.push(tokio::spawn(async move {
            for _ in 0..10 {
                let _ = s.list_boards();
                let _ = s.list_all_columns();
                let _ = s.list_all_cards();
                let _ = s.snapshot();
            }
        }));
    }

    for h in handles {
        h.await.expect("task should not panic");
    }

    let boards = store.list_boards().unwrap();
    assert_eq!(boards.len(), 10);
}
