use kanban_api::{
    AddBlockRequest, AddRelatedRequest, AttachChildrenRequest, CreateBoardRequest,
    CreateCardRequest, CreateColumnRequest, CreateSprintRequest, RelatesKindDto, SeverityDto,
    SortFieldDto, SortOrderDto, TaskListViewDto,
};
use kanban_backend_http::HttpBackend;
use kanban_domain::{
    Board, Card, Column, DataStore, DependencyGraph, EntityIds, Invalidation, KanbanOperations,
    LoadState, Model, NoProjections, Prefix, RelatesKind, Severity, Sprint,
};
use kanban_server::test_helpers::TestServer;
use kanban_service::{
    requestable, AppConfig, FetchPlan, FetchRound, KanbanBackend, KanbanContext, LoadedEntities,
};
use std::sync::Arc;
use uuid::Uuid;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(f).await.unwrap()
}

async fn seed_board(server: &TestServer, name: &str) -> Uuid {
    let req = CreateBoardRequest {
        id: None,
        name: name.to_string(),
        description: None,
        sprint_prefix: None,
        card_prefix: None,
        task_sort_field: Some(SortFieldDto::Default),
        task_sort_order: Some(SortOrderDto::Ascending),
        sprint_duration_days: None,
        task_list_view: Some(TaskListViewDto::Flat),
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

async fn seed_column(
    server: &TestServer,
    board_id: Uuid,
    name: &str,
    wip_limit: Option<i32>,
    default_status: Option<kanban_api::CardStatusDto>,
) -> Uuid {
    let req = CreateColumnRequest {
        id: None,
        name: name.to_string(),
        wip_limit,
        default_status,
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
    sprint_id: Option<Uuid>,
) -> Uuid {
    let req = CreateCardRequest {
        id: None,
        title: title.to_string(),
        description: None,
        priority: None,
        due_date: None,
        points: None,
        sprint_id,
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

async fn seed_sprint(server: &TestServer, board_id: Uuid, name: &str) -> Uuid {
    let req = CreateSprintRequest {
        id: None,
        name: Some(name.to_string()),
        prefix: None,
        card_prefix: None,
    };
    let resp = server
        .client()
        .post(format!(
            "{}/v1/boards/{board_id}/sprints",
            server.base_url()
        ))
        .json(&req)
        .send()
        .await
        .unwrap();
    let sprint: kanban_api::SprintResponse = resp.json().await.unwrap();
    sprint.id
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_datastore_call_from_a_blocking_thread_inside_an_ambient_runtime_returns_data() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Bridge Board").await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let boards: Vec<Board> = blocking(move || backend.list_boards().unwrap()).await;

    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].id, board_id);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_a_datastore_call_directly_on_a_runtime_worker_thread_returns_data() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Direct Board").await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let boards = backend.list_boards().unwrap();

    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].id, board_id);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_boards_returns_every_seeded_board() {
    let server = TestServer::start().await;
    let a = seed_board(&server, "Board A").await;
    let b = seed_board(&server, "Board B").await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let boards: Vec<Board> = blocking(move || backend.list_boards().unwrap()).await;

    let ids: Vec<Uuid> = boards.iter().map(|b| b.id).collect();
    assert!(ids.contains(&a));
    assert!(ids.contains(&b));
    let names: Vec<&str> = boards.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"Board A"));
    assert!(names.contains(&"Board B"));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_boards_follows_pagination_past_the_first_page() {
    let server = TestServer::start().await;
    for i in 0..51 {
        seed_board(&server, &format!("Board {i}")).await;
    }
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let boards: Vec<Board> = blocking(move || backend.list_boards().unwrap()).await;

    let unique: std::collections::HashSet<Uuid> = boards.iter().map(|b| b.id).collect();
    assert_eq!(
        unique.len(),
        51,
        "expected all 51 boards, got {}",
        boards.len()
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_returns_the_seeded_board() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Solo Board").await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let board: Option<Board> = blocking(move || backend.get_board(board_id).unwrap()).await;
    let board = board.expect("board should be found");
    assert_eq!(board.id, board_id);
    assert_eq!(board.name, "Solo Board");
    assert_eq!(board.card_prefix, None);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_board_returns_ok_none_for_an_unknown_id() {
    let server = TestServer::start().await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let board: Option<Board> = blocking(move || backend.get_board(Uuid::new_v4()).unwrap()).await;
    assert!(board.is_none());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_column_round_trips_default_status_and_wip_limit() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Column Board").await;
    let column_id = seed_column(
        &server,
        board_id,
        "In Progress",
        Some(3),
        Some(kanban_api::CardStatusDto::InProgress),
    )
    .await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let column: Option<Column> = blocking(move || backend.get_column(column_id).unwrap()).await;
    let column = column.expect("column should be found");
    assert_eq!(column.id, column_id);
    assert_eq!(column.board_id, board_id);
    assert_eq!(column.wip_limit, Some(3));
    assert_eq!(
        column.default_status,
        Some(kanban_domain::CardStatus::InProgress)
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_columns_by_board_returns_columns_in_position_order() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Columns Board").await;
    seed_column(&server, board_id, "Todo", None, None).await;
    seed_column(&server, board_id, "Doing", None, None).await;
    seed_column(&server, board_id, "Done", None, None).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let columns: Vec<Column> =
        blocking(move || backend.list_columns_by_board(board_id).unwrap()).await;

    assert_eq!(columns.len(), 3);
    let positions: Vec<i32> = columns.iter().map(|c| c.position).collect();
    assert!(
        positions.windows(2).all(|w| w[0] <= w[1]),
        "expected ascending positions, got {positions:?}"
    );
    let names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["Todo", "Doing", "Done"]);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_columns_by_board_returns_empty_for_an_unknown_board() {
    let server = TestServer::start().await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let columns: Vec<Column> =
        blocking(move || backend.list_columns_by_board(Uuid::new_v4()).unwrap()).await;
    assert!(columns.is_empty());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_round_trips_board_id_and_prefix() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Card Board").await;
    let column_id = seed_column(&server, board_id, "Todo", None, None).await;
    let card_id = seed_card(&server, column_id, "Ship it", None).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let card: Option<Card> = blocking(move || backend.get_card(card_id).unwrap()).await;
    let card = card.expect("card should be found");
    assert_eq!(card.board_id, board_id);
    assert!(!card.prefix.is_empty());
    assert!(card.card_number > 0);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_returns_ok_none_for_an_unknown_id() {
    let server = TestServer::start().await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let card: Option<Card> = blocking(move || backend.get_card(Uuid::new_v4()).unwrap()).await;
    assert!(card.is_none());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_by_column_returns_only_that_columns_cards() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Multi Column Board").await;
    let col_a = seed_column(&server, board_id, "A", None, None).await;
    let col_b = seed_column(&server, board_id, "B", None, None).await;
    seed_card(&server, col_a, "A1", None).await;
    seed_card(&server, col_a, "A2", None).await;
    seed_card(&server, col_b, "B1", None).await;
    seed_card(&server, col_b, "B2", None).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let cards: Vec<Card> = blocking(move || backend.list_cards_by_column(col_a).unwrap()).await;

    assert_eq!(cards.len(), 2);
    let titles: Vec<&str> = cards.iter().map(|c| c.title.as_str()).collect();
    assert!(titles.contains(&"A1"));
    assert!(titles.contains(&"A2"));
    assert!(!titles.contains(&"B1"));
    assert!(!titles.contains(&"B2"));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_by_column_returns_empty_for_an_unknown_column() {
    let server = TestServer::start().await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let cards: Vec<Card> =
        blocking(move || backend.list_cards_by_column(Uuid::new_v4()).unwrap()).await;
    assert!(cards.is_empty());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_by_sprint_returns_only_that_sprints_cards() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Sprint Board").await;
    let column_id = seed_column(&server, board_id, "Todo", None, None).await;
    let sprint_id = seed_sprint(&server, board_id, "Sprint 1").await;
    seed_card(&server, column_id, "In sprint 1", Some(sprint_id)).await;
    seed_card(&server, column_id, "In sprint 2", Some(sprint_id)).await;
    seed_card(&server, column_id, "Not in sprint", None).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let cards: Vec<Card> = blocking(move || backend.list_cards_by_sprint(sprint_id).unwrap()).await;

    assert_eq!(cards.len(), 2);
    let titles: Vec<&str> = cards.iter().map(|c| c.title.as_str()).collect();
    assert!(titles.contains(&"In sprint 1"));
    assert!(titles.contains(&"In sprint 2"));
    assert!(!titles.contains(&"Not in sprint"));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_sprint_returns_the_seeded_sprint_with_name_index_none() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Sprint Board").await;
    let sprint_id = seed_sprint(&server, board_id, "Alpha").await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let sprint: Option<Sprint> = blocking(move || backend.get_sprint(sprint_id).unwrap()).await;
    let sprint = sprint.expect("sprint should be found");
    assert_eq!(sprint.board_id, board_id);
    assert_eq!(sprint.name_index, None);
    assert_eq!(sprint.status, kanban_domain::SprintStatus::Planning);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_sprints_by_board_returns_the_boards_sprints() {
    let server = TestServer::start().await;
    let board_a = seed_board(&server, "Board A").await;
    let board_b = seed_board(&server, "Board B").await;
    seed_sprint(&server, board_a, "S1").await;
    seed_sprint(&server, board_a, "S2").await;
    seed_sprint(&server, board_b, "S3").await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let sprints: Vec<Sprint> =
        blocking(move || backend.list_sprints_by_board(board_a).unwrap()).await;

    assert_eq!(sprints.len(), 2);
    assert!(sprints.iter().all(|s| s.board_id == board_a));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_sprints_by_board_returns_empty_for_an_unknown_board() {
    let server = TestServer::start().await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let sprints: Vec<Sprint> =
        blocking(move || backend.list_sprints_by_board(Uuid::new_v4()).unwrap()).await;
    assert!(sprints.is_empty());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_card_by_sprint_and_number_returns_the_matching_card() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Sprint Number Board").await;
    let column_id = seed_column(&server, board_id, "Todo", None, None).await;
    let sprint_id = seed_sprint(&server, board_id, "Sprint 1").await;
    let card_a_id = seed_card(&server, column_id, "Card A", Some(sprint_id)).await;
    seed_card(&server, column_id, "Card B", Some(sprint_id)).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let card: Option<Card> = blocking(move || {
        let card_a_number = backend.get_card(card_a_id).unwrap().unwrap().card_number;
        backend
            .get_card_by_sprint_and_number(sprint_id, card_a_number)
            .unwrap()
    })
    .await;
    let card = card.expect("card should be found");
    assert_eq!(card.id, card_a_id);
    assert_eq!(card.title, "Card A");

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_by_columns_returns_cards_from_every_requested_column() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Columns Union Board").await;
    let col_a = seed_column(&server, board_id, "A", None, None).await;
    let col_b = seed_column(&server, board_id, "B", None, None).await;
    let col_c = seed_column(&server, board_id, "C", None, None).await;
    seed_card(&server, col_a, "A1", None).await;
    seed_card(&server, col_b, "B1", None).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let cards: Vec<Card> = blocking(move || {
        backend
            .list_cards_by_columns(&[col_a, col_b, col_c])
            .unwrap()
    })
    .await;

    assert_eq!(cards.len(), 2);
    let titles: Vec<&str> = cards.iter().map(|c| c.title.as_str()).collect();
    assert!(titles.contains(&"A1"));
    assert!(titles.contains(&"B1"));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_prefixes_returns_the_namespace_minted_by_a_card_create() {
    let server = TestServer::start().await;
    let req = CreateBoardRequest {
        id: None,
        name: "Prefix Board".to_string(),
        description: None,
        sprint_prefix: None,
        card_prefix: Some("PFX".to_string()),
        task_sort_field: Some(SortFieldDto::Default),
        task_sort_order: Some(SortOrderDto::Ascending),
        sprint_duration_days: None,
        task_list_view: Some(TaskListViewDto::Flat),
    };
    let resp = server
        .client()
        .post(format!("{}/v1/boards", server.base_url()))
        .json(&req)
        .send()
        .await
        .unwrap();
    let board: kanban_api::BoardResponse = resp.json().await.unwrap();
    let column_id = seed_column(&server, board.id, "Col", None, None).await;
    seed_card(&server, column_id, "First card", None).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let prefixes: Vec<Prefix> = blocking(move || backend.list_prefixes().unwrap()).await;

    let names: Vec<&str> = prefixes.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"pfx"), "expected pfx in {names:?}");

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_prefix_returns_the_row_minted_by_a_card_create() {
    let server = TestServer::start().await;
    let req = CreateBoardRequest {
        id: None,
        name: "Prefix Board".to_string(),
        description: None,
        sprint_prefix: None,
        card_prefix: Some("GET".to_string()),
        task_sort_field: Some(SortFieldDto::Default),
        task_sort_order: Some(SortOrderDto::Ascending),
        sprint_duration_days: None,
        task_list_view: Some(TaskListViewDto::Flat),
    };
    let resp = server
        .client()
        .post(format!("{}/v1/boards", server.base_url()))
        .json(&req)
        .send()
        .await
        .unwrap();
    let board: kanban_api::BoardResponse = resp.json().await.unwrap();
    let column_id = seed_column(&server, board.id, "Col", None, None).await;
    seed_card(&server, column_id, "First card", None).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let prefix: Option<Prefix> = blocking(move || backend.get_prefix("get").unwrap()).await;

    let prefix = prefix.expect("get_prefix should find the minted row");
    assert_eq!(prefix.name, "get");
    assert_eq!(prefix.card_counter, 1);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_prefix_unknown_name_returns_none() {
    let server = TestServer::start().await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let prefix: Option<Prefix> = blocking(move || backend.get_prefix("missing").unwrap()).await;

    assert!(prefix.is_none());

    server.shutdown().await;
}

async fn seed_board_with_card_prefix(server: &TestServer, card_prefix: &str) -> Uuid {
    let req = CreateBoardRequest {
        id: None,
        name: "Prefix Board".to_string(),
        description: None,
        sprint_prefix: None,
        card_prefix: Some(card_prefix.to_string()),
        task_sort_field: Some(SortFieldDto::Default),
        task_sort_order: Some(SortOrderDto::Ascending),
        sprint_duration_days: None,
        task_list_view: Some(TaskListViewDto::Flat),
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

async fn mint_prefix_via_card(server: &TestServer, card_prefix: &str) {
    let board_id = seed_board_with_card_prefix(server, card_prefix).await;
    let column_id = seed_column(server, board_id, "Col", None, None).await;
    seed_card(server, column_id, "First card", None).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_prefix_round_trips_a_name_containing_a_hash() {
    let server = TestServer::start().await;
    mint_prefix_via_card(&server, "a#b").await;
    mint_prefix_via_card(&server, "a").await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let prefix: Option<Prefix> = blocking(move || backend.get_prefix("a#b").unwrap()).await;

    let prefix = prefix.expect("get_prefix should find the a#b row, not the a row");
    assert_eq!(prefix.name, "a#b");
    assert_eq!(prefix.card_counter, 1);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_prefix_round_trips_a_name_containing_a_slash() {
    let server = TestServer::start().await;
    mint_prefix_via_card(&server, "a/b").await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let prefix: Option<Prefix> = blocking(move || backend.get_prefix("a/b").unwrap()).await;

    let prefix = prefix.expect("get_prefix should find the a/b row");
    assert_eq!(prefix.name, "a/b");
    assert_eq!(prefix.card_counter, 1);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_prefix_round_trips_a_name_containing_a_question_mark() {
    let server = TestServer::start().await;
    mint_prefix_via_card(&server, "a?b").await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let prefix: Option<Prefix> = blocking(move || backend.get_prefix("a?b").unwrap()).await;

    let prefix = prefix.expect("get_prefix should find the a?b row");
    assert_eq!(prefix.name, "a?b");
    assert_eq!(prefix.card_counter, 1);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_prefix_round_trips_a_name_containing_a_space() {
    let server = TestServer::start().await;
    mint_prefix_via_card(&server, "a b").await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let prefix: Option<Prefix> = blocking(move || backend.get_prefix("a b").unwrap()).await;

    let prefix = prefix.expect("get_prefix should find the 'a b' row");
    assert_eq!(prefix.name, "a b");
    assert_eq!(prefix.card_counter, 1);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_prefix_round_trips_the_empty_name() {
    let server = TestServer::start().await;
    mint_prefix_via_card(&server, "").await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let prefix: Option<Prefix> = blocking(move || backend.get_prefix("").unwrap()).await;

    let prefix = prefix.expect("get_prefix should find the empty-name row, not report absent");
    assert_eq!(prefix.name, "");
    assert_eq!(prefix.card_counter, 1);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_prefixes_follows_pagination_past_the_first_page() {
    let server = TestServer::start().await;
    for i in 0..75 {
        mint_prefix_via_card(&server, &format!("p{i}")).await;
    }
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let prefixes: Vec<Prefix> = blocking(move || backend.list_prefixes().unwrap()).await;

    let unique: std::collections::HashSet<String> =
        prefixes.iter().map(|p| p.name.clone()).collect();
    assert_eq!(
        unique.len(),
        75,
        "expected all 75 prefixes, got {}",
        prefixes.len()
    );

    server.shutdown().await;
}

#[test]
fn test_a_read_against_an_unreachable_server_maps_to_a_transport_error() {
    let backend = HttpBackend::new("http://127.0.0.1:1").unwrap();

    let err = backend
        .list_boards()
        .expect_err("no server is listening on port 1");

    assert!(err.is_transport(), "expected transport error, got {err:?}");
    assert!(!err.is_unsupported());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_by_prefix_and_number_round_trips_over_http() {
    let server = TestServer::start().await;
    let board_id = seed_board_with_card_prefix(&server, "KAN").await;
    let column_id = seed_column(&server, board_id, "Col", None, None).await;
    let card_id = seed_card(&server, column_id, "Card", None).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let cards: Vec<Card> =
        blocking(move || backend.list_cards_by_prefix_and_number("kan", 1).unwrap()).await;

    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].id, card_id);
    assert_eq!(cards[0].card_number, 1);
    assert!(!cards[0].prefix.is_empty());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_cards_by_number_round_trips_over_http_including_the_ambiguous_case() {
    let server = TestServer::start().await;
    let board_a = seed_board_with_card_prefix(&server, "aaa").await;
    let col_a = seed_column(&server, board_a, "Col", None, None).await;
    let card_a = seed_card(&server, col_a, "Card A", None).await;
    let board_b = seed_board_with_card_prefix(&server, "bbb").await;
    let col_b = seed_column(&server, board_b, "Col", None, None).await;
    let card_b = seed_card(&server, col_b, "Card B", None).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let cards: Vec<Card> = blocking(move || backend.list_cards_by_number(1).unwrap()).await;

    let ids: std::collections::HashSet<Uuid> = cards.iter().map(|c| c.id).collect();
    assert_eq!(ids, [card_a, card_b].into_iter().collect());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_lookup_miss_returns_empty_vec_over_http() {
    let server = TestServer::start().await;
    let board_id = seed_board_with_card_prefix(&server, "KAN").await;
    let column_id = seed_column(&server, board_id, "Col", None, None).await;
    seed_card(&server, column_id, "Card", None).await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let by_prefix: kanban_domain::KanbanResult<Vec<Card>> = blocking({
        let backend = HttpBackend::new(&server.base_url()).unwrap();
        move || backend.list_cards_by_prefix_and_number("kan", 999)
    })
    .await;
    assert!(by_prefix.is_ok(), "expected Ok, got {by_prefix:?}");
    assert!(by_prefix.unwrap().is_empty());

    let by_number: kanban_domain::KanbanResult<Vec<Card>> =
        blocking(move || backend.list_cards_by_number(999)).await;
    assert!(by_number.is_ok(), "expected Ok, got {by_number:?}");
    assert!(by_number.unwrap().is_empty());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_find_cards_by_identifier_resolves_over_http() {
    let server = TestServer::start().await;
    let board_id = seed_board_with_card_prefix(&server, "KAN").await;
    let column_id = seed_column(&server, board_id, "Col", None, None).await;
    let card_id = seed_card(&server, column_id, "Card", None).await;

    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new(&server.base_url()).unwrap());
    let ctx = KanbanContext::open(Arc::clone(&backend), AppConfig::default())
        .await
        .unwrap();

    let by_prefix = ctx.find_cards_by_identifier("KAN-1").unwrap();
    assert_eq!(by_prefix.len(), 1);
    assert_eq!(by_prefix[0].id, card_id);

    let by_number = ctx.find_cards_by_identifier("1").unwrap();
    assert_eq!(by_number.len(), 1);
    assert_eq!(by_number[0].id, card_id);

    let miss = ctx.find_cards_by_identifier("KAN-999").unwrap();
    assert!(miss.is_empty());

    drop(ctx);
    drop(backend);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_resolve_card_ids_resolves_a_batch_over_http() {
    let server = TestServer::start().await;
    let board_id = seed_board_with_card_prefix(&server, "KAN").await;
    let column_id = seed_column(&server, board_id, "Col", None, None).await;
    let card_a = seed_card(&server, column_id, "Card A", None).await;
    let card_b = seed_card(&server, column_id, "Card B", None).await;

    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new(&server.base_url()).unwrap());
    let ctx = KanbanContext::open(Arc::clone(&backend), AppConfig::default())
        .await
        .unwrap();

    let resolved = ctx
        .resolve_card_ids(&["KAN-1".to_string(), "KAN-2".to_string()])
        .unwrap();
    assert_eq!(resolved, vec![card_a, card_b]);

    let mixed = ctx
        .resolve_card_ids(&[card_a.to_string(), "KAN-2".to_string()])
        .unwrap();
    assert_eq!(mixed, vec![card_a, card_b]);

    drop(ctx);
    drop(backend);

    server.shutdown().await;
}

async fn attach_children(server: &TestServer, parent: Uuid, children: &[Uuid]) {
    let req = AttachChildrenRequest {
        children: children.to_vec(),
    };
    let resp = server
        .client()
        .post(format!("{}/v1/cards/{parent}/children", server.base_url()))
        .json(&req)
        .send()
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "attach_children failed: {resp:?}"
    );
}

async fn add_block(server: &TestServer, blocker: Uuid, blocked: Uuid, severity: SeverityDto) {
    let req = AddBlockRequest { blocked, severity };
    let resp = server
        .client()
        .post(format!("{}/v1/cards/{blocker}/blocks", server.base_url()))
        .json(&req)
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "add_block failed: {resp:?}");
}

async fn add_related(server: &TestServer, subject: Uuid, other: Uuid, kind: RelatesKindDto) {
    let req = AddRelatedRequest { other, kind };
    let resp = server
        .client()
        .post(format!("{}/v1/cards/{subject}/related", server.base_url()))
        .json(&req)
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "add_related failed: {resp:?}");
}

async fn archive_card(server: &TestServer, card_id: Uuid) {
    let resp = server
        .client()
        .post(format!("{}/v1/cards/{card_id}/archive", server.base_url()))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "archive_card failed: {resp:?}");
}

struct GraphOnly;

impl FetchPlan for GraphOnly {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        FetchRound {
            graph: requestable(loaded.graph()),
            ..Default::default()
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_get_graph_against_a_server_without_the_route_errors_instead_of_reporting_an_empty_graph(
) {
    let server = TestServer::start().await;
    let backend = HttpBackend::new(&format!("{}/legacy", server.base_url())).unwrap();

    let err = blocking(move || backend.get_graph())
        .await
        .expect_err("a 404 from a route-less server must not be reported as an empty graph");

    match &err {
        kanban_domain::KanbanError::Unsupported { operation } => {
            assert!(
                operation.contains("/v1/graph"),
                "expected the unsupported operation to mention /v1/graph, got {operation:?}"
            );
        }
        other => panic!("expected Unsupported mentioning /v1/graph, got {other:?}"),
    }

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_get_graph_returns_every_edge_kind_including_archived() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Graph Board").await;
    let column_id = seed_column(&server, board_id, "Todo", None, None).await;
    let parent = seed_card(&server, column_id, "Parent", None).await;
    let child = seed_card(&server, column_id, "Child", None).await;
    let blocker = seed_card(&server, column_id, "Blocker", None).await;
    let rel = seed_card(&server, column_id, "Related", None).await;
    let doomed = seed_card(&server, column_id, "Doomed", None).await;

    attach_children(&server, parent, &[child]).await;
    add_block(&server, blocker, parent, SeverityDto::High).await;
    add_related(&server, parent, rel, RelatesKindDto::Duplicates).await;
    attach_children(&server, parent, &[doomed]).await;
    archive_card(&server, doomed).await;

    let backend = HttpBackend::new(&server.base_url()).unwrap();
    let graph: DependencyGraph = blocking(move || backend.get_graph().unwrap()).await;

    let live_spawn = graph
        .spawns_edges()
        .iter()
        .find(|e| e.base.source == parent && e.base.target == child)
        .expect("expected the live parent->child spawns edge");
    assert!(live_spawn.base.archived_at.is_none());

    let archived_spawn = graph
        .spawns_edges()
        .iter()
        .find(|e| e.base.source == parent && e.base.target == doomed)
        .expect("expected the archived parent->doomed spawns edge");
    assert!(archived_spawn.base.archived_at.is_some());

    assert_eq!(graph.blocks_edges()[0].severity, Severity::High);
    assert_eq!(graph.relates_edges()[0].kind, RelatesKind::Duplicates);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_remote_graph_tier_resolves_loaded_and_serves_relation_children() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Tier Board").await;
    let column_id = seed_column(&server, board_id, "Todo", None, None).await;
    let parent = seed_card(&server, column_id, "Parent", None).await;
    let child = seed_card(&server, column_id, "Child", None).await;
    attach_children(&server, parent, &[child]).await;

    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new(&server.base_url()).unwrap());
    let ctx = KanbanContext::open(Arc::clone(&backend), AppConfig::default())
        .await
        .unwrap();

    let mut model = Model::default();
    ctx.sync(&GraphOnly, &mut model, &mut NoProjections);

    let graph = match model.graph_state() {
        LoadState::Loaded(graph) => graph,
        other => panic!("expected LoadState::Loaded, got {other:?}"),
    };
    assert_eq!(graph.children(parent), vec![child]);

    drop(ctx);
    drop(backend);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_invalidating_the_graph_tier_refetches_it_over_http_and_lands_loaded() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Invalidate Board").await;
    let column_id = seed_column(&server, board_id, "Todo", None, None).await;
    let parent = seed_card(&server, column_id, "Parent", None).await;
    let child_a = seed_card(&server, column_id, "Child A", None).await;
    let child_b = seed_card(&server, column_id, "Child B", None).await;
    attach_children(&server, parent, &[child_a]).await;

    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new(&server.base_url()).unwrap());
    let ctx = KanbanContext::open(Arc::clone(&backend), AppConfig::default())
        .await
        .unwrap();

    let mut model = Model::default();
    ctx.sync(&GraphOnly, &mut model, &mut NoProjections);

    let graph = match model.graph_state() {
        LoadState::Loaded(graph) => graph,
        other => panic!("expected LoadState::Loaded, got {other:?}"),
    };
    assert_eq!(graph.children(parent), vec![child_a]);

    attach_children(&server, parent, &[child_b]).await;

    let _ = model.invalidate(Invalidation::Entities(EntityIds::default().with_graph()));
    assert!(
        model.graph_state().is_not_loaded(),
        "graph:true must drop the tier"
    );

    ctx.sync(&GraphOnly, &mut model, &mut NoProjections);

    let mut children = match model.graph_state() {
        LoadState::Loaded(graph) => graph.children(parent),
        other => panic!("expected the refetch to land Loaded, got {other:?}"),
    };
    children.sort();
    let mut expected = vec![child_a, child_b];
    expected.sort();
    assert_eq!(children, expected);

    drop(ctx);
    drop(backend);

    server.shutdown().await;
}

struct BoardScopedPlan {
    board_id: Uuid,
}

impl FetchPlan for BoardScopedPlan {
    fn next_round(&self, loaded: &dyn LoadedEntities) -> FetchRound {
        let mut round = FetchRound {
            board_list: requestable(loaded.board_list()),
            graph: requestable(loaded.graph()),
            ..Default::default()
        };

        if requestable(loaded.columns_of_board(self.board_id)) {
            round.columns_by_board.push(self.board_id);
        }
        if requestable(loaded.sprints_of_board(self.board_id)) {
            round.sprints_by_board.push(self.board_id);
        }
        if requestable(loaded.archived_cards_of_board(self.board_id)) {
            round.archived_cards_by_board.push(self.board_id);
        }
        if let Some(columns) = loaded.loaded_columns_of_board(self.board_id) {
            for column in columns {
                if requestable(loaded.cards_of_column(column.id)) {
                    round.cards_by_column.push(column.id);
                }
            }
        }

        round
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_full_scoped_resolve_over_http_never_hits_a_declining_route() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Scoped Board").await;
    let column_id = seed_column(&server, board_id, "Todo", None, None).await;
    let card_a = seed_card(&server, column_id, "A", None).await;
    let card_b = seed_card(&server, column_id, "B", None).await;
    let _sprint_id = seed_sprint(&server, board_id, "Sprint 1").await;
    archive_card(&server, card_b).await;

    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new(&server.base_url()).unwrap());
    let ctx = KanbanContext::open(Arc::clone(&backend), AppConfig::default())
        .await
        .unwrap();

    let mut model = Model::default();
    let plan = BoardScopedPlan { board_id };
    ctx.sync(&plan, &mut model, &mut NoProjections);

    assert!(model.boards_state().is_loaded());
    assert!(!model.boards_state().is_failed());
    assert!(model.board_columns_state(board_id).is_loaded());
    assert!(!model.board_columns_state(board_id).is_failed());
    assert!(model.column_cards_state(column_id).is_loaded());
    assert!(!model.column_cards_state(column_id).is_failed());
    assert!(model.board_sprints_state(board_id).is_loaded());
    assert!(!model.board_sprints_state(board_id).is_failed());
    assert!(model.board_archived_cards_state(board_id).is_loaded());
    assert!(!model.board_archived_cards_state(board_id).is_failed());
    assert!(model.graph_state().is_loaded());
    assert!(!model.graph_state().is_failed());

    let live_ids: Vec<Uuid> = model
        .column_cards_state(column_id)
        .loaded()
        .unwrap()
        .iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(live_ids, vec![card_a]);

    drop(ctx);
    drop(backend);

    server.shutdown().await;
}
