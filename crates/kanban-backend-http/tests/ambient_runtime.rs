use kanban_api::{CreateBoardRequest, SortFieldDto, SortOrderDto, TaskListViewDto};
use kanban_backend_http::HttpBackend;
use kanban_domain::DataStore;
use kanban_server::test_helpers::TestServer;
use uuid::Uuid;

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_ambient_multi_thread_runtime_read_succeeds_without_spawn_blocking() {
    let server = TestServer::start().await;
    let board_id = seed_board(&server, "Ambient").await;

    let backend = HttpBackend::new(&server.base_url()).unwrap();
    let result = backend.get_board(board_id);

    let board = result.unwrap().unwrap();
    assert_eq!(board.id, board_id);

    server.shutdown().await;
}
