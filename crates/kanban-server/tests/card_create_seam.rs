//! KAN-796: the server's card-create seam wires the shared `CreateCardRequest`
//! through `into_new_card(column_id)` → `create_or_replace_card` (idempotent
//! PUT-create) and projects the result via `CardResponse`. The actual
//! HTTP/router binding is out of scope for the stub; this pins the typed seam
//! end-to-end against a real `KanbanContext`.

use kanban_persistence_json::JsonFileStore;
use kanban_server::handlers::cards::{create_card, create_or_replace_card};
use kanban_service::api::CreateCardRequest;
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

/// Seed a board with a single column; the card seam takes the column id as the
/// path-supplied FK (the owning board is derived from the column server-side).
fn seed_column(ctx: &mut KanbanContext) -> Uuid {
    let board = ctx
        .create_board("Board".to_string(), Some("KAN".to_string()))
        .unwrap();
    ctx.create_column(board.id, "TODO".to_string(), Some(0))
        .unwrap()
        .id
}

#[tokio::test(flavor = "multi_thread")]
async fn test_server_seam_post_creates_card_with_seeded_number() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let column_id = seed_column(&mut ctx);

    let (first, created) = create_card(
        &mut ctx,
        column_id,
        serde_json::from_value(serde_json::json!({ "title": "First" })).unwrap(),
    )
    .unwrap();
    assert!(created, "absent id must report created (201)");
    assert_eq!(first.title, "First");
    assert_eq!(first.column_id, column_id);
    // Factory-seeded user-facing number from the board counter.
    assert_eq!(first.card_number, 1);
    assert_eq!(first.position, 0, "first card appends at 0");

    let (second, _) = create_card(
        &mut ctx,
        column_id,
        serde_json::from_value(serde_json::json!({ "title": "Second" })).unwrap(),
    )
    .unwrap();
    assert_eq!(second.card_number, 2, "second card bumps the board counter");
    assert_eq!(second.position, 1, "second card appends at 1");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_card_create_or_replace_is_idempotent() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let column_id = seed_column(&mut ctx);
    let id = Uuid::new_v4();

    let req = |title: &str| -> CreateCardRequest {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "title": title,
            "priority": "high",
            "points": 3,
        }))
        .unwrap()
    };

    let (first, created) =
        create_or_replace_card(&mut ctx, column_id, id, req("Original")).unwrap();
    assert!(created, "absent id must report created (201)");
    assert_eq!(first.id, id);
    assert_eq!(first.points, Some(3));

    let (second, created_again) =
        create_or_replace_card(&mut ctx, column_id, id, req("Replaced")).unwrap();
    assert!(!created_again, "present id must report replace (200)");
    assert_eq!(second.id, id, "id stable across replace");
    assert_eq!(second.title, "Replaced");

    // Idempotent: PUT twice does not duplicate the card.
    let cards = ctx.list_cards(Default::default()).unwrap();
    assert_eq!(cards.len(), 1, "PUT twice must not duplicate");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_put_card_replace_preserves_server_managed_number_and_position() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let column_id = seed_column(&mut ctx);
    let id = Uuid::new_v4();

    let (created, _) = create_or_replace_card(
        &mut ctx,
        column_id,
        id,
        serde_json::from_value(serde_json::json!({ "id": id, "title": "Original", "points": 5 }))
            .unwrap(),
    )
    .unwrap();
    let number_before = created.card_number;
    let position_before = created.position;

    // Replace with a new title and an omitted points (wholesale clear).
    let (resp, created_flag) = create_or_replace_card(
        &mut ctx,
        column_id,
        id,
        serde_json::from_value(serde_json::json!({ "id": id, "title": "Renamed" })).unwrap(),
    )
    .unwrap();

    assert!(!created_flag);
    assert_eq!(resp.title, "Renamed", "content replaced");
    assert_eq!(resp.points, None, "omitted points cleared on replace");
    assert_eq!(
        resp.card_number, number_before,
        "server-managed card_number preserved across replace"
    );
    assert_eq!(
        resp.position, position_before,
        "server-managed position preserved across replace"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_server_seam_projects_via_card_response() {
    // The seam returns the wire CardResponse, not the domain Card: its JSON
    // hides internal history (`sprint_logs`) and serializes wire enums snake_case.
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let column_id = seed_column(&mut ctx);

    let (resp, _) = create_card(
        &mut ctx,
        column_id,
        serde_json::from_value(serde_json::json!({ "title": "C", "priority": "high" })).unwrap(),
    )
    .unwrap();
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["title"], "C");
    assert_eq!(json["priority"], "high", "wire enum is snake_case");
    assert_eq!(json["status"], "todo");
    assert!(json.get("created_at").is_some());
    assert!(
        json.get("sprint_logs").is_none(),
        "CardResponse hides internal sprint_logs: {json}"
    );
}
