use kanban_api::{
    CreateBoardRequest, CreateCardRequest, CreateColumnRequest, CreateSprintRequest, SortFieldDto,
    SortOrderDto, TaskListViewDto,
};
use kanban_backend_http::HttpBackend;
use kanban_domain::{Board, Card, Column, DataStore, Prefix, Sprint};
use kanban_server::test_helpers::TestServer;
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
#[should_panic(expected = "Cannot start a runtime from within a runtime")]
async fn test_a_datastore_call_directly_on_a_runtime_worker_thread_panics() {
    let server = TestServer::start().await;
    let backend = HttpBackend::new(&server.base_url()).unwrap();

    let _ = backend.list_boards();

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
