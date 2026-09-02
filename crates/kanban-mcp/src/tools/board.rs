use crate::helpers::model_read::resolve_board;
use crate::helpers::{
    core_err_to_mcp, kanban_err_to_mcp, locked_read, locked_write, parse_archived_selector,
    parse_board_sort_field, parse_sort_field, parse_sort_order, to_call_tool_result,
    to_call_tool_result_json,
};
use crate::requests::board::{
    ArchiveBoardRequest, CreateBoardParams, DeleteArchivedBoardRequest, DeleteBoardRequest,
    GetBoardRequest, ListBoardsRequest, RestoreBoardRequest, SetBoardSortRequest,
    UpdateBoardRequest,
};
use crate::scope::{Ref, ToolScope, ToolScoped};
use crate::KanbanMcpServer;
use chrono::{DateTime, Utc};
use kanban_core::{resolve_page_params, PaginatedList};
use kanban_domain::{BoardListFilter, BoardUpdate, FieldUpdate, KanbanOperations};
use kanban_service::api::BoardResponse;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ErrorData as McpError},
    tool, tool_router,
};
use std::collections::HashMap;
use uuid::Uuid;

impl ToolScoped for GetBoardRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            board: Some(Ref::of(&self.board)),
            ..Default::default()
        }
    }
}

impl ToolScoped for UpdateBoardRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            board: Some(Ref::of(&self.board)),
            ..Default::default()
        }
    }
}

impl ToolScoped for DeleteBoardRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            board: Some(Ref::of(&self.board)),
            ..Default::default()
        }
    }
}

impl ToolScoped for ArchiveBoardRequest {
    fn scope(&self) -> ToolScope {
        ToolScope {
            board: Some(Ref::of(&self.board)),
            ..Default::default()
        }
    }
}

#[tool_router(router = board_router, vis = "pub(crate)")]
impl KanbanMcpServer {
    #[tool(
        description = "Create a new kanban board. Pass with_default_columns=true to seed the standard template columns (TODO, Doing, Complete) with default statuses todo, in_progress, done."
    )]
    pub async fn tool_create_board(
        &self,
        Parameters(req): Parameters<CreateBoardParams>,
    ) -> Result<CallToolResult, McpError> {
        // Funnel through the Board factory: split the shared DTO into its
        // optional client id + content spec, then create-from-spec. The JSON
        // edge projects the resulting domain Board via BoardResponse.
        let (id, spec) = req.content.into_new_board();
        let seed_columns = req.with_default_columns.unwrap_or(false);
        let board = locked_write(&self.ctx, |ctx| {
            let (board, _) = ctx
                .create_board_from_spec(id, spec)
                .map_err(kanban_err_to_mcp)?;
            if seed_columns {
                for (name, default_status) in kanban_domain::DEFAULT_TEMPLATE_COLUMNS {
                    ctx.create_column_from_spec(
                        None,
                        kanban_domain::NewColumn {
                            board_id: board.id,
                            name: name.to_string(),
                            wip_limit: None,
                            default_status,
                        },
                    )
                    .map_err(kanban_err_to_mcp)?;
                }
            }
            Ok::<_, McpError>(board)
        })
        .await?;
        to_call_tool_result(&BoardResponse::from(&board))
    }

    #[tool(
        description = "List kanban boards with an `archived` selector: 'exclude' (default, live only), 'only' (archived only), or 'include' (both). Archived boards carry an archived_at timestamp. Use page/page_size for pagination (default: page=1, page_size=50)."
    )]
    pub async fn tool_list_boards(
        &self,
        Parameters(req): Parameters<ListBoardsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let archived = req
            .archived
            .as_deref()
            .map(parse_archived_selector)
            .transpose()?
            .unwrap_or_default();
        let sort = req
            .sort
            .as_deref()
            .map(parse_board_sort_field)
            .transpose()?;
        let sort_order = req.order.as_deref().map(parse_sort_order).transpose()?;
        // One gather path: the service filter yields the live/archived/both head
        // set (mirroring `filter_cards`). The archive markers only supply the
        // per-board `archived_at`, so we decorate the filtered heads by looking
        // each up in a marker map — a live head stays `None` (key skipped on the
        // wire), an archived head is stamped `Some`.
        let responses = locked_read(&self.ctx, |ctx| -> Result<Vec<BoardResponse>, McpError> {
            let archived_at: HashMap<Uuid, DateTime<Utc>> = ctx
                .list_archived_boards()
                .map_err(kanban_err_to_mcp)?
                .into_iter()
                .map(|m| (m.entity_id, m.metadata.archived_at))
                .collect();
            let filter = BoardListFilter {
                archived,
                sort,
                sort_order,
                search: None,
            };
            Ok(ctx
                .list_boards_filtered(filter)
                .map_err(kanban_err_to_mcp)?
                .iter()
                .map(|board| {
                    BoardResponse::with_archived_at(board, archived_at.get(&board.id).copied())
                })
                .collect())
        })
        .await?;
        let (page, page_size) =
            resolve_page_params(req.page, req.page_size).map_err(core_err_to_mcp)?;
        let paged = PaginatedList::paginate(responses, page, page_size).map_err(core_err_to_mcp)?;
        to_call_tool_result(&paged)
    }

    #[tool(description = "Get a specific board by UUID or name")]
    pub async fn tool_get_board(
        &self,
        Parameters(req): Parameters<GetBoardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let response = locked_read(&self.ctx, |ctx| {
            let model = ctx.model_for(&scope);
            let id = resolve_board(&model, &req.board)?;
            let board = ctx.get_board(id).map_err(kanban_err_to_mcp)?;
            // Stamp the marker's `archived_at` so an archived board is not
            // returned looking live.
            Ok::<_, McpError>(match board.as_ref() {
                Some(b) => Some(BoardResponse::with_archived_at(
                    b,
                    ctx.board_archived_at(id).map_err(kanban_err_to_mcp)?,
                )),
                None => None,
            })
        })
        .await?;
        to_call_tool_result(&response)
    }

    #[tool(
        description = "Update a board's properties (name, description, sprint_prefix, card_prefix, task_sort_field, task_sort_order)"
    )]
    pub async fn tool_update_board(
        &self,
        Parameters(req): Parameters<UpdateBoardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let task_sort_field = req
            .task_sort_field
            .as_deref()
            .map(parse_sort_field)
            .transpose()?;
        let task_sort_order = req
            .task_sort_order
            .as_deref()
            .map(parse_sort_order)
            .transpose()?;
        let scope = req.scope();
        let board = locked_write(&self.ctx, |ctx| {
            let model = ctx.model_for(&scope);
            let id = resolve_board(&model, &req.board)?;
            let updates = BoardUpdate {
                name: req.name,
                description: req
                    .description
                    .map(FieldUpdate::Set)
                    .unwrap_or(FieldUpdate::NoChange),
                sprint_prefix: req
                    .sprint_prefix
                    .map(FieldUpdate::Set)
                    .unwrap_or(FieldUpdate::NoChange),
                card_prefix: req
                    .card_prefix
                    .map(FieldUpdate::Set)
                    .unwrap_or(FieldUpdate::NoChange),
                task_sort_field,
                task_sort_order,
                ..Default::default()
            };
            ctx.mutate(|c| c.update_board_impl(id, updates))
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&BoardResponse::from(&board))
    }

    #[tool(
        description = "Set the default board-list sort persisted in the app config (board_sort_field / board_sort_order). Sort field valid: position, name, created_at, archived_at. Order valid: asc, desc. Either may be omitted to leave that dimension unchanged. Subsequent list_boards calls without an explicit sort/order use this default."
    )]
    pub async fn tool_set_board_sort(
        &self,
        Parameters(req): Parameters<SetBoardSortRequest>,
    ) -> Result<CallToolResult, McpError> {
        // Parse the raw strings at the tool boundary via the canonical domain
        // `FromStr` (R1). An invalid value is rejected here, before any config
        // write. Either dimension may be omitted to leave it unchanged; the
        // context resolves the omitted half from the current config.
        let field = req
            .sort
            .as_deref()
            .map(parse_board_sort_field)
            .transpose()?;
        let order = req.order.as_deref().map(parse_sort_order).transpose()?;
        let (resolved_field, resolved_order) = locked_write(&self.ctx, |ctx| {
            ctx.set_board_sort(field, order).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({
            "board_sort_field": resolved_field.to_string(),
            "board_sort_order": resolved_order.to_string(),
        }))
    }

    #[tool(description = "Delete a board and all its columns, cards, and sprints")]
    pub async fn tool_delete_board(
        &self,
        Parameters(req): Parameters<DeleteBoardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let id = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let model = ctx.model_for(&scope);
            let id = resolve_board(&model, &req.board)?;
            ctx.mutate_unit(|c| c.delete_board_impl(id))
                .map_err(kanban_err_to_mcp)?;
            Ok(id)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"deleted": id.to_string()}))
    }

    #[tool(description = "Archive a board by UUID or name (hides it from the live list)")]
    pub async fn tool_archive_board(
        &self,
        Parameters(req): Parameters<ArchiveBoardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let id = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let model = ctx.model_for(&scope);
            let id = resolve_board(&model, &req.board)?;
            ctx.mutate_unit(|c| c.archive_board_impl(id))
                .map_err(kanban_err_to_mcp)?;
            Ok(id)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"archived": id.to_string()}))
    }

    #[tool(description = "Restore an archived board by UUID or name (returns it to the live list)")]
    pub async fn tool_restore_board(
        &self,
        Parameters(req): Parameters<RestoreBoardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let board = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            // Resolve against the ARCHIVED-only set: an archived board is not in
            // the live list, and scoping to archived-only guarantees a same-named
            // live board is never hit (KAN-894 data-loss guard).
            let id = resolve_archived_board(ctx, &req.board)?;
            ctx.mutate_unit(|c| c.restore_board_impl(id))
                .map_err(kanban_err_to_mcp)?;
            ctx.get_board(id).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&board.as_ref().map(BoardResponse::from))
    }

    #[tool(
        description = "Permanently delete an archived board by UUID or name, along with its columns, cards, and sprints"
    )]
    pub async fn tool_delete_archived_board(
        &self,
        Parameters(req): Parameters<DeleteArchivedBoardRequest>,
    ) -> Result<CallToolResult, McpError> {
        // A board is just a board: delete works on an archived one because
        // `get_board` is unfiltered. Resolve from either view.
        let id = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let id = resolve_archived_board(ctx, &req.board)?;
            ctx.mutate_unit(|c| c.delete_board_impl(id))
                .map_err(kanban_err_to_mcp)?;
            Ok(id)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"deleted": id.to_string()}))
    }
}

/// Resolve a board id from the ARCHIVED collection ONLY (for the `-archived`
/// tools). A UUID passes straight through; a name is matched against the
/// archived-only head set via `find_boards_by_name`. Scoping the candidate set
/// to `ArchivedFilter::ArchivedOnly` structurally guarantees a same-named live
/// board can never be hit by an archived-scoped command (KAN-894 data-loss).
fn resolve_archived_board(ctx: &crate::context::McpContext, raw: &str) -> Result<Uuid, McpError> {
    if let Ok(uuid) = Uuid::parse_str(raw) {
        return Ok(uuid);
    }
    let heads = ctx
        .list_boards_filtered(BoardListFilter {
            archived: kanban_domain::ArchivedFilter::ArchivedOnly,
            ..Default::default()
        })
        .map_err(kanban_err_to_mcp)?;
    let matches: Vec<Uuid> = kanban_domain::find_boards_by_name(raw, &heads)
        .iter()
        .map(|b| b.id)
        .collect();
    match matches.as_slice() {
        [id] => Ok(*id),
        [] => Err(McpError::invalid_params(
            format!("No archived board named: '{raw}'"),
            None,
        )),
        _ => Err(McpError::invalid_params(
            format!("Ambiguous archived board name: '{raw}'"),
            None,
        )),
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
    use kanban_persistence_sqlite::SqliteBackendFactory;
    use kanban_service::test_helpers::FaultInjectingBackend;
    use kanban_service::FetchPlan;
    use rmcp::model::ErrorCode;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    #[test]
    fn test_board_request_scopes_map_a_name_to_the_board_list_and_a_uuid_to_no_tier() {
        let name = GetBoardRequest {
            board: "Alpha".into(),
        };
        assert!(name.scope().next_round(&Model::default()).board_list);

        let id = GetBoardRequest {
            board: Uuid::new_v4().to_string(),
        };
        assert!(id.scope().next_round(&Model::default()).is_empty());

        let name = UpdateBoardRequest {
            board: "Alpha".into(),
            name: None,
            description: None,
            sprint_prefix: None,
            card_prefix: None,
            task_sort_field: None,
            task_sort_order: None,
        };
        assert!(name.scope().next_round(&Model::default()).board_list);

        let name = DeleteBoardRequest {
            board: "Alpha".into(),
        };
        assert!(name.scope().next_round(&Model::default()).board_list);

        let name = ArchiveBoardRequest {
            board: "Alpha".into(),
        };
        assert!(name.scope().next_round(&Model::default()).board_list);
    }

    #[test]
    fn test_get_board_scope_does_not_request_the_board_list_for_a_uuid() {
        let req = GetBoardRequest {
            board: Uuid::new_v4().to_string(),
        };
        let scope = req.scope();
        assert!(scope.next_round(&Model::default()).is_empty());
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

        server
            .tool_create_board(Parameters(CreateBoardParams {
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

        let handle = sqlite_handle
            .lock()
            .unwrap()
            .clone()
            .or_else(|| json_handle.lock().unwrap().clone())
            .expect("a backend must have been created");
        (server, dir, handle)
    }

    #[tokio::test]
    async fn test_get_board_with_an_unloadable_board_list_errors_naming_the_collection_on_json() {
        let (server, _dir, handle) = seeded_server("test.json").await;
        handle.clear_ops();
        handle.fail("list_boards");

        let err = server
            .tool_get_board(Parameters(GetBoardRequest {
                board: "Alpha".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("board list"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_board_with_an_unloadable_board_list_errors_naming_the_collection_on_sqlite() {
        let (server, _dir, handle) = seeded_server("test.sqlite").await;
        handle.clear_ops();
        handle.fail("list_boards");

        let err = server
            .tool_get_board(Parameters(GetBoardRequest {
                board: "Alpha".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("board list"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test]
    async fn test_archive_board_with_an_unloadable_board_list_errors_naming_the_collection_on_json()
    {
        let (server, _dir, handle) = seeded_server("test.json").await;
        handle.clear_ops();
        handle.fail("list_boards");

        let err = server
            .tool_archive_board(Parameters(ArchiveBoardRequest {
                board: "Alpha".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("board list"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_archive_board_with_an_unloadable_board_list_errors_naming_the_collection_on_sqlite(
    ) {
        let (server, _dir, handle) = seeded_server("test.sqlite").await;
        handle.clear_ops();
        handle.fail("list_boards");

        let err = server
            .tool_archive_board(Parameters(ArchiveBoardRequest {
                board: "Alpha".into(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("board list"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test]
    async fn test_get_board_by_uuid_reads_the_board_by_id_not_the_live_board_list_on_json() {
        let (server, _dir, handle) = seeded_server("test.json").await;
        let created = text_payload(
            &server
                .tool_list_boards(Parameters(ListBoardsRequest {
                    archived: None,
                    sort: None,
                    order: None,
                    page: None,
                    page_size: None,
                }))
                .await
                .unwrap(),
        );
        let board_id = created["items"][0]["id"].as_str().unwrap().to_string();

        server
            .tool_archive_board(Parameters(ArchiveBoardRequest {
                board: board_id.clone(),
            }))
            .await
            .unwrap();
        handle.clear_ops();

        let response = text_payload(
            &server
                .tool_get_board(Parameters(GetBoardRequest {
                    board: board_id.clone(),
                }))
                .await
                .unwrap(),
        );

        assert!(!response["archived_at"].is_null());
        assert_eq!(handle.op_count("list_boards"), 0);
        assert!(handle.op_count("get_board") >= 1);
    }

    #[tokio::test]
    async fn test_update_board_result_json_is_unchanged() {
        let (server, _dir, _handle) = seeded_server("test.json").await;

        let response = text_payload(
            &server
                .tool_update_board(Parameters(UpdateBoardRequest {
                    board: "Alpha".into(),
                    name: Some("Beta".into()),
                    description: None,
                    sprint_prefix: None,
                    card_prefix: None,
                    task_sort_field: None,
                    task_sort_order: None,
                }))
                .await
                .unwrap(),
        );

        assert_eq!(response["name"], "Beta");
        assert!(response["id"].is_string());
    }
}
