//! KAN-798: the server's sprint-create seam wires the shared
//! `CreateSprintRequest` through the conflicting POST create
//! (`create_sprint_from_spec`) and `ReplaceSprintRequest` through the idempotent
//! PUT-create (`create_or_replace_sprint`), projecting the result via
//! `SprintResponse` (the sprint `name` resolved against its owning board). The
//! actual HTTP/router binding is out of scope for the stub; this pins the typed
//! seam end-to-end against a real `KanbanContext`. The board FK is path-supplied
//! on the nested route, so it is a handler arg, not a body field.

use kanban_persistence_json::{JsonDataStore, JsonFileStore};
use kanban_server::handlers::sprints::{create_or_replace_sprint, create_sprint};
use kanban_service::api::{CreateSprintRequest, ErrorCode, ReplaceSprintRequest};
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

fn create_req(id: Option<Uuid>, name: &str, prefix: Option<&str>) -> CreateSprintRequest {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "name": name,
        "prefix": prefix,
    }))
    .unwrap()
}

fn replace_req(name: Option<&str>, prefix: Option<&str>) -> ReplaceSprintRequest {
    serde_json::from_value(serde_json::json!({
        "name": name,
        "prefix": prefix,
    }))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_server_seam_post_mints_sprint_number() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let board_id = seed_board(&mut ctx);

    let (first, created) =
        create_sprint(&mut ctx, board_id, create_req(None, "Alpha", Some("SPR"))).unwrap();
    assert!(created, "absent id appends and reports created (201)");
    assert_eq!(first.sprint_number, 1, "first sprint mints number 1");
    assert_eq!(first.board_id, board_id);
    assert_eq!(first.name, Some("Alpha".to_string()));

    let (second, _) =
        create_sprint(&mut ctx, board_id, create_req(None, "Beta", Some("SPR"))).unwrap();
    assert_eq!(second.sprint_number, 2, "second sprint bumps the counter");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_server_seam_post_with_existing_id_conflicts_409() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let board_id = seed_board(&mut ctx);
    let id = Uuid::new_v4();

    create_sprint(&mut ctx, board_id, create_req(Some(id), "Alpha", None)).unwrap();

    // A POST re-using an existing client id is a conflict at the service tier,
    // mapped to 409 (Conflict) by the API edge.
    let err = create_sprint(&mut ctx, board_id, create_req(Some(id), "Beta", None)).unwrap_err();
    assert_eq!(err.code, ErrorCode::AlreadyExists);
    assert_eq!(
        err.code.http_status(),
        409,
        "re-POST of an existing client id must map to 409, got: {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_sprint_create_or_replace_is_idempotent() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let board_id = seed_board(&mut ctx);
    let id = Uuid::new_v4();

    let (first, created) = create_or_replace_sprint(
        &mut ctx,
        board_id,
        id,
        replace_req(Some("Alpha"), Some("SPR")),
    )
    .unwrap();
    assert!(created, "novel id creates with that exact id (201)");
    assert_eq!(first.id, id);
    assert_eq!(first.name, Some("Alpha".to_string()));

    let (second, created_again) = create_or_replace_sprint(
        &mut ctx,
        board_id,
        id,
        replace_req(Some("Alpha"), Some("SPR")),
    )
    .unwrap();
    assert!(!created_again, "present id reports replace (200)");
    assert_eq!(second.id, id, "id stable across replace");

    // Idempotent: no duplicate sprint was created.
    assert_eq!(
        ctx.list_sprints(board_id).unwrap().len(),
        1,
        "PUT twice must not duplicate"
    );
}

// create_or_replace_sprint's replace arm never checked that the existing
// sprint (fetched by id alone) actually belongs to the path board_id.
// SprintUpdate has no board_id field, so this wasn't a relocation hole like
// cards/columns — it was worse: a PUT scoped to the WRONG board could
// silently edit a sprint's name/prefix that board doesn't own. Mirrors the
// cross-board guard already on create_or_replace_column.
#[tokio::test(flavor = "multi_thread")]
async fn test_put_sprint_wrong_board_returns_404() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let board_a = seed_board(&mut ctx);
    let board_b = seed_board(&mut ctx);
    let id = Uuid::new_v4();

    create_or_replace_sprint(&mut ctx, board_a, id, replace_req(Some("Alpha"), Some("SPR")))
        .unwrap();

    let err = create_or_replace_sprint(&mut ctx, board_b, id, replace_req(Some("Hijacked"), None))
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
    assert_eq!(err.code.http_status(), 404);

    // The sprint under board_a must be untouched — still owned by board_a
    // with its original prefix, not cleared by the rejected hijack attempt.
    let sprint = ctx.get_sprint(id).unwrap().unwrap();
    assert_eq!(sprint.board_id, board_a);
    assert_eq!(sprint.prefix, Some("SPR".to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_sprint_replace_preserves_server_managed_number() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let board_id = seed_board(&mut ctx);
    let id = Uuid::new_v4();

    let (created, _) = create_or_replace_sprint(
        &mut ctx,
        board_id,
        id,
        replace_req(Some("Original"), Some("SPR")),
    )
    .unwrap();
    let number_before = created.sprint_number;

    let (resp, was_created) =
        create_or_replace_sprint(&mut ctx, board_id, id, replace_req(Some("Renamed"), None))
            .unwrap();

    assert!(!was_created);
    assert_eq!(resp.name, Some("Renamed".to_string()), "content replaced");
    assert_eq!(
        resp.sprint_number, number_before,
        "server-managed sprint_number preserved across replace"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_server_seam_projects_via_sprint_response() {
    // The seam returns the wire SprintResponse: snake_case status, resolved
    // name, and the internal name_index never on the wire.
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let board_id = seed_board(&mut ctx);

    let (resp, _) = create_sprint(&mut ctx, board_id, create_req(None, "Gamma", None)).unwrap();
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["name"], "Gamma");
    assert_eq!(json["board_id"], board_id.to_string());
    assert_eq!(json["status"], "planning");
    assert!(
        json.get("name_index").is_none(),
        "SprintResponse must not leak name_index: {json}"
    );
}
