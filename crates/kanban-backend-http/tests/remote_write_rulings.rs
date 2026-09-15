use kanban_backend::KanbanBackend as _;
use kanban_backend_http::HttpBackend;
use kanban_domain::{
    BoardUpdate, CreateCardOptions, FieldUpdate, GraphOperations, KanbanOperations, NewBoard,
    UndoOperations,
};
use kanban_server::test_helpers::TestServer;
use kanban_service::{AppConfig, KanbanContext};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

const FENCE_MESSAGE: &str =
    "this operation is not supported over the HTTP backend in v1 (only board/column/card create/update/delete are)";

async fn ctx_over(server: &TestServer) -> KanbanContext {
    let backend = Arc::new(HttpBackend::new(&server.base_url()).unwrap());
    KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap()
}

fn a_new_board() -> NewBoard {
    NewBoard {
        name: "Roadmap".to_string(),
        description: None,
        sprint_prefix: None,
        card_prefix: None,
        task_sort_field: None,
        task_sort_order: None,
        sprint_duration_days: None,
        task_list_view: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_create_board_with_duplicate_client_id_over_http_returns_already_exists() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let id = Uuid::new_v4();
    let _ = ctx.create_board_from_spec(Some(id), a_new_board()).unwrap();

    let err = ctx
        .create_board_from_spec(Some(id), a_new_board())
        .unwrap_err();

    assert!(err.to_string().contains("ALREADY_EXISTS"), "got: {err}");

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_patch_missing_board_over_http_returns_not_found_error_not_ok() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;

    let err = ctx
        .update_board_impl(
            Uuid::new_v4(),
            BoardUpdate {
                name: Some("x".to_string()),
                ..Default::default()
            },
        )
        .unwrap_err();

    assert!(err.to_string().contains("NOT_FOUND"), "got: {err}");

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_board_with_position_set_returns_unsupported_not_silent_drop() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();

    let err = ctx
        .update_board_impl(
            board.id,
            BoardUpdate {
                position: Some(3),
                ..Default::default()
            },
        )
        .unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported(
            "update_board.position over HTTP (server-managed field)"
        )
        .to_string()
    );
    let readback = ctx.data_store().get_board(board.id).unwrap().unwrap();
    assert_eq!(readback.name, board.name);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_update_board_with_active_sprint_id_set_returns_unsupported_not_silent_drop() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;
    let (board, _) = ctx.create_board_from_spec(None, a_new_board()).unwrap();

    let err = ctx
        .update_board_impl(
            board.id,
            BoardUpdate {
                active_sprint_id: FieldUpdate::Set(Uuid::new_v4()),
                ..Default::default()
            },
        )
        .unwrap_err();

    assert!(err.is_unsupported(), "got: {err:?}");
    assert_eq!(
        err.to_string(),
        kanban_domain::KanbanError::unsupported(
            "update_board.active_sprint_id over HTTP (server-managed field)"
        )
        .to_string()
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mutation_sends_client_id_header_visible_as_sse_issued_by() {
    let server = TestServer::start().await;
    let backend = Arc::new(HttpBackend::new(&server.base_url()).unwrap());
    let instance_id = backend.instance_id();
    let mut ctx = KanbanContext::open(backend, AppConfig::default())
        .await
        .unwrap();

    let mut events_response = server
        .client()
        .get(format!("{}/v1/events", server.base_url()))
        .send()
        .await
        .unwrap();

    let _ = ctx.create_board_from_spec(None, a_new_board()).unwrap();

    let frame = tokio::time::timeout(Duration::from_secs(5), async {
        read_one_sse_frame(&mut events_response).await
    })
    .await
    .expect("timed out waiting for the SSE frame");

    assert_eq!(
        frame["issued_by"].as_str().unwrap(),
        instance_id.to_string()
    );

    drop(events_response);
    server.shutdown().await;
}

async fn read_one_sse_frame(response: &mut reqwest::Response) -> serde_json::Value {
    let mut buf = Vec::new();
    loop {
        let chunk = response
            .chunk()
            .await
            .unwrap()
            .expect("stream ended before a full SSE frame arrived");
        buf.extend_from_slice(&chunk);
        let text = String::from_utf8_lossy(&buf);
        if let Some(idx) = text.find("\n\n") {
            let frame_text = text[..idx].to_string();
            let data_line = frame_text
                .lines()
                .find(|l| l.starts_with("data:"))
                .expect("frame must have a data: line");
            return serde_json::from_str(data_line.trim_start_matches("data:").trim()).unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_sprint_and_archive_mutations_over_http_hit_the_fence_message_graph_declines_earlier()
{
    let seeded = Arc::new(std::sync::Mutex::new(None::<(Uuid, Uuid, Uuid, Uuid)>));
    let seeded_for_seed = Arc::clone(&seeded);

    let server = TestServer::start_with(move |ctx| {
        let board_id = ctx
            .create_board("Fence Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let column_id = ctx
            .create_column(board_id, "To Do".to_string(), None)
            .unwrap()
            .id;
        let card_a = ctx
            .create_card(
                board_id,
                column_id,
                "Card A".to_string(),
                CreateCardOptions::default(),
            )
            .unwrap()
            .id;
        let card_b = ctx
            .create_card(
                board_id,
                column_id,
                "Card B".to_string(),
                CreateCardOptions::default(),
            )
            .unwrap()
            .id;
        *seeded_for_seed.lock().unwrap() = Some((board_id, column_id, card_a, card_b));
    })
    .await;

    let (board_id, _column_id, card_a, card_b) = seeded.lock().unwrap().take().unwrap();
    let mut ctx = ctx_over(&server).await;

    let sprint_err = ctx
        .create_sprint_from_spec(board_id, None, None, None, false)
        .unwrap_err();
    assert_eq!(
        sprint_err.to_string(),
        kanban_domain::KanbanError::unsupported(FENCE_MESSAGE).to_string()
    );

    let graph_err = ctx.attach_children(card_a, vec![card_b]).unwrap_err();
    assert_eq!(
        graph_err.to_string(),
        kanban_domain::KanbanError::unsupported("get_archived_card").to_string(),
        "attach_children_impl's edge_born_archived check reads get_archived_card \
         before execute() reaches the fence, so this declines earlier than the \
         fence message rather than with it"
    );

    let archive_err = ctx.archive_board_impl(board_id).unwrap_err();
    assert_eq!(
        archive_err.to_string(),
        kanban_domain::KanbanError::unsupported(FENCE_MESSAGE).to_string()
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_undo_over_http_is_naturally_empty() {
    let server = TestServer::start().await;
    let mut ctx = ctx_over(&server).await;

    let _ = ctx
        .create_board_from_spec(None, a_new_board())
        .expect("the seeding mutation must succeed for this to be a real test");

    assert_eq!(ctx.undo_depth(), 0);
    assert!(!ctx.can_undo());
    assert_eq!(ctx.undo().unwrap(), None);

    server.shutdown().await;
}
