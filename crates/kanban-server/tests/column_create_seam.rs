//! KAN-794: the server's column-create seam wires the shared
//! `CreateColumnRequest` through `into_new_column(board_id)` →
//! `create_or_replace_column` (idempotent PUT-create) and projects the result
//! via `ColumnResponse`. The actual HTTP/router binding is out of scope for the
//! stub; this pins the typed seam end-to-end against a real `KanbanContext`.

use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_server::handlers::columns::{create_column, create_or_replace_column};
use kanban_service::api::ReplaceColumnRequest;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext, KanbanOperations};
use std::sync::Arc;
use tempfile::tempdir;
use uuid::Uuid;

fn make_ctx(path: &std::path::Path) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    KanbanContext::open_deferred(backend, AppConfig::default())
}

fn seed_board(ctx: &mut KanbanContext) -> Uuid {
    ctx.create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap()
        .id
}

fn replace_req_with_id(
    id: Uuid,
    name: &str,
    position: i32,
    wip_limit: Option<i32>,
) -> ReplaceColumnRequest {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "name": name,
        "position": position,
        "wip_limit": wip_limit,
    }))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_server_seam_post_creates_with_append_position() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let board_id = seed_board(&mut ctx);

    let (first, created) = create_column(
        &mut ctx,
        board_id,
        serde_json::from_value(serde_json::json!({ "name": "To Do" })).unwrap(),
    )
    .unwrap();
    assert!(created);
    assert_eq!(first.position, 0, "first column appends at 0");

    let (second, _) = create_column(
        &mut ctx,
        board_id,
        serde_json::from_value(serde_json::json!({ "name": "Doing" })).unwrap(),
    )
    .unwrap();
    assert_eq!(second.position, 1, "second column appends at 1");
    assert_eq!(second.board_id, board_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_column_create_or_replace_is_idempotent() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let board_id = seed_board(&mut ctx);
    let id = Uuid::new_v4();

    let (first, created) = create_or_replace_column(
        &mut ctx,
        board_id,
        id,
        replace_req_with_id(id, "To Do", 0, Some(3)),
    )
    .unwrap();
    assert!(created, "absent id must report created (201)");
    assert_eq!(first.id, id);
    assert_eq!(first.wip_limit, Some(3));

    let (second, created_again) = create_or_replace_column(
        &mut ctx,
        board_id,
        id,
        replace_req_with_id(id, "To Do", 0, Some(3)),
    )
    .unwrap();
    assert!(!created_again, "present id must report replace (200)");
    assert_eq!(second.id, id, "id stable across replace");

    // Idempotent: no duplicate column was created.
    assert_eq!(
        ctx.list_columns(board_id).unwrap().len(),
        1,
        "PUT twice must not duplicate"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_column_replace_sets_position_from_request() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let board_id = seed_board(&mut ctx);
    let id = Uuid::new_v4();

    create_or_replace_column(
        &mut ctx,
        board_id,
        id,
        replace_req_with_id(id, "Original", 0, Some(5)),
    )
    .unwrap();
    let position_before = ctx.get_column(id).unwrap().unwrap().position;

    // Replace with a different position (PUT sets position from the request).
    let new_position = position_before + 3;
    let (resp, created) = create_or_replace_column(
        &mut ctx,
        board_id,
        id,
        replace_req_with_id(id, "Renamed", new_position, None),
    )
    .unwrap();

    assert!(!created);
    assert_eq!(resp.name, "Renamed", "content replaced");
    assert_eq!(resp.wip_limit, None, "omitted wip_limit cleared on replace");
    assert_eq!(
        resp.position, new_position,
        "position should be set from the request, not preserved"
    );

    // Send back the same position (no-op replace).
    let (resp2, _) = create_or_replace_column(
        &mut ctx,
        board_id,
        id,
        replace_req_with_id(id, "Renamed", new_position, None),
    )
    .unwrap();
    assert_eq!(
        resp2.position, new_position,
        "re-sending position is a safe no-op"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_server_seam_projects_via_column_response() {
    // The seam returns the wire ColumnResponse: its JSON carries exactly the
    // documented fields and round-trips.
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let board_id = seed_board(&mut ctx);

    let (resp, _) = create_column(
        &mut ctx,
        board_id,
        serde_json::from_value(serde_json::json!({ "name": "C" })).unwrap(),
    )
    .unwrap();
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["name"], "C");
    assert_eq!(json["board_id"], board_id.to_string());
    assert!(json.get("created_at").is_some());
}
