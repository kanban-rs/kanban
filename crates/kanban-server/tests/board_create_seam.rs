//! Board create/replace seams for the HTTP server, wiring typed request DTOs
//! through domain operations and projecting results via BoardResponse.

use kanban_persistence_json::JsonFileStore;
use kanban_server::handlers::boards::{create_board, create_or_replace_board};
use kanban_service::api::{
    CreateBoardRequest, ReplaceBoardRequest, SortFieldDto, SortOrderDto, TaskListViewDto,
};
use kanban_service::json_backend::JsonDataStore;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
use std::sync::Arc;
use tempfile::tempdir;
use uuid::Uuid;

fn make_ctx(path: &std::path::Path) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    KanbanContext::open_deferred(backend, AppConfig::default())
}

fn create_req(id: Option<Uuid>, name: &str) -> CreateBoardRequest {
    CreateBoardRequest {
        id,
        name: name.to_string(),
        description: None,
        sprint_prefix: None,
        card_prefix: None,
        task_sort_field: None,
        task_sort_order: None,
        sprint_duration_days: None,
        task_list_view: None,
    }
}

fn replace_req(name: &str) -> ReplaceBoardRequest {
    ReplaceBoardRequest {
        name: name.to_string(),
        description: None,
        sprint_prefix: None,
        card_prefix: None,
        task_sort_field: SortFieldDto::Priority,
        task_sort_order: SortOrderDto::Ascending,
        sprint_duration_days: None,
        task_list_view: TaskListViewDto::GroupedByColumn,
        completion_column_id: None,
    }
}

/// `POST /v1/boards`: pure create, server-mints id when absent.
#[tokio::test(flavor = "multi_thread")]
async fn test_create_board_seam_mints_id_when_absent() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));

    let req = create_req(None, "Fresh");

    let resp = create_board(&mut ctx, req).unwrap();

    assert!(resp.id != Uuid::nil(), "absent id must mint a fresh one");
    assert_eq!(resp.name, "Fresh");
}

/// `POST /v1/boards`: honour client-supplied id.
#[tokio::test(flavor = "multi_thread")]
async fn test_create_board_seam_honours_client_supplied_id() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let id = Uuid::new_v4();

    let req = create_req(Some(id), "Fresh");

    let resp = create_board(&mut ctx, req).unwrap();

    assert_eq!(resp.id, id, "client-supplied id must be honoured");
}

/// `POST /v1/boards`: existing id conflicts (AlreadyExists error).
#[tokio::test(flavor = "multi_thread")]
async fn test_create_board_seam_existing_id_conflicts() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let id = Uuid::new_v4();

    let req = create_req(Some(id), "First");
    create_board(&mut ctx, req).unwrap();

    let req2 = create_req(Some(id), "Second");
    let err = create_board(&mut ctx, req2).unwrap_err();

    assert_eq!(err.code, kanban_service::api::ErrorCode::AlreadyExists);
}

/// `PUT /v1/boards/:id`: creates when absent.
#[tokio::test(flavor = "multi_thread")]
async fn test_create_or_replace_board_seam_creates_when_absent() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let id = Uuid::new_v4();

    let req = replace_req("Fresh");

    let (resp, created) = create_or_replace_board(&mut ctx, id, req).unwrap();

    assert!(created, "absent id must report created (201)");
    assert_eq!(resp.id, id);
    assert_eq!(resp.name, "Fresh");
}

/// `PUT /v1/boards/:id`: replaces when present.
#[tokio::test(flavor = "multi_thread")]
async fn test_create_or_replace_board_seam_replaces_when_present() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let id = Uuid::new_v4();

    create_or_replace_board(&mut ctx, id, replace_req("Original")).unwrap();

    let (resp, created) = create_or_replace_board(&mut ctx, id, replace_req("Replaced")).unwrap();

    assert!(!created, "present id must report replace (200)");
    assert_eq!(resp.id, id, "id stable across replace");
    assert_eq!(resp.name, "Replaced");
}

/// `PUT /v1/boards/:id` with completion_column_id on fresh id rejects.
#[tokio::test(flavor = "multi_thread")]
async fn test_create_or_replace_board_seam_with_completion_column_id_on_fresh_id_rejects() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let id = Uuid::new_v4();

    let req = ReplaceBoardRequest {
        completion_column_id: Some(Uuid::new_v4()),
        ..replace_req("Fresh")
    };

    let err = create_or_replace_board(&mut ctx, id, req).unwrap_err();

    assert_eq!(err.code, kanban_service::api::ErrorCode::ValidationFailed);
    assert_eq!(ctx.list_boards().unwrap().len(), 0);
}

/// Both seams project via BoardResponse, omitting internal allocation state.
#[tokio::test(flavor = "multi_thread")]
async fn test_seams_project_via_board_response() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));

    let resp1 = create_board(&mut ctx, create_req(None, "Create")).unwrap();
    let json1 = serde_json::to_string(&resp1).unwrap();

    for hidden in ["card_counter", "sprint_counters", "next_sprint_number"] {
        assert!(
            !json1.contains(hidden),
            "BoardResponse from create_board leaked {hidden}: {json1}"
        );
    }

    let (resp2, _) =
        create_or_replace_board(&mut ctx, Uuid::new_v4(), replace_req("Replace")).unwrap();
    let json2 = serde_json::to_string(&resp2).unwrap();

    for hidden in ["card_counter", "sprint_counters", "next_sprint_number"] {
        assert!(
            !json2.contains(hidden),
            "BoardResponse from create_or_replace_board leaked {hidden}: {json2}"
        );
    }
}
