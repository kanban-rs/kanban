use crate::helpers::{
    card_board, core_err_to_mcp, kanban_err_to_mcp, locked_read, locked_write,
    parse_archived_selector, parse_datetime, parse_priority, parse_sort_field, parse_sort_order,
    parse_status, to_call_tool_result, to_call_tool_result_json, McpResolve,
};
use crate::requests::card::{
    ArchiveCardRequest, CreateCardParams, DeleteCardRequest, GetCardBranchNameRequest,
    GetCardGitCheckoutRequest, GetCardRequest, ListCardsRequest, MoveCardRequest,
    RestoreCardRequest, UpdateCardRequest,
};
use crate::KanbanMcpServer;
use kanban_core::resolve_page_params;
use kanban_domain::{CardListFilter, CardUpdate, FieldUpdate, KanbanOperations};
use kanban_service::api::CardResponse;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ErrorData as McpError},
    tool, tool_router,
};

#[tool_router(router = card_crud_router, vis = "pub(crate)")]
impl KanbanMcpServer {
    #[tool(description = "Create a new card in a column")]
    pub async fn tool_create_card(
        &self,
        Parameters(mut req): Parameters<CreateCardParams>,
    ) -> Result<CallToolResult, McpError> {
        let card = locked_write(&self.ctx, |ctx| {
            // Resolve the parent FKs (board/column name→id) and the optional
            // loose `sprint` name-or-id, threading the latter into the shared
            // content's typed `sprint_id` before conversion. Then funnel through
            // the Card factory: split the shared DTO into its optional client id
            // + content spec via `into_new_card(column_id)`, and
            // create-from-spec. The JSON edge projects via CardResponse.
            let board_id = ctx.mcp_resolve_board(&req.board)?;
            let column_id = ctx.mcp_resolve_column_in_board(&req.column, board_id)?;
            if let Some(raw) = req.sprint.as_deref() {
                req.content.sprint_id = Some(ctx.mcp_resolve_sprint_in_board(raw, board_id)?);
            }
            let (id, spec) = req
                .content
                .into_new_card(column_id)
                .map_err(kanban_err_to_mcp)?;
            ctx.create_card_from_spec(id, spec)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&CardResponse::from(&card))
    }

    #[tool(
        description = "List cards with optional filters, including an `archived` selector: 'exclude' (default, live only), 'only' (archived only), or 'include' (both). Returns CardSummary (title, status, priority — no description; archived items carry archived_at). Use card get for full details. Use page/page_size for pagination (default: page=1, page_size=50)."
    )]
    pub async fn tool_list_cards(
        &self,
        Parameters(req): Parameters<ListCardsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let status = req.status.as_deref().map(parse_status).transpose()?;
        let archived = req
            .archived
            .as_deref()
            .map(parse_archived_selector)
            .transpose()?
            .unwrap_or_default();
        let sort = req.sort.as_deref().map(parse_sort_field).transpose()?;
        let sort_order = req.order.as_deref().map(parse_sort_order).transpose()?;
        let (page, page_size) =
            resolve_page_params(req.page, req.page_size).map_err(core_err_to_mcp)?;
        let result = locked_read(&self.ctx, |ctx| {
            let board_id = match &req.board {
                Some(raw) => Some(ctx.mcp_resolve_board(raw)?),
                None => None,
            };
            let column_id = match &req.column {
                Some(raw) => Some(match board_id {
                    Some(bid) => ctx.mcp_resolve_column_in_board(raw, bid)?,
                    None => ctx.mcp_resolve_column_global(raw)?,
                }),
                None => None,
            };
            let sprint_id = match &req.sprint {
                Some(raw) => Some(match board_id {
                    Some(bid) => ctx.mcp_resolve_sprint_in_board(raw, bid)?,
                    None => ctx.mcp_resolve_sprint_global(raw)?,
                }),
                None => None,
            };
            let filter = CardListFilter {
                board_id,
                column_id,
                sprint_ids: sprint_id.map(|sid| std::iter::once(sid).collect()),
                status,
                sort,
                sort_order,
                archived,
                ..Default::default()
            };
            ctx.list_cards_paged(filter, page, page_size)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&result)
    }

    #[tool(
        description = "Get a specific card by UUID or identifier (e.g. KAN-5). Returns a single card for UUID or unambiguous identifier, or a list of all matching cards if the identifier is ambiguous."
    )]
    pub async fn tool_get_card(
        &self,
        Parameters(req): Parameters<GetCardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let guard = self.ctx.lock().await;
        if let Ok(uuid) = uuid::Uuid::parse_str(&req.card) {
            let card = guard.get_card(uuid).map_err(kanban_err_to_mcp)?;
            let response = match card.as_ref() {
                // Stamp the marker's `archived_at` so an archived card is not
                // returned looking live (get and list must agree).
                Some(c) => Some(CardResponse::with_archived_at(
                    c,
                    guard.card_archived_at(uuid).map_err(kanban_err_to_mcp)?,
                )),
                None => None,
            };
            return to_call_tool_result(&response);
        }
        let cards = guard
            .find_cards_by_identifier(&req.card)
            .map_err(kanban_err_to_mcp)?;
        match cards.as_slice() {
            [] => Err(McpError::invalid_params(
                format!("Card not found: '{}'", req.card),
                None,
            )),
            [card] => {
                let at = guard.card_archived_at(card.id).map_err(kanban_err_to_mcp)?;
                to_call_tool_result(&CardResponse::with_archived_at(card, at))
            }
            _ => {
                let mut responses: Vec<CardResponse> = Vec::with_capacity(cards.len());
                for c in &cards {
                    let at = guard.card_archived_at(c.id).map_err(kanban_err_to_mcp)?;
                    responses.push(CardResponse::with_archived_at(c, at));
                }
                to_call_tool_result(&responses)
            }
        }
    }

    #[tool(
        description = "Update a card's properties (title, description, priority, status, due_date, points)"
    )]
    pub async fn tool_update_card(
        &self,
        Parameters(req): Parameters<UpdateCardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let priority = req.priority.as_deref().map(parse_priority).transpose()?;
        let status = req.status.as_deref().map(parse_status).transpose()?;
        let due_date = if req.clear_due_date == Some(true) {
            FieldUpdate::Clear
        } else {
            match req.due_date {
                Some(ref d) => FieldUpdate::Set(parse_datetime(d)?),
                None => FieldUpdate::NoChange,
            }
        };
        let updates = CardUpdate {
            title: req.title,
            description: req
                .description
                .map(FieldUpdate::Set)
                .unwrap_or(FieldUpdate::NoChange),
            priority,
            status,
            position: None,
            column_id: None,
            points: req
                .points
                .map(FieldUpdate::Set)
                .unwrap_or(FieldUpdate::NoChange),
            due_date,
            sprint_id: FieldUpdate::NoChange,
        };
        let card = locked_write(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_card(&req.card)?;
            ctx.update_card(id, updates).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&CardResponse::from(&card))
    }

    #[tool(description = "Move a card to a different column on the same board")]
    pub async fn tool_move_card(
        &self,
        Parameters(req): Parameters<MoveCardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let card = locked_write(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_card(&req.card)?;
            let board_id = card_board(ctx, id)?;
            let column_id = ctx.mcp_resolve_column_in_board(&req.column, board_id)?;
            ctx.move_card(id, column_id, req.position)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&CardResponse::from(&card))
    }

    #[tool(description = "Archive a card (move to archive, can be restored later)")]
    pub async fn tool_archive_card(
        &self,
        Parameters(req): Parameters<ArchiveCardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let id = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let id = ctx.mcp_resolve_card(&req.card)?;
            ctx.archive_card(id).map_err(kanban_err_to_mcp)?;
            Ok(id)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"archived": id.to_string()}))
    }

    #[tool(description = "Restore an archived card")]
    pub async fn tool_restore_card(
        &self,
        Parameters(req): Parameters<RestoreCardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let card = locked_write(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_card(&req.card)?;
            let column_id = match req.column.as_deref() {
                Some(raw) => {
                    let board_id = card_board(ctx, id)?;
                    Some(ctx.mcp_resolve_column_in_board(raw, board_id)?)
                }
                None => None,
            };
            ctx.restore_card(id, column_id).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&CardResponse::from(&card))
    }

    #[tool(description = "Delete a card permanently")]
    pub async fn tool_delete_card(
        &self,
        Parameters(req): Parameters<DeleteCardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let id = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let id = ctx.mcp_resolve_card(&req.card)?;
            ctx.delete_card(id).map_err(kanban_err_to_mcp)?;
            Ok(id)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"deleted": id.to_string()}))
    }

    // Card Utilities

    #[tool(description = "Get the git branch name for a card")]
    pub async fn tool_get_card_branch_name(
        &self,
        Parameters(req): Parameters<GetCardBranchNameRequest>,
    ) -> Result<CallToolResult, McpError> {
        let branch_name = locked_read(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_card(&req.card)?;
            ctx.get_card_branch_name(id).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"branch_name": branch_name}))
    }

    #[tool(description = "Get the git checkout command for a card")]
    pub async fn tool_get_card_git_checkout(
        &self,
        Parameters(req): Parameters<GetCardGitCheckoutRequest>,
    ) -> Result<CallToolResult, McpError> {
        let command = locked_read(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_card(&req.card)?;
            ctx.get_card_git_checkout(id).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"command": command}))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::requests::board::CreateBoardRequest;
    use crate::requests::column::CreateColumnParams;
    use crate::McpServer;
    use kanban_backend::{KanbanBackend, KanbanBackendFactory};
    use kanban_core::AppConfig;
    use kanban_persistence_json::{JsonBackendFactory, JsonStoreFactory};
    use kanban_persistence_sqlite::{SqliteBackendFactory, SqliteStoreFactory};
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
        board_id: String,
        column_id: String,
        card_id: String,
        card_identifier: String,
    }

    async fn seeded_server(file_name: &str) -> Seeded {
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

        let card = text_payload(
            &server
                .tool_create_card(Parameters(CreateCardParams {
                    board: board_id.clone(),
                    column: column_id.clone(),
                    sprint: None,
                    content: kanban_service::api::CreateCardRequest {
                        id: None,
                        title: "Do the thing".to_string(),
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
        let card_id = card["id"].as_str().unwrap().to_string();
        let card_identifier = format!(
            "{}-{}",
            card["prefix"].as_str().unwrap(),
            card["card_number"].as_u64().unwrap()
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
            board_id,
            column_id,
            card_id,
            card_identifier,
        }
    }

    #[tokio::test]
    async fn test_card_crud_tools_resolve_names_from_the_model_not_the_backend_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();

        let updated = text_payload(
            &seeded
                .server
                .tool_update_card(Parameters(UpdateCardRequest {
                    card: seeded.card_id.clone(),
                    title: Some("Renamed".into()),
                    description: None,
                    priority: None,
                    status: None,
                    due_date: None,
                    clear_due_date: None,
                    points: None,
                }))
                .await
                .unwrap(),
        );
        assert_eq!(updated["title"], "Renamed");
        assert_eq!(seeded.handle.op_count("list_all_cards"), 0);

        seeded.handle.clear_ops();
        text_payload(
            &seeded
                .server
                .tool_create_card(Parameters(CreateCardParams {
                    board: "Alpha".to_string(),
                    column: "TODO".to_string(),
                    sprint: None,
                    content: kanban_service::api::CreateCardRequest {
                        id: None,
                        title: "Another".to_string(),
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
        assert!(seeded.handle.op_count("list_boards") >= 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_card_crud_tools_resolve_names_from_the_model_not_the_backend_on_sqlite() {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();

        let updated = text_payload(
            &seeded
                .server
                .tool_update_card(Parameters(UpdateCardRequest {
                    card: seeded.card_id.clone(),
                    title: Some("Renamed".into()),
                    description: None,
                    priority: None,
                    status: None,
                    due_date: None,
                    clear_due_date: None,
                    points: None,
                }))
                .await
                .unwrap(),
        );
        assert_eq!(updated["title"], "Renamed");
        assert_eq!(seeded.handle.op_count("list_all_cards"), 0);

        seeded.handle.clear_ops();
        text_payload(
            &seeded
                .server
                .tool_create_card(Parameters(CreateCardParams {
                    board: "Alpha".to_string(),
                    column: "TODO".to_string(),
                    sprint: None,
                    content: kanban_service::api::CreateCardRequest {
                        id: None,
                        title: "Another".to_string(),
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
        assert!(seeded.handle.op_count("list_boards") >= 1);
    }

    #[tokio::test]
    async fn test_card_crud_tool_with_an_unloadable_collection_errors_instead_of_reporting_not_found_on_json(
    ) {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_all_cards");

        let err = seeded
            .server
            .tool_archive_card(Parameters(ArchiveCardRequest {
                card: seeded.card_identifier.clone(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("card list"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_card_crud_tool_with_an_unloadable_collection_errors_instead_of_reporting_not_found_on_sqlite(
    ) {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_all_cards");

        let err = seeded
            .server
            .tool_archive_card(Parameters(ArchiveCardRequest {
                card: seeded.card_identifier.clone(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("card list"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test]
    async fn test_list_cards_archived_selector_reads_the_board_scoped_archived_tier_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded
            .server
            .tool_archive_card(Parameters(ArchiveCardRequest {
                card: seeded.card_id.clone(),
            }))
            .await
            .unwrap();
        seeded.handle.clear_ops();

        let result = text_payload(
            &seeded
                .server
                .tool_list_cards(Parameters(ListCardsRequest {
                    board: Some("Alpha".to_string()),
                    column: None,
                    sprint: None,
                    status: None,
                    archived: Some("only".to_string()),
                    sort: None,
                    order: None,
                    page: None,
                    page_size: None,
                }))
                .await
                .unwrap(),
        );

        assert_eq!(result["items"].as_array().unwrap().len(), 1);
        assert!(seeded.handle.op_count("list_archived_cards_by_board") >= 1);
        assert!(seeded.handle.op_count("list_boards") >= 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_list_cards_archived_selector_reads_the_board_scoped_archived_tier_on_sqlite() {
        let seeded = seeded_server("test.sqlite").await;
        seeded
            .server
            .tool_archive_card(Parameters(ArchiveCardRequest {
                card: seeded.card_id.clone(),
            }))
            .await
            .unwrap();
        seeded.handle.clear_ops();

        let result = text_payload(
            &seeded
                .server
                .tool_list_cards(Parameters(ListCardsRequest {
                    board: Some("Alpha".to_string()),
                    column: None,
                    sprint: None,
                    status: None,
                    archived: Some("only".to_string()),
                    sort: None,
                    order: None,
                    page: None,
                    page_size: None,
                }))
                .await
                .unwrap(),
        );

        assert_eq!(result["items"].as_array().unwrap().len(), 1);
        assert!(seeded.handle.op_count("list_archived_cards_by_board") >= 1);
        assert!(seeded.handle.op_count("list_boards") >= 1);
    }

    #[tokio::test]
    async fn test_list_cards_with_an_unloadable_board_list_errors_naming_the_collection() {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_boards");

        let err = seeded
            .server
            .tool_list_cards(Parameters(ListCardsRequest {
                board: Some("Alpha".to_string()),
                column: None,
                sprint: None,
                status: None,
                archived: Some("only".to_string()),
                sort: None,
                order: None,
                page: None,
                page_size: None,
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("board list"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test]
    async fn test_card_crud_tool_result_json_is_unchanged() {
        let seeded = seeded_server("test.json").await;

        let response = text_payload(
            &seeded
                .server
                .tool_update_card(Parameters(UpdateCardRequest {
                    card: seeded.card_identifier.clone(),
                    title: Some("Renamed again".into()),
                    description: None,
                    priority: None,
                    status: None,
                    due_date: None,
                    clear_due_date: None,
                    points: None,
                }))
                .await
                .unwrap(),
        );

        assert_eq!(response["title"], "Renamed again");
        assert!(response["id"].is_string());
        assert!(response["prefix"].is_string());
        assert!(response["card_number"].is_number());
        let _ = (&seeded.board_id, &seeded.column_id);
    }
}
