use crate::helpers::model_read::require_loaded;
use crate::helpers::{
    core_err_to_mcp, kanban_err_to_mcp, locked_read, locked_write, mcp_enrich_add_error,
    mcp_enrich_remove_error, resolve_summaries, to_call_tool_result, to_call_tool_result_json,
};
use crate::requests::card::{
    ListCardChildrenRequest, ListCardParentsRequest, RemoveCardParentRequest, SetCardParentRequest,
};
use crate::scope::{ToolScope, ToolScoped};
use crate::KanbanMcpServer;
use kanban_core::{resolve_page_params, PaginatedList};
use kanban_domain::{KanbanOperations, Model};
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ErrorData as McpError},
    tool, tool_router,
};
use uuid::Uuid;

impl ToolScoped for SetCardParentRequest {
    fn scope(&self) -> ToolScope {
        ToolScope::default()
    }
}

impl ToolScoped for RemoveCardParentRequest {
    fn scope(&self) -> ToolScope {
        ToolScope::default()
    }
}

impl ToolScoped for ListCardParentsRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            wants_graph: true,
            ..Default::default()
        }
    }
}

impl ToolScoped for ListCardChildrenRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            wants_graph: true,
            ..Default::default()
        }
    }
}

fn list_parents_in_model(model: &Model, child: Uuid) -> Result<Vec<Uuid>, McpError> {
    let graph = require_loaded(model.graph_state().as_ref(), "dependency graph")?;
    Ok(graph.parents(child))
}

fn list_children_in_model(model: &Model, parent: Uuid) -> Result<Vec<Uuid>, McpError> {
    let graph = require_loaded(model.graph_state().as_ref(), "dependency graph")?;
    Ok(graph.children(parent))
}

#[tool_router(router = card_relations_router, vis = "pub(crate)")]
impl KanbanMcpServer {
    #[tool(
        description = "Add a parent -> child edge between two cards. Rejects cycles and self-references."
    )]
    pub async fn tool_set_card_parent(
        &self,
        Parameters(req): Parameters<SetCardParentRequest>,
    ) -> Result<CallToolResult, McpError> {
        let parent_raw = req.parent.clone();
        let child_raw = req.child.clone();
        let (child_id, parent_id) = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let child_id = ctx.resolve_card_id(&req.child).map_err(kanban_err_to_mcp)?;
            let parent_id = ctx
                .resolve_card_id(&req.parent)
                .map_err(kanban_err_to_mcp)?;
            let _inv = ctx
                .mutate_unit(|c| c.attach_children_impl(parent_id, vec![child_id]))
                .map_err(|e| mcp_enrich_add_error(e, &parent_raw, &child_raw))?;
            Ok((child_id, parent_id))
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({
            "parent": parent_id.to_string(),
            "child":  child_id.to_string(),
        }))
    }

    #[tool(description = "Remove a parent -> child edge between two cards.")]
    pub async fn tool_remove_card_parent(
        &self,
        Parameters(req): Parameters<RemoveCardParentRequest>,
    ) -> Result<CallToolResult, McpError> {
        let parent_raw = req.parent.clone();
        let child_raw = req.child.clone();
        let (child_id, parent_id) = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let child_id = ctx.resolve_card_id(&req.child).map_err(kanban_err_to_mcp)?;
            let parent_id = ctx
                .resolve_card_id(&req.parent)
                .map_err(kanban_err_to_mcp)?;
            let _inv = ctx
                .mutate_unit(|c| c.detach_children_impl(parent_id, vec![child_id]))
                .map_err(|e| mcp_enrich_remove_error(e, &parent_raw, &child_raw))?;
            Ok((child_id, parent_id))
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({
            "parent": parent_id.to_string(),
            "child":  child_id.to_string(),
        }))
    }

    #[tool(
        description = "List direct parents of a card. Use page/page_size for pagination (default: page=1, page_size=50)."
    )]
    pub async fn tool_list_card_parents(
        &self,
        Parameters(req): Parameters<ListCardParentsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let parents = locked_read(&self.ctx, |ctx| -> Result<_, McpError> {
            let model = ctx.model_for(&scope);
            let id = ctx.resolve_card_id(&req.card).map_err(kanban_err_to_mcp)?;
            let ids = list_parents_in_model(&model, id)?;
            Ok(resolve_summaries(ctx, ids))
        })
        .await?;
        let (page, page_size) =
            resolve_page_params(req.page, req.page_size).map_err(core_err_to_mcp)?;
        let paged = PaginatedList::paginate(parents, page, page_size).map_err(core_err_to_mcp)?;
        to_call_tool_result(&paged)
    }

    #[tool(
        description = "List direct children of a card. Use page/page_size for pagination (default: page=1, page_size=50)."
    )]
    pub async fn tool_list_card_children(
        &self,
        Parameters(req): Parameters<ListCardChildrenRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let children = locked_read(&self.ctx, |ctx| -> Result<_, McpError> {
            let model = ctx.model_for(&scope);
            let id = ctx.resolve_card_id(&req.card).map_err(kanban_err_to_mcp)?;
            let ids = list_children_in_model(&model, id)?;
            Ok(resolve_summaries(ctx, ids))
        })
        .await?;
        let (page, page_size) =
            resolve_page_params(req.page, req.page_size).map_err(core_err_to_mcp)?;
        let paged = PaginatedList::paginate(children, page, page_size).map_err(core_err_to_mcp)?;
        to_call_tool_result(&paged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::requests::board::CreateBoardRequest;
    use crate::requests::card::CreateCardParams;
    use crate::requests::column::CreateColumnParams;
    use crate::McpServer;
    use kanban_backend::{KanbanBackend, KanbanBackendFactory};
    use kanban_core::AppConfig;
    use kanban_persistence_json::{JsonBackendFactory, JsonStoreFactory};
    use kanban_persistence_sqlite::SqliteBackendFactory;
    use kanban_service::test_helpers::FaultInjectingBackend;
    use rmcp::model::ErrorCode;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    struct RecordingFactory {
        inner: Box<dyn KanbanBackendFactory>,
        handle: Arc<Mutex<Option<Arc<FaultInjectingBackend>>>>,
    }

    #[async_trait::async_trait]
    impl KanbanBackendFactory for RecordingFactory {
        fn name(&self) -> &str {
            self.inner.name()
        }

        fn matches_locator(&self, locator: &str, header: &[u8]) -> bool {
            self.inner.matches_locator(locator, header)
        }

        async fn create(
            &self,
            locator: &str,
            config: &AppConfig,
        ) -> kanban_domain::KanbanResult<Arc<dyn KanbanBackend>> {
            let inner = self.inner.create(locator, config).await?;
            let wrapped = Arc::new(FaultInjectingBackend::new(inner));
            *self.handle.lock().unwrap() = Some(Arc::clone(&wrapped));
            Ok(wrapped as Arc<dyn KanbanBackend>)
        }
    }

    fn text_payload(result: &rmcp::model::CallToolResult) -> serde_json::Value {
        let raw = &result.content[0]
            .as_text()
            .expect("expected text content")
            .text;
        serde_json::from_str(raw).expect("tool result is JSON")
    }

    struct Seeded {
        server: KanbanMcpServer,
        _dir: TempDir,
        handle: Arc<FaultInjectingBackend>,
        card_a_id: String,
        card_a_identifier: String,
        card_b_id: String,
        card_b_identifier: String,
    }

    async fn seeded_server(file_name: &str) -> Seeded {
        let sqlite_handle = Arc::new(Mutex::new(None));
        let json_handle = Arc::new(Mutex::new(None));
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(file_name);

        let server = McpServer::default()
            .register_backend_only(Box::new(RecordingFactory {
                inner: Box::new(SqliteBackendFactory),
                handle: Arc::clone(&sqlite_handle),
            }))
            .register_backend(
                Box::new(JsonStoreFactory),
                Box::new(RecordingFactory {
                    inner: Box::new(JsonBackendFactory),
                    handle: Arc::clone(&json_handle),
                }),
            )
            .with_data_file(path.to_string_lossy().to_string())
            .build()
            .await
            .unwrap();

        let board = text_payload(
            &server
                .tool_create_board(Parameters(crate::requests::board::CreateBoardParams {
                    content: CreateBoardRequest {
                        id: None,
                        name: "Alpha".to_string(),
                        description: None,
                        sprint_prefix: None,
                        card_prefix: None,
                        task_sort_field: None,
                        task_sort_order: None,
                        sprint_duration_days: None,
                        task_list_view: None,
                    },
                    with_default_columns: None,
                }))
                .await
                .unwrap(),
        );
        let board_id = board["id"].as_str().unwrap().to_string();

        let column = text_payload(
            &server
                .tool_create_column(Parameters(CreateColumnParams {
                    board: board_id.clone(),
                    content: kanban_service::api::CreateColumnRequest {
                        id: None,
                        name: "TODO".to_string(),
                        wip_limit: None,
                        default_status: None,
                    },
                }))
                .await
                .unwrap(),
        );
        let column_id = column["id"].as_str().unwrap().to_string();

        let card_a = text_payload(
            &server
                .tool_create_card(Parameters(CreateCardParams {
                    board: board_id.clone(),
                    column: column_id.clone(),
                    sprint: None,
                    content: kanban_service::api::CreateCardRequest {
                        id: None,
                        title: "Card A".to_string(),
                        description: None,
                        priority: None,
                        due_date: None,
                        points: None,
                        sprint_id: None,
                    },
                }))
                .await
                .unwrap(),
        );
        let card_a_id = card_a["id"].as_str().unwrap().to_string();
        let card_a_identifier = format!(
            "{}-{}",
            card_a["prefix"].as_str().unwrap(),
            card_a["card_number"].as_u64().unwrap()
        );

        let card_b = text_payload(
            &server
                .tool_create_card(Parameters(CreateCardParams {
                    board: board_id.clone(),
                    column: column_id.clone(),
                    sprint: None,
                    content: kanban_service::api::CreateCardRequest {
                        id: None,
                        title: "Card B".to_string(),
                        description: None,
                        priority: None,
                        due_date: None,
                        points: None,
                        sprint_id: None,
                    },
                }))
                .await
                .unwrap(),
        );
        let card_b_id = card_b["id"].as_str().unwrap().to_string();
        let card_b_identifier = format!(
            "{}-{}",
            card_b["prefix"].as_str().unwrap(),
            card_b["card_number"].as_u64().unwrap()
        );

        let handle = sqlite_handle
            .lock()
            .unwrap()
            .clone()
            .or_else(|| json_handle.lock().unwrap().clone())
            .expect("a backend must have been created");

        Seeded {
            server,
            _dir: dir,
            handle,
            card_a_id,
            card_a_identifier,
            card_b_id,
            card_b_identifier,
        }
    }

    #[test]
    fn test_a_relation_tool_with_a_not_loaded_graph_errors_instead_of_reporting_no_edges() {
        let err = list_parents_in_model(&Model::default(), Uuid::new_v4()).unwrap_err();
        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("graph"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test]
    async fn test_set_card_parent_with_an_unloadable_card_index_errors_naming_the_lookup_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_cards_by_prefix_and_number");

        let err = seeded
            .server
            .tool_set_card_parent(Parameters(SetCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: seeded.card_b_identifier.clone(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_set_card_parent_with_an_unloadable_card_index_errors_naming_the_lookup_on_sqlite()
    {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_cards_by_prefix_and_number");

        let err = seeded
            .server
            .tool_set_card_parent(Parameters(SetCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: seeded.card_b_identifier.clone(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test]
    async fn test_remove_card_parent_with_an_unloadable_card_index_errors_naming_the_lookup_on_json(
    ) {
        let seeded = seeded_server("test.json").await;
        seeded
            .server
            .tool_set_card_parent(Parameters(SetCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: seeded.card_b_identifier.clone(),
            }))
            .await
            .unwrap();
        seeded.handle.clear_ops();
        seeded.handle.fail("list_cards_by_prefix_and_number");

        let err = seeded
            .server
            .tool_remove_card_parent(Parameters(RemoveCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: seeded.card_b_identifier.clone(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_remove_card_parent_with_an_unloadable_card_index_errors_naming_the_lookup_on_sqlite(
    ) {
        let seeded = seeded_server("test.sqlite").await;
        seeded
            .server
            .tool_set_card_parent(Parameters(SetCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: seeded.card_b_identifier.clone(),
            }))
            .await
            .unwrap();
        seeded.handle.clear_ops();
        seeded.handle.fail("list_cards_by_prefix_and_number");

        let err = seeded
            .server
            .tool_remove_card_parent(Parameters(RemoveCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: seeded.card_b_identifier.clone(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test]
    async fn test_set_card_parent_does_not_fetch_the_graph_or_the_card_list_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();

        seeded
            .server
            .tool_set_card_parent(Parameters(SetCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: seeded.card_b_identifier.clone(),
            }))
            .await
            .unwrap();

        assert_eq!(seeded.handle.op_count("get_graph"), 0);
        assert_eq!(seeded.handle.op_count("list_all_cards"), 0);
        assert_eq!(seeded.handle.op_count("list_cards_by_prefix_and_number"), 2);

        let response = text_payload(
            &seeded
                .server
                .tool_list_card_children(Parameters(ListCardChildrenRequest {
                    card: seeded.card_a_identifier.clone(),
                    page: None,
                    page_size: None,
                }))
                .await
                .unwrap(),
        );
        let items = response["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], seeded.card_b_id);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_set_card_parent_does_not_fetch_the_graph_or_the_card_list_on_sqlite() {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();

        seeded
            .server
            .tool_set_card_parent(Parameters(SetCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: seeded.card_b_identifier.clone(),
            }))
            .await
            .unwrap();

        assert_eq!(seeded.handle.op_count("get_graph"), 0);
        assert_eq!(seeded.handle.op_count("list_all_cards"), 0);
        assert_eq!(seeded.handle.op_count("list_cards_by_prefix_and_number"), 2);

        let response = text_payload(
            &seeded
                .server
                .tool_list_card_children(Parameters(ListCardChildrenRequest {
                    card: seeded.card_a_identifier.clone(),
                    page: None,
                    page_size: None,
                }))
                .await
                .unwrap(),
        );
        let items = response["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], seeded.card_b_id);
    }

    #[tokio::test]
    async fn test_remove_card_parent_does_not_fetch_the_graph_or_the_card_list_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded
            .server
            .tool_set_card_parent(Parameters(SetCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: seeded.card_b_identifier.clone(),
            }))
            .await
            .unwrap();
        seeded.handle.clear_ops();

        seeded
            .server
            .tool_remove_card_parent(Parameters(RemoveCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: seeded.card_b_identifier.clone(),
            }))
            .await
            .unwrap();

        assert_eq!(seeded.handle.op_count("get_graph"), 0);
        assert_eq!(seeded.handle.op_count("list_all_cards"), 0);
        assert_eq!(seeded.handle.op_count("list_cards_by_prefix_and_number"), 2);

        let response = text_payload(
            &seeded
                .server
                .tool_list_card_children(Parameters(ListCardChildrenRequest {
                    card: seeded.card_a_identifier.clone(),
                    page: None,
                    page_size: None,
                }))
                .await
                .unwrap(),
        );
        let items = response["items"].as_array().unwrap();
        assert!(items.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_remove_card_parent_does_not_fetch_the_graph_or_the_card_list_on_sqlite() {
        let seeded = seeded_server("test.sqlite").await;
        seeded
            .server
            .tool_set_card_parent(Parameters(SetCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: seeded.card_b_identifier.clone(),
            }))
            .await
            .unwrap();
        seeded.handle.clear_ops();

        seeded
            .server
            .tool_remove_card_parent(Parameters(RemoveCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: seeded.card_b_identifier.clone(),
            }))
            .await
            .unwrap();

        assert_eq!(seeded.handle.op_count("get_graph"), 0);
        assert_eq!(seeded.handle.op_count("list_all_cards"), 0);
        assert_eq!(seeded.handle.op_count("list_cards_by_prefix_and_number"), 2);

        let response = text_payload(
            &seeded
                .server
                .tool_list_card_children(Parameters(ListCardChildrenRequest {
                    card: seeded.card_a_identifier.clone(),
                    page: None,
                    page_size: None,
                }))
                .await
                .unwrap(),
        );
        let items = response["items"].as_array().unwrap();
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn test_list_card_parents_still_fetches_the_graph_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded
            .server
            .tool_set_card_parent(Parameters(SetCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: seeded.card_b_identifier.clone(),
            }))
            .await
            .unwrap();
        seeded.handle.clear_ops();

        seeded
            .server
            .tool_list_card_parents(Parameters(ListCardParentsRequest {
                card: seeded.card_b_identifier.clone(),
                page: None,
                page_size: None,
            }))
            .await
            .unwrap();

        assert!(seeded.handle.op_count("get_graph") >= 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_list_card_parents_still_fetches_the_graph_on_sqlite() {
        let seeded = seeded_server("test.sqlite").await;
        seeded
            .server
            .tool_set_card_parent(Parameters(SetCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: seeded.card_b_identifier.clone(),
            }))
            .await
            .unwrap();
        seeded.handle.clear_ops();

        seeded
            .server
            .tool_list_card_parents(Parameters(ListCardParentsRequest {
                card: seeded.card_b_identifier.clone(),
                page: None,
                page_size: None,
            }))
            .await
            .unwrap();

        assert!(seeded.handle.op_count("get_graph") >= 1);
    }

    #[tokio::test]
    async fn test_list_card_children_resolves_the_card_by_identifier_from_the_model() {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();

        seeded
            .server
            .tool_set_card_parent(Parameters(SetCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: seeded.card_b_identifier.clone(),
            }))
            .await
            .unwrap();

        let response = text_payload(
            &seeded
                .server
                .tool_list_card_children(Parameters(ListCardChildrenRequest {
                    card: seeded.card_a_identifier.clone(),
                    page: None,
                    page_size: None,
                }))
                .await
                .unwrap(),
        );

        let items = response["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], seeded.card_b_id);
    }

    #[tokio::test]
    async fn test_remove_card_parent_distinguishes_a_missing_card_from_a_not_loaded_one() {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();

        let not_found_err = seeded
            .server
            .tool_remove_card_parent(Parameters(RemoveCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: "KAN-999".to_string(),
            }))
            .await
            .unwrap_err();

        assert_eq!(not_found_err.code, ErrorCode::INVALID_PARAMS);
        assert!(not_found_err.message.to_lowercase().contains("not found"));

        seeded.handle.fail("list_cards_by_prefix_and_number");

        let fault_err = seeded
            .server
            .tool_remove_card_parent(Parameters(RemoveCardParentRequest {
                parent: seeded.card_a_identifier.clone(),
                child: "KAN-999".to_string(),
            }))
            .await
            .unwrap_err();

        assert_eq!(fault_err.code, ErrorCode::INTERNAL_ERROR);
        assert!(fault_err.message.contains("injected fault"));

        assert_ne!(not_found_err.code, fault_err.code);
        assert_ne!(not_found_err.message, fault_err.message);
        let _ = (&seeded.card_a_id, &seeded.card_b_id);
    }

    async fn assert_set_and_remove_card_parent_on_an_archived_board_still_mutate_the_graph(
        file_name: &str,
    ) {
        let seeded = seeded_server(file_name).await;

        let second_board = text_payload(
            &seeded
                .server
                .tool_create_board(Parameters(crate::requests::board::CreateBoardParams {
                    content: CreateBoardRequest {
                        id: None,
                        name: "Beta".to_string(),
                        description: None,
                        sprint_prefix: None,
                        card_prefix: Some("BETA".to_string()),
                        task_sort_field: None,
                        task_sort_order: None,
                        sprint_duration_days: None,
                        task_list_view: None,
                    },
                    with_default_columns: None,
                }))
                .await
                .unwrap(),
        );
        let second_board_id = second_board["id"].as_str().unwrap().to_string();

        let second_column = text_payload(
            &seeded
                .server
                .tool_create_column(Parameters(CreateColumnParams {
                    board: second_board_id.clone(),
                    content: kanban_service::api::CreateColumnRequest {
                        id: None,
                        name: "TODO".to_string(),
                        wip_limit: None,
                        default_status: None,
                    },
                }))
                .await
                .unwrap(),
        );
        let second_column_id = second_column["id"].as_str().unwrap().to_string();

        let beta_a = text_payload(
            &seeded
                .server
                .tool_create_card(Parameters(CreateCardParams {
                    board: second_board_id.clone(),
                    column: second_column_id.clone(),
                    sprint: None,
                    content: kanban_service::api::CreateCardRequest {
                        id: None,
                        title: "Beta A".to_string(),
                        description: None,
                        priority: None,
                        due_date: None,
                        points: None,
                        sprint_id: None,
                    },
                }))
                .await
                .unwrap(),
        );
        let beta_a_id = beta_a["id"].as_str().unwrap().to_string();
        let beta_a_identifier = format!(
            "{}-{}",
            beta_a["prefix"].as_str().unwrap(),
            beta_a["card_number"].as_u64().unwrap()
        );

        let beta_b = text_payload(
            &seeded
                .server
                .tool_create_card(Parameters(CreateCardParams {
                    board: second_board_id.clone(),
                    column: second_column_id,
                    sprint: None,
                    content: kanban_service::api::CreateCardRequest {
                        id: None,
                        title: "Beta B".to_string(),
                        description: None,
                        priority: None,
                        due_date: None,
                        points: None,
                        sprint_id: None,
                    },
                }))
                .await
                .unwrap(),
        );
        let beta_b_id = beta_b["id"].as_str().unwrap().to_string();
        let beta_b_identifier = format!(
            "{}-{}",
            beta_b["prefix"].as_str().unwrap(),
            beta_b["card_number"].as_u64().unwrap()
        );

        seeded
            .server
            .tool_archive_board(Parameters(crate::requests::board::ArchiveBoardRequest {
                board: second_board_id,
            }))
            .await
            .unwrap();

        let set_response = text_payload(
            &seeded
                .server
                .tool_set_card_parent(Parameters(SetCardParentRequest {
                    parent: beta_a_identifier.clone(),
                    child: beta_b_identifier.clone(),
                }))
                .await
                .unwrap(),
        );
        assert_eq!(set_response["parent"], beta_a_id);
        assert_eq!(set_response["child"], beta_b_id);

        let children_after_set = text_payload(
            &seeded
                .server
                .tool_list_card_children(Parameters(ListCardChildrenRequest {
                    card: beta_a_identifier.clone(),
                    page: None,
                    page_size: None,
                }))
                .await
                .unwrap(),
        );
        let items_after_set = children_after_set["items"].as_array().unwrap();
        assert_eq!(items_after_set.len(), 1);
        assert_eq!(items_after_set[0]["id"], beta_b_id);

        seeded
            .server
            .tool_remove_card_parent(Parameters(RemoveCardParentRequest {
                parent: beta_a_identifier.clone(),
                child: beta_b_identifier,
            }))
            .await
            .unwrap();

        let children_after_remove = text_payload(
            &seeded
                .server
                .tool_list_card_children(Parameters(ListCardChildrenRequest {
                    card: beta_a_identifier,
                    page: None,
                    page_size: None,
                }))
                .await
                .unwrap(),
        );
        let items_after_remove = children_after_remove["items"].as_array().unwrap();
        assert!(items_after_remove.is_empty());
    }

    #[tokio::test]
    async fn test_set_and_remove_card_parent_on_an_archived_board_still_mutate_the_graph_on_json() {
        assert_set_and_remove_card_parent_on_an_archived_board_still_mutate_the_graph("test.json")
            .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_set_and_remove_card_parent_on_an_archived_board_still_mutate_the_graph_on_sqlite()
    {
        assert_set_and_remove_card_parent_on_an_archived_board_still_mutate_the_graph(
            "test.sqlite",
        )
        .await;
    }
}
