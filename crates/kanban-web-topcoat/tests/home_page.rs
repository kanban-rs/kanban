use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_service::{
    AppConfig, Board, Card, CardPriority, Column, KanbanBackend, KanbanContext, NewBoard, NewCard,
    NewColumn,
};
use kanban_web_topcoat::context::{router, SharedCtx};
use std::sync::Arc;
use tempfile::tempdir;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex};

fn make_backend(path: &std::path::Path) -> Arc<dyn KanbanBackend> {
    Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))))
}

fn empty_ctx(path: &std::path::Path) -> KanbanContext {
    KanbanContext::open_deferred(make_backend(path), AppConfig::default())
}

fn board_spec(name: &str) -> NewBoard {
    NewBoard {
        name: name.to_string(),
        description: None,
        sprint_prefix: None,
        card_prefix: None,
        task_sort_field: None,
        task_sort_order: None,
        sprint_duration_days: None,
        task_list_view: None,
        completion_column_id: None,
    }
}

fn seed_board(ctx: &mut KanbanContext, name: &str) -> Board {
    ctx.create_board_from_spec(None, board_spec(name)).unwrap()
}

fn seed_column(ctx: &mut KanbanContext, board: &Board, name: &str) -> Column {
    ctx.create_column_from_spec(
        None,
        NewColumn {
            board_id: board.id,
            name: name.to_string(),
            wip_limit: None,
        },
    )
    .unwrap()
}

fn seed_card(ctx: &mut KanbanContext, column: &Column, title: &str) -> Card {
    ctx.create_card_from_spec(
        None,
        NewCard {
            column_id: column.id,
            title: title.to_string(),
            description: None,
            priority: CardPriority::Medium,
            due_date: None,
            points: None,
            sprint_id: None,
        },
    )
    .unwrap()
}

/// Spawns the topcoat router over `ctx` on an ephemeral port and returns the
/// base URL plus a shutdown handle, mirroring `kanban-server`'s
/// `TestServer::start` (`crates/kanban-server/src/test_helpers.rs`).
async fn spawn(ctx: KanbanContext) -> (String, oneshot::Sender<()>, tokio::task::JoinHandle<()>) {
    let shared: SharedCtx = Arc::new(Mutex::new(ctx));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let handle = tokio::spawn(async move {
        topcoat::serve_until(listener, router(shared), async {
            let _ = shutdown_rx.await;
        })
        .await
        .unwrap();
    });
    (format!("http://{addr}"), shutdown_tx, handle)
}

async fn shutdown(shutdown_tx: oneshot::Sender<()>, handle: tokio::task::JoinHandle<()>) {
    let _ = shutdown_tx.send(());
    let _ = handle.await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_home_page_returns_ok_status() {
    let dir = tempdir().unwrap();
    let ctx = empty_ctx(&dir.path().join("empty.json"));
    let (base_url, shutdown_tx, handle) = spawn(ctx).await;

    let response = reqwest::get(&base_url).await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    shutdown(shutdown_tx, handle).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_home_page_with_no_boards_renders_empty_state() {
    let dir = tempdir().unwrap();
    let ctx = empty_ctx(&dir.path().join("empty.json"));
    let (base_url, shutdown_tx, handle) = spawn(ctx).await;

    let body = reqwest::get(&base_url).await.unwrap().text().await.unwrap();
    assert!(body.contains("Boards"));
    assert!(!body.contains("<section"));

    shutdown(shutdown_tx, handle).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_home_page_lists_seeded_board_name() {
    let dir = tempdir().unwrap();
    let mut ctx = empty_ctx(&dir.path().join("board.json"));
    seed_board(&mut ctx, "Spike Board");
    let (base_url, shutdown_tx, handle) = spawn(ctx).await;

    let body = reqwest::get(&base_url).await.unwrap().text().await.unwrap();
    assert!(body.contains("Spike Board"));

    shutdown(shutdown_tx, handle).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_home_page_lists_seeded_column_names_in_order() {
    let dir = tempdir().unwrap();
    let mut ctx = empty_ctx(&dir.path().join("columns.json"));
    let board = seed_board(&mut ctx, "Columns Board");
    seed_column(&mut ctx, &board, "Todo");
    seed_column(&mut ctx, &board, "Done");
    let (base_url, shutdown_tx, handle) = spawn(ctx).await;

    let body = reqwest::get(&base_url).await.unwrap().text().await.unwrap();
    let todo_pos = body.find("Todo").expect("Todo column name in body");
    let done_pos = body.find("Done").expect("Done column name in body");
    assert!(todo_pos < done_pos, "expected Todo before Done in body");

    shutdown(shutdown_tx, handle).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_home_page_lists_seeded_card_titles_under_their_column() {
    let dir = tempdir().unwrap();
    let mut ctx = empty_ctx(&dir.path().join("cards.json"));
    let board = seed_board(&mut ctx, "Cards Board");
    let column = seed_column(&mut ctx, &board, "Todo");
    seed_card(&mut ctx, &column, "Card A");
    seed_card(&mut ctx, &column, "Card B");
    let (base_url, shutdown_tx, handle) = spawn(ctx).await;

    let body = reqwest::get(&base_url).await.unwrap().text().await.unwrap();
    assert_eq!(body.matches("Card A").count(), 1);
    assert_eq!(body.matches("Card B").count(), 1);

    shutdown(shutdown_tx, handle).await;
}
