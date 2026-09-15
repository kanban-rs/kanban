#![cfg(feature = "test-helpers")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use kanban_domain::GraphOperations;
use kanban_server::app;
use kanban_server::layers::LayerConfig;
use kanban_server::state::AppState;
use kanban_server::test_helpers::{json_of, make_sqlite_state, make_state, send};
use kanban_service::api::{ArchivedCardResponse, BoardResponse, Page};
use kanban_service::{KanbanContext, KanbanOperations};
use serde_json::Value;
use tempfile::tempdir;
use tower::ServiceExt;
use uuid::Uuid;

struct SeedIds {
    board: Uuid,
    card1: Uuid,
    card2: Uuid,
    card3: Uuid,
    sprint: Uuid,
    column_a: Uuid,
    column_b: Uuid,
}

fn seed_graph(ctx: &mut KanbanContext, bind_sprint: bool) -> SeedIds {
    let board = ctx
        .create_board("Round Trip".to_string(), Some("RT".to_string()))
        .unwrap()
        .id;
    let column_a = ctx
        .create_column(board, "Todo".to_string(), None)
        .unwrap()
        .id;
    let column_b = ctx
        .create_column(board, "Doing".to_string(), None)
        .unwrap()
        .id;
    let card1 = ctx
        .create_card(board, column_a, "Card One".to_string(), Default::default())
        .unwrap()
        .id;
    let card2 = ctx
        .create_card(board, column_a, "Card Two".to_string(), Default::default())
        .unwrap()
        .id;
    let card3 = ctx
        .create_card(
            board,
            column_b,
            "Card Three".to_string(),
            Default::default(),
        )
        .unwrap()
        .id;
    let sprint = ctx
        .create_sprint(board, Some("RT".to_string()), Some("Sprint 1".to_string()))
        .unwrap()
        .id;
    if bind_sprint {
        ctx.assign_card_to_sprint(card1, sprint).unwrap();
    }
    ctx.block(card1, card2, kanban_domain::Severity::Medium)
        .unwrap();
    ctx.archive_card(card3).unwrap();

    SeedIds {
        board,
        card1,
        card2,
        card3,
        sprint,
        column_a,
        column_b,
    }
}

async fn send_raw(
    state: &AppState,
    method: &str,
    uri: &str,
    body: String,
) -> axum::response::Response {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("content-length", body.len().to_string())
        .body(Body::from(body))
        .unwrap();
    app::router(state.clone()).oneshot(request).await.unwrap()
}

async fn send_raw_with(
    state: &AppState,
    config: LayerConfig,
    method: &str,
    uri: &str,
    body: String,
) -> axum::response::Response {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("content-length", body.len().to_string())
        .body(Body::from(body))
        .unwrap();
    app::router_with(state.clone(), config)
        .oneshot(request)
        .await
        .unwrap()
}

async fn export_body(state: &AppState, board_id: Uuid) -> String {
    let response = send(
        &state.clone(),
        "GET",
        &format!("/v1/boards/{board_id}/export"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_then_import_into_fresh_server_round_trips_the_full_graph() {
    let dir_a = tempdir().unwrap();
    let state_a = make_state(&dir_a.path().join("s.json"));
    let ids = {
        let mut guard = state_a.ctx.lock().await;
        seed_graph(&mut guard.ctx, true)
    };

    let response = send(
        &state_a,
        "GET",
        &format!("/v1/boards/{}/export", ids.board),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_string = String::from_utf8(body_bytes.to_vec()).unwrap();

    let dir_b = tempdir().unwrap();
    let state_b = make_state(&dir_b.path().join("s.json"));

    let response = send_raw(&state_b, "POST", "/v1/import", body_string).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let imported: BoardResponse = serde_json::from_value(json_of(response).await).unwrap();
    assert_eq!(imported.id, ids.board);

    let response = send(&state_b, "GET", &format!("/v1/boards/{}", ids.board), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let board: BoardResponse = serde_json::from_value(json_of(response).await).unwrap();
    assert_eq!(board.name, "Round Trip");

    let response = send(
        &state_b,
        "GET",
        &format!("/v1/boards/{}/columns", ids.board),
        None,
    )
    .await;
    let columns: Value = json_of(response).await;
    let column_ids: Vec<Uuid> = columns["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().parse().unwrap())
        .collect();
    assert!(column_ids.contains(&ids.column_a));
    assert!(column_ids.contains(&ids.column_b));

    let response = send(
        &state_b,
        "GET",
        &format!("/v1/boards/{}/cards", ids.board),
        None,
    )
    .await;
    let cards: Value = json_of(response).await;
    let items = cards["items"].as_array().unwrap();
    let card1_json = items
        .iter()
        .find(|c| c["id"].as_str().unwrap() == ids.card1.to_string())
        .expect("card1 present");
    assert_eq!(
        card1_json["sprint_id"].as_str().unwrap(),
        ids.sprint.to_string()
    );
    assert!(items
        .iter()
        .any(|c| c["id"].as_str().unwrap() == ids.card2.to_string()));

    let response = send(
        &state_b,
        "GET",
        &format!("/v1/boards/{}/sprints", ids.board),
        None,
    )
    .await;
    let sprints: Value = json_of(response).await;
    assert!(sprints["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"].as_str().unwrap() == ids.sprint.to_string()));

    let response = send(
        &state_b,
        "GET",
        &format!("/v1/boards/{}/archived-cards", ids.board),
        None,
    )
    .await;
    let archived: Page<ArchivedCardResponse> =
        serde_json::from_value(json_of(response).await).unwrap();
    assert_eq!(archived.items.len(), 1);
    assert_eq!(archived.items[0].entity_id, ids.card3);

    let response = send(
        &state_b,
        "GET",
        &format!("/v1/cards/{}/graph", ids.card1),
        None,
    )
    .await;
    let graph: Value = json_of(response).await;
    let blocks: Vec<String> = graph["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(blocks.contains(&ids.card2.to_string()));
    let block_edges = graph["block_edges"].as_array().unwrap();
    assert_eq!(block_edges[0]["severity"].as_str().unwrap(), "medium");

    let response = send(&state_b, "GET", "/v1/prefixes?name=RT", None).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_then_import_round_trips_the_full_graph_on_sqlite() {
    let dir_a = tempdir().unwrap();
    let state_a = make_sqlite_state(&dir_a.path().join("s.sqlite")).await;
    let ids = {
        let mut guard = state_a.ctx.lock().await;
        seed_graph(&mut guard.ctx, true)
    };

    let body_string = export_body(&state_a, ids.board).await;

    let dir_b = tempdir().unwrap();
    let state_b = make_sqlite_state(&dir_b.path().join("s.sqlite")).await;

    let response = send_raw(&state_b, "POST", "/v1/import", body_string).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = send(
        &state_b,
        "GET",
        &format!("/v1/boards/{}/cards", ids.board),
        None,
    )
    .await;
    let cards: Value = json_of(response).await;
    let items = cards["items"].as_array().unwrap();
    let card1_json = items
        .iter()
        .find(|c| c["id"].as_str().unwrap() == ids.card1.to_string())
        .expect("card1 present");
    assert_eq!(
        card1_json["sprint_id"].as_str().unwrap(),
        ids.sprint.to_string()
    );
    assert!(items
        .iter()
        .any(|c| c["id"].as_str().unwrap() == ids.card2.to_string()));

    let response = send(
        &state_b,
        "GET",
        &format!("/v1/boards/{}/archived-cards", ids.board),
        None,
    )
    .await;
    let archived: Page<ArchivedCardResponse> =
        serde_json::from_value(json_of(response).await).unwrap();
    assert_eq!(archived.items.len(), 1);
    assert_eq!(archived.items[0].entity_id, ids.card3);

    let response = send(
        &state_b,
        "GET",
        &format!("/v1/boards/{}/columns", ids.board),
        None,
    )
    .await;
    let columns: Value = json_of(response).await;
    let column_ids: Vec<Uuid> = columns["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().parse().unwrap())
        .collect();
    assert!(column_ids.contains(&ids.column_a));
    assert!(column_ids.contains(&ids.column_b));

    let response = send(
        &state_b,
        "GET",
        &format!("/v1/boards/{}/sprints", ids.board),
        None,
    )
    .await;
    let sprints: Value = json_of(response).await;
    assert!(sprints["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"].as_str().unwrap() == ids.sprint.to_string()));

    let response = send(
        &state_b,
        "GET",
        &format!("/v1/cards/{}/graph", ids.card1),
        None,
    )
    .await;
    let graph: Value = json_of(response).await;
    let blocks: Vec<String> = graph["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(blocks.contains(&ids.card2.to_string()));
    let block_edges = graph["block_edges"].as_array().unwrap();
    assert_eq!(block_edges[0]["severity"].as_str().unwrap(), "medium");

    let response = send(&state_b, "GET", "/v1/prefixes?name=RT", None).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_board_route_returns_snapshot_json_with_the_subtree() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let board_id = {
        let mut guard = state.ctx.lock().await;
        let board_id = guard
            .ctx
            .create_board("B".to_string(), Some("BB".to_string()))
            .unwrap()
            .id;
        let column_id = guard
            .ctx
            .create_column(board_id, "Col".to_string(), None)
            .unwrap()
            .id;
        guard
            .ctx
            .create_card(board_id, column_id, "Card".to_string(), Default::default())
            .unwrap();
        board_id
    };

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/export"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("application/json"));
    let body = json_of(response).await;
    assert!(!body["boards"].as_array().unwrap().is_empty());
    assert!(!body["columns"].as_array().unwrap().is_empty());
    assert!(!body["cards"].as_array().unwrap().is_empty());
    assert!(body.get("graph").is_some());
    assert!(body.get("version").is_none());
    assert!(body.get("metadata").is_none());
    assert!(body.get("data").is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_missing_board_returns_404_not_found_envelope() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{}/export", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = json_of(response).await;
    assert_eq!(body["code"].as_str().unwrap(), "NOT_FOUND");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_export_archived_board_succeeds_and_carries_its_marker() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let board_id = {
        let mut guard = state.ctx.lock().await;
        let board_id = guard
            .ctx
            .create_board("Archivable".to_string(), Some("AR".to_string()))
            .unwrap()
            .id;
        let column_id = guard
            .ctx
            .create_column(board_id, "Col".to_string(), None)
            .unwrap()
            .id;
        guard
            .ctx
            .create_card(board_id, column_id, "Card".to_string(), Default::default())
            .unwrap();
        board_id
    };

    let response = send(
        &state,
        "POST",
        &format!("/v1/boards/{board_id}/archive"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = send(
        &state,
        "GET",
        &format!("/v1/boards/{board_id}/export"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_of(response).await;
    assert!(!body["boards"].as_array().unwrap().is_empty());
    let archived_boards = body["archived_boards"].as_array().unwrap();
    assert!(archived_boards
        .iter()
        .any(|m| m["entity_id"].as_str().unwrap() == board_id.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_full_export_route_returns_every_board() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let (board_a, board_b) = {
        let mut guard = state.ctx.lock().await;
        let a = guard
            .ctx
            .create_board("A".to_string(), Some("AA".to_string()))
            .unwrap()
            .id;
        let b = guard
            .ctx
            .create_board("B".to_string(), Some("BB".to_string()))
            .unwrap()
            .id;
        (a, b)
    };

    let response = send(&state, "GET", "/v1/export", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("application/json"));
    let body = json_of(response).await;
    let board_ids: Vec<String> = body["boards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["id"].as_str().unwrap().to_string())
        .collect();
    assert!(board_ids.contains(&board_a.to_string()));
    assert!(board_ids.contains(&board_b.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_with_no_board_returns_422_validation_failed() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let body = r#"{"boards": [], "columns": [], "cards": []}"#.to_string();
    let response = send_raw(&state, "POST", "/v1/import", body).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = json_of(response).await;
    assert_eq!(json["code"].as_str().unwrap(), "VALIDATION_FAILED");
    assert!(json["message"]
        .as_str()
        .unwrap()
        .contains("No board in import data"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_with_colliding_ids_returns_422_and_leaves_the_store_unchanged() {
    let dir_a = tempdir().unwrap();
    let state_a = make_state(&dir_a.path().join("s.json"));
    let board_id = {
        let mut guard = state_a.ctx.lock().await;
        let board_id = guard
            .ctx
            .create_board("Board".to_string(), Some("BB".to_string()))
            .unwrap()
            .id;
        let column_id = guard
            .ctx
            .create_column(board_id, "Col".to_string(), None)
            .unwrap()
            .id;
        guard
            .ctx
            .create_card(board_id, column_id, "Card".to_string(), Default::default())
            .unwrap();
        board_id
    };
    let body_string = export_body(&state_a, board_id).await;

    let dir_b = tempdir().unwrap();
    let state_b = make_state(&dir_b.path().join("s.json"));

    let response = send_raw(&state_b, "POST", "/v1/import", body_string.clone()).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let response = send_raw(&state_b, "POST", "/v1/import", body_string).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = json_of(response).await;
    assert_eq!(json["code"].as_str().unwrap(), "VALIDATION_FAILED");
    assert!(json["message"].as_str().unwrap().contains("Duplicate"));

    let response = send(&state_b, "GET", "/v1/boards", None).await;
    let boards: Value = json_of(response).await;
    assert_eq!(boards["items"].as_array().unwrap().len(), 1);

    let response = send(
        &state_b,
        "GET",
        &format!("/v1/boards/{board_id}/cards"),
        None,
    )
    .await;
    let cards: Value = json_of(response).await;
    assert_eq!(cards["items"].as_array().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_with_dangling_column_reference_returns_422() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let board_id = {
        let mut guard = state.ctx.lock().await;
        let board_id = guard
            .ctx
            .create_board("Board".to_string(), Some("BB".to_string()))
            .unwrap()
            .id;
        let column_id = guard
            .ctx
            .create_column(board_id, "Col".to_string(), None)
            .unwrap()
            .id;
        guard
            .ctx
            .create_card(board_id, column_id, "Card".to_string(), Default::default())
            .unwrap();
        board_id
    };
    let body_string = export_body(&state, board_id).await;
    let mut value: Value = serde_json::from_str(&body_string).unwrap();
    value["columns"] = Value::Array(vec![]);
    if let Some(cards) = value["cards"].as_array_mut() {
        for card in cards.iter_mut() {
            card["column_id"] = Value::String(Uuid::new_v4().to_string());
        }
    }

    let dir_b = tempdir().unwrap();
    let state_b = make_state(&dir_b.path().join("s.json"));

    let response = send_raw(
        &state_b,
        "POST",
        "/v1/import",
        serde_json::to_string(&value).unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = json_of(response).await;
    assert_eq!(json["code"].as_str().unwrap(), "VALIDATION_FAILED");
    assert!(json["message"]
        .as_str()
        .unwrap()
        .contains("which does not exist in the import or the current store"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_of_wrong_shaped_json_returns_422_not_500() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let body = r#"{"boards": 5}"#.to_string();
    let response = send_raw(&state, "POST", "/v1/import", body).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = json_of(response).await;
    assert_eq!(json["code"].as_str().unwrap(), "VALIDATION_FAILED");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_of_syntactically_invalid_body_returns_422() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let response = send_raw(&state, "POST", "/v1/import", "not json at all".to_string()).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = json_of(response).await;
    assert_eq!(json["code"].as_str().unwrap(), "VALIDATION_FAILED");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_of_unrecognized_json_object_reports_the_missing_board() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));

    let body = r#"{"hello": 1}"#.to_string();
    let response = send_raw(&state, "POST", "/v1/import", body).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json = json_of(response).await;
    assert!(json["message"]
        .as_str()
        .unwrap()
        .contains("No board in import data"));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_is_durably_saved_before_the_response() {
    let dir_a = tempdir().unwrap();
    let state_a = make_state(&dir_a.path().join("s.json"));
    let board_id = {
        let mut guard = state_a.ctx.lock().await;
        guard
            .ctx
            .create_board("Durable".to_string(), Some("DU".to_string()))
            .unwrap()
            .id
    };
    let body_string = export_body(&state_a, board_id).await;

    let dir_b = tempdir().unwrap();
    let target_path = dir_b.path().join("s.json");
    let state_b = make_state(&target_path);

    let response = send_raw(&state_b, "POST", "/v1/import", body_string).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let on_disk = std::fs::read_to_string(&target_path).unwrap();
    assert!(on_disk.contains(&board_id.to_string()));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_body_over_the_import_limit_returns_413() {
    let dir = tempdir().unwrap();
    let state = make_state(&dir.path().join("s.json"));
    let config = LayerConfig {
        body_limit_bytes: 512,
        import_body_limit_bytes: 1024,
        ..Default::default()
    };

    let body = "a".repeat(2048);
    let response = send_raw_with(&state, config, "POST", "/v1/import", body).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_import_body_over_the_global_limit_is_accepted_under_the_import_limit() {
    let dir_a = tempdir().unwrap();
    let state_a = make_state(&dir_a.path().join("s.json"));
    let board_id = {
        let mut guard = state_a.ctx.lock().await;
        let board_id = guard
            .ctx
            .create_board("Padded".to_string(), Some("PA".to_string()))
            .unwrap()
            .id;
        let column_id = guard
            .ctx
            .create_column(board_id, "Col".to_string(), None)
            .unwrap()
            .id;
        guard
            .ctx
            .create_card(
                board_id,
                column_id,
                "Card".to_string(),
                kanban_domain::CreateCardOptions {
                    description: Some("x".repeat(1024)),
                    ..Default::default()
                },
            )
            .unwrap();
        board_id
    };
    let body_string = export_body(&state_a, board_id).await;
    assert!(body_string.len() > 512);

    let dir_b = tempdir().unwrap();
    let state_b = make_state(&dir_b.path().join("s.json"));
    let config = LayerConfig {
        body_limit_bytes: 512,
        import_body_limit_bytes: 1024 * 1024,
        ..Default::default()
    };

    let response = send_raw_with(&state_b, config.clone(), "POST", "/v1/import", body_string).await;
    assert_eq!(response.status(), StatusCode::CREATED);

    let oversized_board = r#"{"name":"Too Big","card_prefix":"TB","padding":""}"#
        .replace("\"\"", &format!("\"{}\"", "a".repeat(1024)));
    let response = send_raw_with(&state_b, config, "POST", "/v1/boards", oversized_board).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
