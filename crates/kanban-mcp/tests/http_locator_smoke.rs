#![cfg(feature = "http")]

use kanban_domain::dependencies::messages;
use kanban_domain::{CreateCardOptions, GraphOperations, KanbanOperations};
use kanban_mcp::{
    ActivateSprintRequest, CreateCardParams, CreateSprintParams, GetColumnRequest,
    GetSprintRequest, KanbanMcpServer, ListCardChildrenRequest, SetCardParentRequest,
};
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

#[tokio::test(flavor = "multi_thread")]
async fn test_tool_get_column_by_name_with_a_board_resolves_over_an_http_locator() {
    let ids = Arc::new(Mutex::new(None::<Uuid>));
    let ids_for_seed = Arc::clone(&ids);

    let server = TestServer::start_with(move |ctx| {
        ctx.create_board("Board A".to_string(), Some("A".to_string()))
            .unwrap();
        let board_b = ctx
            .create_board("Board B".to_string(), Some("B".to_string()))
            .unwrap()
            .id;
        let ready_b = ctx
            .create_column(board_b, "Ready".to_string(), None)
            .unwrap()
            .id;
        *ids_for_seed.lock().unwrap() = Some(ready_b);
    })
    .await;
    let ready_b = ids.lock().unwrap().take().unwrap();

    let store_manager = http_only_store_manager();
    let mcp_server = KanbanMcpServer::new(&store_manager, &server.base_url(), AppConfig::default())
        .await
        .unwrap();

    let result = mcp_server
        .tool_get_column(Parameters(GetColumnRequest {
            board: Some("Board B".to_string()),
            column: "Ready".to_string(),
        }))
        .await
        .unwrap();

    let text = result
        .content
        .iter()
        .find_map(|c| c.as_text().map(|t| t.text.clone()))
        .expect("tool result should carry a text content block");
    let payload: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(payload["id"], ready_b.to_string());

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_tool_set_card_parent_closing_a_cycle_against_http_locator_returns_the_local_hint() {
    let ids = Arc::new(Mutex::new(None::<(Uuid, Uuid)>));
    let ids_for_seed = Arc::clone(&ids);

    let server = TestServer::start_with(move |ctx| {
        let board_id = ctx
            .create_board("MCP Relation Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let column_id = ctx
            .create_column(board_id, "To Do".to_string(), None)
            .unwrap()
            .id;
        let a = ctx
            .create_card(
                board_id,
                column_id,
                "A".to_string(),
                CreateCardOptions::default(),
            )
            .unwrap()
            .id;
        let b = ctx
            .create_card(
                board_id,
                column_id,
                "B".to_string(),
                CreateCardOptions::default(),
            )
            .unwrap()
            .id;
        ctx.attach_children(a, vec![b]).unwrap();
        *ids_for_seed.lock().unwrap() = Some((a, b));
    })
    .await;
    let (a, b) = ids.lock().unwrap().take().unwrap();
    let (a_s, b_s) = (a.to_string(), b.to_string());

    let store_manager = http_only_store_manager();
    let mcp_server = KanbanMcpServer::new(&store_manager, &server.base_url(), AppConfig::default())
        .await
        .unwrap();

    let err = mcp_server
        .tool_set_card_parent(Parameters(SetCardParentRequest {
            child: a_s.clone(),
            parent: b_s.clone(),
        }))
        .await
        .unwrap_err();

    assert_eq!(err.message, messages::parent_cycle(&b_s, &a_s));
    assert_eq!(err.code, rmcp::model::ErrorCode::INVALID_PARAMS);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_tool_create_sprint_over_http_when_the_board_reread_fails_once_returns_the_real_name()
{
    let ids = Arc::new(Mutex::new(None::<Uuid>));
    let ids_for_seed = Arc::clone(&ids);

    let (server, fault) = TestServer::start_with_one_shot_fault(move |ctx| {
        let board_id = ctx
            .create_board("MCP Sprint Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        *ids_for_seed.lock().unwrap() = Some(board_id);
    })
    .await;
    let board_id = ids.lock().unwrap().take().unwrap();
    *fault.lock().unwrap() = Some(("GET", format!("/v1/boards/{board_id}")));

    let store_manager = http_only_store_manager();
    let mcp_server = KanbanMcpServer::new(&store_manager, &server.base_url(), AppConfig::default())
        .await
        .unwrap();

    let result = mcp_server
        .tool_create_sprint(Parameters(CreateSprintParams {
            board: board_id.to_string(),
            content: kanban_service::api::CreateSprintRequest {
                id: None,
                name: Some("Alpha".into()),
                prefix: None,
                card_prefix: None,
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
    assert_eq!(created["name"], "Alpha");
    assert!(
        fault.lock().unwrap().is_none(),
        "the shot should have fired"
    );

    let list: serde_json::Value = server
        .client()
        .get(format!(
            "{}/v1/boards/{board_id}/sprints",
            server.base_url()
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let items = list["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], created["id"]);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_tool_create_sprint_over_http_when_the_board_reads_keep_failing_returns_the_committed_sprint_unnamed(
) {
    let ids = Arc::new(Mutex::new(None::<Uuid>));
    let ids_for_seed = Arc::clone(&ids);

    let (server, fault) = TestServer::start_with_fault(move |ctx| {
        let board_id = ctx
            .create_board("MCP Sprint Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        *ids_for_seed.lock().unwrap() = Some(board_id);
    })
    .await;
    let board_id = ids.lock().unwrap().take().unwrap();
    *fault.lock().unwrap() = Some(("GET", format!("/v1/boards/{board_id}")));

    let store_manager = http_only_store_manager();
    let mcp_server = KanbanMcpServer::new(&store_manager, &server.base_url(), AppConfig::default())
        .await
        .unwrap();

    let result = mcp_server
        .tool_create_sprint(Parameters(CreateSprintParams {
            board: board_id.to_string(),
            content: kanban_service::api::CreateSprintRequest {
                id: None,
                name: Some("Alpha".into()),
                prefix: None,
                card_prefix: None,
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
    assert!(created["name"].is_null());

    let list: serde_json::Value = server
        .client()
        .get(format!(
            "{}/v1/boards/{board_id}/sprints",
            server.base_url()
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let items = list["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], created["id"]);
    assert_eq!(items[0]["name"], "Alpha");

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_tool_activate_sprint_over_http_when_the_board_reread_fails_once_returns_the_real_name(
) {
    let ids = Arc::new(Mutex::new(None::<(Uuid, Uuid)>));
    let ids_for_seed = Arc::clone(&ids);

    let (server, fault) = TestServer::start_with_one_shot_fault(move |ctx| {
        let board_id = ctx
            .create_board("MCP Sprint Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let sprint_id = ctx
            .create_sprint_from_spec(board_id, None, Some("Alpha".to_string()), None, false)
            .unwrap()
            .0
            .id;
        *ids_for_seed.lock().unwrap() = Some((board_id, sprint_id));
    })
    .await;
    let (board_id, sprint_id) = ids.lock().unwrap().take().unwrap();
    *fault.lock().unwrap() = Some(("GET", format!("/v1/boards/{board_id}")));

    let store_manager = http_only_store_manager();
    let mcp_server = KanbanMcpServer::new(&store_manager, &server.base_url(), AppConfig::default())
        .await
        .unwrap();

    let result = mcp_server
        .tool_activate_sprint(Parameters(ActivateSprintRequest {
            board: None,
            sprint: sprint_id.to_string(),
            duration_days: None,
        }))
        .await
        .unwrap();

    let text = result
        .content
        .iter()
        .find_map(|c| c.as_text().map(|t| t.text.clone()))
        .expect("tool result should carry a text content block");
    let activated: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(activated["status"], "active");
    assert_eq!(activated["name"], "Alpha");
    assert!(
        fault.lock().unwrap().is_none(),
        "the shot should have fired"
    );

    let list: serde_json::Value = server
        .client()
        .get(format!(
            "{}/v1/boards/{board_id}/sprints",
            server.base_url()
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let items = list["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], sprint_id.to_string());
    assert_eq!(items[0]["status"], "active");

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_tool_get_sprint_over_http_when_the_board_read_fails_still_errors() {
    let ids = Arc::new(Mutex::new(None::<(Uuid, Uuid)>));
    let ids_for_seed = Arc::clone(&ids);

    let (server, fault) = TestServer::start_with_fault(move |ctx| {
        let board_id = ctx
            .create_board("MCP Sprint Board".to_string(), Some("KAN".to_string()))
            .unwrap()
            .id;
        let sprint_id = ctx
            .create_sprint_from_spec(board_id, None, Some("Alpha".to_string()), None, false)
            .unwrap()
            .0
            .id;
        *ids_for_seed.lock().unwrap() = Some((board_id, sprint_id));
    })
    .await;
    let (board_id, sprint_id) = ids.lock().unwrap().take().unwrap();
    *fault.lock().unwrap() = Some(("GET", format!("/v1/boards/{board_id}")));

    let store_manager = http_only_store_manager();
    let mcp_server = KanbanMcpServer::new(&store_manager, &server.base_url(), AppConfig::default())
        .await
        .unwrap();

    let err = mcp_server
        .tool_get_sprint(Parameters(GetSprintRequest {
            board: None,
            sprint: sprint_id.to_string(),
        }))
        .await
        .unwrap_err();

    assert!(!err.message.is_empty());

    server.shutdown().await;
}
