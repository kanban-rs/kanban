//! KAN-792: the server's board-create seam wires the shared
//! `CreateOrReplaceBoardRequest` through `into_new_board` → `create_or_replace_board`
//! (idempotent PUT-create) and projects the result via `BoardResponse`. The
//! actual HTTP/router binding is out of scope for the stub; this pins the typed
//! seam end-to-end against a real `KanbanContext`.

use kanban_persistence_json::JsonFileStore;
use kanban_server::handlers::boards::create_or_replace_board;
use kanban_service::api::CreateOrReplaceBoardRequest;
use kanban_service::json_backend::JsonDataStore;
use kanban_service::{AppConfig, KanbanBackend, KanbanContext};
use std::sync::Arc;
use tempfile::tempdir;
use uuid::Uuid;

fn make_ctx(path: &std::path::Path) -> KanbanContext {
    let backend: Arc<dyn KanbanBackend> =
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path))));
    KanbanContext::open_deferred(backend, AppConfig::default())
}

fn req_with_id(id: Uuid, name: &str) -> CreateOrReplaceBoardRequest {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "name": name,
        "card_prefix": "KAN",
    }))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_server_seam_put_create_creates_when_absent() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let id = Uuid::new_v4();

    let (resp, created) = create_or_replace_board(&mut ctx, req_with_id(id, "Fresh")).unwrap();

    assert!(created, "absent id must report created (201)");
    assert_eq!(resp.id, id);
    assert_eq!(resp.name, "Fresh");
    assert_eq!(resp.card_prefix, Some("KAN".to_string()));
    assert_eq!(resp.position, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_server_seam_put_create_replaces_when_present() {
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let id = Uuid::new_v4();

    create_or_replace_board(&mut ctx, req_with_id(id, "Original")).unwrap();
    let (resp, created) = create_or_replace_board(&mut ctx, req_with_id(id, "Replaced")).unwrap();

    assert!(!created, "present id must report replace (200)");
    assert_eq!(resp.id, id, "id stable across replace");
    assert_eq!(resp.name, "Replaced");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_server_seam_projects_via_board_response() {
    // The seam returns the wire BoardResponse, not the domain Board: its JSON
    // omits internal allocation state.
    let dir = tempdir().unwrap();
    let mut ctx = make_ctx(&dir.path().join("s.json"));
    let (resp, _) = create_or_replace_board(&mut ctx, req_with_id(Uuid::new_v4(), "B")).unwrap();
    let json = serde_json::to_string(&resp).unwrap();
    for hidden in ["card_counter", "sprint_counters", "next_sprint_number"] {
        assert!(
            !json.contains(hidden),
            "BoardResponse leaked {hidden}: {json}"
        );
    }
}
