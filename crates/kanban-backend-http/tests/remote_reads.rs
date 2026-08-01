use kanban_api::{CreateBoardRequest, CreateCardRequest, CreateColumnRequest};
use kanban_backend_http::HttpBackend;
use kanban_domain::{Board, Card, Column, DataStore};
use kanban_server::test_helpers::TestServer;
use uuid::Uuid;

async fn seed_board(server: &TestServer, name: &str) -> Uuid {
    let req = CreateBoardRequest {
        id: None,
        name: name.to_string(),
        description: None,
        sprint_prefix: None,
        card_prefix: None,
        task_sort_field: None,
        task_sort_order: None,
        sprint_duration_days: None,
        task_list_view: None,
    };
    let resp = server
        .client()
        .post(format!("{}/v1/boards", server.base_url()))
        .json(&req)
        .send()
        .await
        .unwrap();
    let board: kanban_api::BoardResponse = resp.json().await.unwrap();
    board.id
}

async fn seed_column(server: &TestServer, board_id: Uuid, name: &str) -> Uuid {
    let req = CreateColumnRequest {
        id: None,
        name: name.to_string(),
        wip_limit: None,
    };
    let resp = server
        .client()
        .post(format!(
            "{}/v1/boards/{board_id}/columns",
            server.base_url()
        ))
        .json(&req)
        .send()
        .await
        .unwrap();
    let column: kanban_api::ColumnResponse = resp.json().await.unwrap();
    column.id
}

async fn seed_card(
    server: &TestServer,
    column_id: Uuid,
    title: &str,
    description: Option<&str>,
) -> Uuid {
    let req = CreateCardRequest {
        id: None,
        title: title.to_string(),
        description: description.map(str::to_string),
        priority: None,
        due_date: None,
        points: None,
        sprint_id: None,
    };
    let resp = server
        .client()
        .post(format!(
            "{}/v1/columns/{column_id}/cards",
            server.base_url()
        ))
        .json(&req)
        .send()
        .await
        .unwrap();
    let card: kanban_api::CardResponse = resp.json().await.unwrap();
    card.id
}

/// `HttpBackend`'s DataStore methods bridge onto their own dedicated runtime
/// (`HttpBackend::block_on`) and panic ("Cannot start a runtime from within a
/// runtime") if called directly from a `#[tokio::test]`'s own runtime.
/// `spawn_blocking` moves the sync call onto a non-runtime blocking thread,
/// matching how a real sync caller (kanban-cli, kanban-tui) would invoke it.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(f).await.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_boards_empty_when_no_boards() {
    let server = TestServer::start().await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let boards: Vec<Board> = blocking(move || backend.list_boards().unwrap()).await;
    assert!(boards.is_empty());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_boards_returns_seeded_board() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Remote Board").await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let boards: Vec<Board> = blocking(move || backend.list_boards().unwrap()).await;
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].id, board_id);
    assert_eq!(boards[0].name, "Remote Board");

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_returns_seeded_board_by_id() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Solo Board").await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let board: Option<Board> = blocking(move || backend.get_board(board_id).unwrap()).await;
    let board = board.expect("board should be found");
    assert_eq!(board.id, board_id);
    assert_eq!(board.name, "Solo Board");

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_returns_none_for_unknown_id() {
    let server = TestServer::start().await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let board: Option<Board> = blocking(move || backend.get_board(Uuid::new_v4()).unwrap()).await;
    assert!(board.is_none());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_columns_by_board_returns_columns_in_order() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Columns Board").await;
    seed_column(&server, board_id, "Todo").await;
    seed_column(&server, board_id, "Done").await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let columns: Vec<Column> =
        blocking(move || backend.list_columns_by_board(board_id).unwrap()).await;
    let names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["Todo", "Done"]);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_column_returns_seeded_column_by_id() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Column Board").await;
    let column_id = seed_column(&server, board_id, "Todo").await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let column: Option<Column> = blocking(move || backend.get_column(column_id).unwrap()).await;
    let column = column.expect("column should be found");
    assert_eq!(column.id, column_id);
    assert_eq!(column.board_id, board_id);
    assert_eq!(column.name, "Todo");

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_by_column_returns_full_card_with_description() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Cards Board").await;
    let column_id = seed_column(&server, board_id, "Todo").await;
    seed_card(&server, column_id, "Card A", Some("has a description")).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let cards: Vec<Card> = blocking(move || backend.list_cards_by_column(column_id).unwrap()).await;
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].title, "Card A");
    assert_eq!(cards[0].board_id, board_id);
    assert_eq!(cards[0].description, Some("has a description".to_string()));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_by_column_returns_empty_for_unknown_column() {
    let server = TestServer::start().await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let cards: Vec<Card> =
        blocking(move || backend.list_cards_by_column(Uuid::new_v4()).unwrap()).await;
    assert!(cards.is_empty());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_by_column_does_not_leak_other_columns_cards() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Multi Column Board").await;
    let todo_id = seed_column(&server, board_id, "Todo").await;
    let done_id = seed_column(&server, board_id, "Done").await;
    seed_card(&server, todo_id, "Todo Card", None).await;
    seed_card(&server, done_id, "Done Card", None).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let todo_cards: Vec<Card> =
        blocking(move || backend.list_cards_by_column(todo_id).unwrap()).await;
    assert_eq!(todo_cards.len(), 1);
    assert_eq!(todo_cards[0].title, "Todo Card");

    server.shutdown().await;
}
