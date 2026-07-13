use kanban_domain::{Board, CardListFilter, KanbanOperations, Snapshot};
use kanban_service::{open_context, AppConfig, KanbanContext};
use tempfile::TempDir;

fn assert_wal_empty(db_path: &std::path::Path) {
    let wal = db_path.with_extension("sqlite3-wal");
    let len = if wal.exists() {
        wal.metadata().unwrap().len()
    } else {
        0
    };
    assert_eq!(len, 0, "WAL should be empty at {}", wal.display());
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_import_board_checkpoints_wal_on_sqlite_path() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let mut ctx = open_context(path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    let snapshot = Snapshot {
        archived_boards: Vec::new(),
        boards: vec![Board::new("Imported", None::<String>)],
        columns: vec![],
        cards: vec![],
        archived_cards: vec![],
        sprints: vec![],
        graph: Default::default(),
    };
    let json = serde_json::to_string(&snapshot).unwrap();
    ctx.import_board(&json).unwrap();
    ctx.save().await.unwrap();
    assert_wal_empty(&path);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_execute_checkpoints_wal_after_save() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let mut ctx = open_context(path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    ctx.create_board("B".to_string(), None).unwrap();
    ctx.save().await.unwrap();
    assert_wal_empty(&path);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_save_checkpoints_wal_on_sqlite_path() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let mut ctx = open_context(path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    ctx.create_board("B".to_string(), None).unwrap();
    ctx.save().await.unwrap();
    assert_wal_empty(&path);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_undo_checkpoints_wal_after_save() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let mut ctx = open_context(path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    ctx.create_board("B".to_string(), None).unwrap();
    ctx.undo().unwrap();
    ctx.save().await.unwrap();
    assert_wal_empty(&path);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_redo_checkpoints_wal_after_save() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite3");
    let mut ctx = open_context(path.to_str().unwrap(), AppConfig::default())
        .await
        .unwrap();
    ctx.create_board("B".to_string(), None).unwrap();
    ctx.undo().unwrap();
    ctx.redo().unwrap();
    ctx.save().await.unwrap();
    assert_wal_empty(&path);
}

async fn open_sqlite_ctx(dir: &TempDir) -> KanbanContext {
    let path = dir.path().join("test.sqlite").to_string_lossy().to_string();
    open_context(&path, AppConfig::default())
        .await
        .expect("open_context must succeed")
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_backend_create_board_and_list() {
    let dir = TempDir::new().unwrap();
    let mut ctx = open_sqlite_ctx(&dir).await;

    let board = ctx.create_board("Test Board".to_string(), None).unwrap();
    assert_eq!(board.name, "Test Board");

    let boards = ctx.list_boards().unwrap();
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "Test Board");
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_backend_full_workflow() {
    let dir = TempDir::new().unwrap();
    let mut ctx = open_sqlite_ctx(&dir).await;

    let board = ctx.create_board("Board".to_string(), None).unwrap();
    let col = ctx
        .create_column(board.id, "TODO".to_string(), None)
        .unwrap();
    let card = ctx
        .create_card(board.id, col.id, "Task 1".to_string(), Default::default())
        .unwrap();

    assert_eq!(ctx.list_boards().unwrap().len(), 1);
    assert_eq!(ctx.list_columns(board.id).unwrap().len(), 1);
    assert_eq!(ctx.list_cards(CardListFilter::default()).unwrap().len(), 1);

    ctx.archive_card(card.id).unwrap();
    assert_eq!(ctx.list_archived_cards().unwrap().len(), 1);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_backend_undo_redo() {
    let dir = TempDir::new().unwrap();
    let mut ctx = open_sqlite_ctx(&dir).await;

    ctx.create_board("Board 1".to_string(), None).unwrap();
    assert_eq!(ctx.list_boards().unwrap().len(), 1);

    assert!(ctx.undo().unwrap());
    assert_eq!(ctx.list_boards().unwrap().len(), 0);

    assert!(ctx.redo().unwrap());
    assert_eq!(ctx.list_boards().unwrap().len(), 1);
}

// multi_thread: sqlx connection pool spawns background tasks that deadlock on single-threaded runtime
#[tokio::test(flavor = "multi_thread")]
async fn test_sqlite_backend_data_persists_across_opens() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.sqlite").to_string_lossy().to_string();

    {
        let mut ctx = open_context(&path, AppConfig::default()).await.unwrap();
        ctx.create_board("Persistent Board".to_string(), None)
            .unwrap();
    }

    let ctx2 = open_context(&path, AppConfig::default()).await.unwrap();
    let boards = ctx2.list_boards().unwrap();
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].name, "Persistent Board");
}
