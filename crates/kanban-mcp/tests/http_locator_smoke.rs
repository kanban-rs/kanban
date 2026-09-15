#![cfg(feature = "http")]

use kanban_domain::{CreateCardOptions, GraphOperations, KanbanOperations};
use kanban_mcp::{CreateCardParams, KanbanMcpServer, ListCardChildrenRequest};
use kanban_server::test_helpers::TestServer;
use kanban_service::{AppConfig, StoreManager};
use rmcp::handler::server::wrapper::Parameters;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

fn http_only_store_manager() -> StoreManager {
    let registry = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    backends.register(Box::new(kanban_backend_http::HttpBackendFactory));
    StoreManager::new(registry, backends)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_tool_create_card_against_http_locator_succeeds() {
    let ids = Arc::new(Mutex::new(None::<(Uuid, Uuid)>));
    let ids_for_seed = Arc::clone(&ids);

    let server = TestServer::start_with(move |ctx| {
        let board_id = ctx
            .create_board("MCP Smoke Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let column_id = ctx
            .create_column(board_id, "To Do".to_string(), None)
            .unwrap()
            .id;
        *ids_for_seed.lock().unwrap() = Some((board_id, column_id));
    })
    .await;
    let (board_id, column_id) = ids.lock().unwrap().take().unwrap();

    let store_manager = http_only_store_manager();
    let mcp_server = KanbanMcpServer::new(&store_manager, &server.base_url(), AppConfig::default())
        .await
        .unwrap();

    let result = mcp_server
        .tool_create_card(Parameters(CreateCardParams {
            board: board_id.to_string(),
            column: column_id.to_string(),
            sprint: None,
            content: kanban_service::api::CreateCardRequest {
                id: None,
                title: "Smoke".to_string(),
                description: None,
                priority: None,
                due_date: None,
                points: None,
                sprint_id: None,
            },
        }))
        .await
        .unwrap();

    let text = result
        .content
        .iter()
        .find_map(|c| c.as_text().map(|t| t.text.clone()))
        .expect("tool result should carry a text content block");
    let created: serde_json::Value = serde_json::from_str(&text).unwrap();
    let card_id = created["id"].as_str().unwrap();

    let card: serde_json::Value = server
        .client()
        .get(format!("{}/v1/cards/{card_id}", server.base_url()))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        card["title"], "Smoke",
        "server should hold the created card: {card:?}"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_list_card_children_over_an_http_locator_resolves_the_graph_tier() {
    let ids = Arc::new(Mutex::new(None::<(Uuid, Uuid)>));
    let ids_for_seed = Arc::clone(&ids);

    let server = TestServer::start_with(move |ctx| {
        let board_id = ctx
            .create_board("MCP Graph Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let column_id = ctx
            .create_column(board_id, "To Do".to_string(), None)
            .unwrap()
            .id;
        let parent_id = ctx
            .create_card(
                board_id,
                column_id,
                "Parent".to_string(),
                CreateCardOptions::default(),
            )
            .unwrap()
            .id;
        let child_id = ctx
            .create_card(
                board_id,
                column_id,
                "Child".to_string(),
                CreateCardOptions::default(),
            )
            .unwrap()
            .id;
        ctx.attach_children(parent_id, vec![child_id]).unwrap();
        *ids_for_seed.lock().unwrap() = Some((parent_id, child_id));
    })
    .await;
    let (parent_id, child_id) = ids.lock().unwrap().take().unwrap();

    let store_manager = http_only_store_manager();
    let mcp_server = KanbanMcpServer::new(&store_manager, &server.base_url(), AppConfig::default())
        .await
        .unwrap();

    let result = mcp_server
        .tool_list_card_children(Parameters(ListCardChildrenRequest {
            card: parent_id.to_string(),
            page: None,
            page_size: None,
        }))
        .await
        .unwrap();

    let text = result
        .content
        .iter()
        .find_map(|c| c.as_text().map(|t| t.text.clone()))
        .expect("tool result should carry a text content block");
    let payload: serde_json::Value = serde_json::from_str(&text).unwrap();
    let items = payload["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], child_id.to_string());

    server.shutdown().await;
}
