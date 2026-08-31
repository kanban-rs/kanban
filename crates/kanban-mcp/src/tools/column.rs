use crate::helpers::model_read::{resolve_board, resolve_column_global};
use crate::helpers::{
    core_err_to_mcp, kanban_err_to_mcp, locked_read, locked_write, to_call_tool_result,
    to_call_tool_result_json,
};
use crate::requests::column::{
    CreateColumnParams, DeleteColumnRequest, GetColumnRequest, ListColumnsRequest,
    ReorderColumnRequest, UpdateColumnRequest,
};
use crate::scope::{Ref, ToolScope, ToolScoped};
use crate::KanbanMcpServer;
use kanban_core::{resolve_page_params, PaginatedList};
use kanban_domain::{ColumnUpdate, FieldUpdate, KanbanOperations};
use kanban_service::api::ColumnResponse;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ErrorData as McpError},
    tool, tool_router,
};

impl ToolScoped for CreateColumnParams {
    fn scope(&self) -> ToolScope {
        ToolScope {
            board: Some(Ref::of(&self.board)),
            ..Default::default()
        }
    }
}

impl ToolScoped for ListColumnsRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            board: Some(Ref::of(&self.board)),
            ..Default::default()
        }
    }
}

impl ToolScoped for GetColumnRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            column: Some(Ref::of(&self.column)),
            ..Default::default()
        }
    }
}

impl ToolScoped for UpdateColumnRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            column: Some(Ref::of(&self.column)),
            ..Default::default()
        }
    }
}

impl ToolScoped for DeleteColumnRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            column: Some(Ref::of(&self.column)),
            ..Default::default()
        }
    }
}

impl ToolScoped for ReorderColumnRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            column: Some(Ref::of(&self.column)),
            ..Default::default()
        }
    }
}

#[tool_router(router = column_router, vis = "pub(crate)")]
impl KanbanMcpServer {
    #[tool(description = "Create a new column in a board")]
    pub async fn tool_create_column(
        &self,
        Parameters(req): Parameters<CreateColumnParams>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let column = locked_write(&self.ctx, |ctx| {
            // Resolve the single parent FK (board name→id) then funnel through
            // the Column factory: split the shared DTO into its optional client
            // id + content spec (server assigns the append position), and
            // create-from-spec. The JSON edge projects via ColumnResponse.
            let model = ctx.model_for(&scope);
            let board_id = resolve_board(&model, &req.board)?;
            let (id, spec) = req
                .content
                .into_new_column(board_id)
                .map_err(kanban_err_to_mcp)?;
            ctx.create_column_from_spec(id, spec)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&ColumnResponse::from(&column))
    }

    #[tool(
        description = "List all columns in a board. Use page/page_size for pagination (default: page=1, page_size=50)."
    )]
    pub async fn tool_list_columns(
        &self,
        Parameters(req): Parameters<ListColumnsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let columns = locked_read(&self.ctx, |ctx| {
            let model = ctx.model_for(&scope);
            let board_id = resolve_board(&model, &req.board)?;
            ctx.list_columns(board_id).map_err(kanban_err_to_mcp)
        })
        .await?;
        let responses: Vec<ColumnResponse> = columns.iter().map(ColumnResponse::from).collect();
        let (page, page_size) =
            resolve_page_params(req.page, req.page_size).map_err(core_err_to_mcp)?;
        let paged = PaginatedList::paginate(responses, page, page_size).map_err(core_err_to_mcp)?;
        to_call_tool_result(&paged)
    }

    #[tool(description = "Get a specific column by UUID or name (searched across all boards)")]
    pub async fn tool_get_column(
        &self,
        Parameters(req): Parameters<GetColumnRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let column = locked_read(&self.ctx, |ctx| {
            let model = ctx.model_for(&scope);
            let id = resolve_column_global(&model, &req.column)?;
            ctx.get_column(id).map_err(kanban_err_to_mcp)
        })
        .await?;
        let response = column.as_ref().map(ColumnResponse::from);
        to_call_tool_result(&response)
    }

    #[tool(
        description = "Update a column's properties (name, position, wip_limit, default_status)"
    )]
    pub async fn tool_update_column(
        &self,
        Parameters(req): Parameters<UpdateColumnRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let updates = ColumnUpdate {
            name: req.name,
            position: req.position,
            wip_limit: if req.clear_wip_limit == Some(true) {
                FieldUpdate::Clear
            } else {
                req.wip_limit
                    .map(|w| FieldUpdate::Set(w as i32))
                    .unwrap_or(FieldUpdate::NoChange)
            },
            default_status: if req.clear_default_status == Some(true) {
                Some(None)
            } else {
                req.default_status.map(|s| Some(s.into()))
            },
        };
        let column = locked_write(&self.ctx, |ctx| {
            let model = ctx.model_for(&scope);
            let id = resolve_column_global(&model, &req.column)?;
            ctx.update_column(id, updates).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&ColumnResponse::from(&column))
    }

    #[tool(description = "Delete a column and all its cards")]
    pub async fn tool_delete_column(
        &self,
        Parameters(req): Parameters<DeleteColumnRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let id = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let model = ctx.model_for(&scope);
            let id = resolve_column_global(&model, &req.column)?;
            ctx.delete_column(id).map_err(kanban_err_to_mcp)?;
            Ok(id)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"deleted": id.to_string()}))
    }

    #[tool(description = "Reorder a column to a new position")]
    pub async fn tool_reorder_column(
        &self,
        Parameters(req): Parameters<ReorderColumnRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let column = locked_write(&self.ctx, |ctx| {
            let model = ctx.model_for(&scope);
            let id = resolve_column_global(&model, &req.column)?;
            ctx.reorder_column(id, req.position)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&ColumnResponse::from(&column))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::requests::board::CreateBoardRequest;
    use crate::McpServer;
    use kanban_backend::{KanbanBackend, KanbanBackendFactory};
    use kanban_core::AppConfig;
    use kanban_domain::Model;
    use kanban_persistence_json::{JsonBackendFactory, JsonStoreFactory};
    use kanban_persistence_sqlite::{SqliteBackendFactory, SqliteStoreFactory};
    use kanban_service::test_helpers::FaultInjectingBackend;
    use kanban_service::FetchPlan;
    use rmcp::model::ErrorCode;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;
    use uuid::Uuid;

    #[test]
    fn test_column_request_scopes_map_names_to_the_global_column_or_board_list_and_never_request_board_columns(
    ) {
        let named_board = CreateColumnParams {
            board: "Alpha".into(),
            content: kanban_service::api::CreateColumnRequest {
                id: None,
                name: "TODO".into(),
                wip_limit: None,
                default_status: None,
            },
        };
        let round = named_board.scope().next_round(&Model::default());
        assert!(round.board_list);
        assert!(!round.column_list);
        assert!(round.columns_by_board.is_empty());
        assert!(!named_board.scope().wants_board_columns);

        let id_board = CreateColumnParams {
            board: Uuid::new_v4().to_string(),
            content: kanban_service::api::CreateColumnRequest {
                id: None,
                name: "TODO".into(),
                wip_limit: None,
                default_status: None,
            },
        };
        assert!(id_board.scope().next_round(&Model::default()).is_empty());

        let named_list = ListColumnsRequest {
            board: "Alpha".into(),
            page: None,
            page_size: None,
        };
        let round = named_list.scope().next_round(&Model::default());
        assert!(round.board_list);
        assert!(!round.column_list);
        assert!(!named_list.scope().wants_board_columns);

        let named_get = GetColumnRequest {
            column: "TODO".into(),
        };
        let round = named_get.scope().next_round(&Model::default());
        assert!(round.column_list);
        assert!(!round.board_list);
        assert!(round.columns_by_board.is_empty());
        assert!(!named_get.scope().wants_board_columns);

        let id_get = GetColumnRequest {
            column: Uuid::new_v4().to_string(),
        };
        assert!(id_get.scope().next_round(&Model::default()).is_empty());

        let named_update = UpdateColumnRequest {
            column: "TODO".into(),
            name: None,
            position: None,
            wip_limit: None,
            clear_wip_limit: None,
            default_status: None,
            clear_default_status: None,
        };
        let round = named_update.scope().next_round(&Model::default());
        assert!(round.column_list);
        assert!(!round.board_list);
        assert!(!named_update.scope().wants_board_columns);

        let named_delete = DeleteColumnRequest {
            column: "TODO".into(),
        };
        let round = named_delete.scope().next_round(&Model::default());
        assert!(round.column_list);
        assert!(!named_delete.scope().wants_board_columns);

        let named_reorder = ReorderColumnRequest {
            column: "TODO".into(),
            position: 1,
        };
        let round = named_reorder.scope().next_round(&Model::default());
        assert!(round.column_list);
        assert!(!named_reorder.scope().wants_board_columns);
    }

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

    async fn seeded_server(
        file_name: &str,
    ) -> (KanbanMcpServer, TempDir, Arc<FaultInjectingBackend>) {
        let sqlite_handle = Arc::new(Mutex::new(None));
        let json_handle = Arc::new(Mutex::new(None));
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(file_name);

        let server = McpServer::default()
            .register_backend(
                Box::new(SqliteStoreFactory),
                Box::new(RecordingFactory {
                    inner: Box::new(SqliteBackendFactory),
                    handle: Arc::clone(&sqlite_handle),
                }),
            )
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

        server
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
            .unwrap();

        server
            .tool_create_column(Parameters(CreateColumnParams {
                board: "Alpha".into(),
                content: kanban_service::api::CreateColumnRequest {
                    id: None,
                    name: "TODO".into(),
                    wip_limit: None,
                    default_status: None,
                },
            }))
            .await
            .unwrap();

        let handle = sqlite_handle
            .lock()
            .unwrap()
            .clone()
            .or_else(|| json_handle.lock().unwrap().clone())
            .expect("a backend must have been created");
        (server, dir, handle)
    }

    #[tokio::test]
    async fn test_get_column_with_an_unloadable_column_list_errors_naming_the_collection_on_json() {
        let (server, _dir, handle) = seeded_server("test.json").await;
        handle.clear_ops();
        handle.fail("list_all_columns");

        let err = server
            .tool_get_column(Parameters(GetColumnRequest {
                column: "TODO".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("column list"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_column_with_an_unloadable_column_list_errors_naming_the_collection_on_sqlite()
    {
        let (server, _dir, handle) = seeded_server("test.sqlite").await;
        handle.clear_ops();
        handle.fail("list_all_columns");

        let err = server
            .tool_get_column(Parameters(GetColumnRequest {
                column: "TODO".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("column list"));
    }

    #[tokio::test]
    async fn test_list_columns_with_an_unloadable_board_list_errors_naming_the_collection_on_json()
    {
        let (server, _dir, handle) = seeded_server("test.json").await;
        handle.clear_ops();
        handle.fail("list_boards");

        let err = server
            .tool_list_columns(Parameters(ListColumnsRequest {
                board: "Alpha".into(),
                page: None,
                page_size: None,
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("board list"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_list_columns_with_an_unloadable_board_list_errors_naming_the_collection_on_sqlite(
    ) {
        let (server, _dir, handle) = seeded_server("test.sqlite").await;
        handle.clear_ops();
        handle.fail("list_boards");

        let err = server
            .tool_list_columns(Parameters(ListColumnsRequest {
                board: "Alpha".into(),
                page: None,
                page_size: None,
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("board list"));
    }

    #[tokio::test]
    async fn test_get_column_by_name_reads_the_column_list_exactly_once_on_json() {
        let (server, _dir, handle) = seeded_server("test.json").await;
        handle.clear_ops();

        server
            .tool_get_column(Parameters(GetColumnRequest {
                column: "TODO".into(),
            }))
            .await
            .unwrap();

        assert_eq!(handle.op_count("list_all_columns"), 1);
        assert_eq!(handle.op_count("list_boards"), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_column_by_name_reads_the_column_list_exactly_once_on_sqlite() {
        let (server, _dir, handle) = seeded_server("test.sqlite").await;
        handle.clear_ops();

        server
            .tool_get_column(Parameters(GetColumnRequest {
                column: "TODO".into(),
            }))
            .await
            .unwrap();

        assert_eq!(handle.op_count("list_all_columns"), 1);
        assert_eq!(handle.op_count("list_boards"), 0);
    }

    #[tokio::test]
    async fn test_create_column_by_board_name_resolves_via_the_model_before_any_write_op() {
        let (server, _dir, handle) = seeded_server("test.json").await;
        handle.clear_ops();

        server
            .tool_create_column(Parameters(CreateColumnParams {
                board: "Alpha".into(),
                content: kanban_service::api::CreateColumnRequest {
                    id: None,
                    name: "Doing".into(),
                    wip_limit: None,
                    default_status: None,
                },
            }))
            .await
            .unwrap();

        let ops = handle.ops();
        let write_op = ops
            .iter()
            .position(|op| op.method == "get_board")
            .expect("resolution reads a board before the write path touches get_board");
        let reads_before_write: Vec<&str> = ops[..write_op].iter().map(|op| op.method).collect();
        assert_eq!(reads_before_write, vec!["list_boards"]);
    }

    #[tokio::test]
    async fn test_get_column_result_json_is_unchanged() {
        let (server, _dir, _handle) = seeded_server("test.json").await;

        let response = text_payload(
            &server
                .tool_get_column(Parameters(GetColumnRequest {
                    column: "TODO".into(),
                }))
                .await
                .unwrap(),
        );

        assert_eq!(response["name"], "TODO");
        assert!(response["id"].is_string());
        assert!(response["board_id"].is_string());
        assert!(response["position"].is_number());
    }
}
