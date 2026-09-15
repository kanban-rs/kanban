use crate::helpers::model_read::{
    resolve_board, resolve_column_global, resolve_column_in_board, resolve_sprint_global,
    resolve_sprint_in_board,
};
use crate::helpers::{
    board_head, card_board, core_err_to_mcp, kanban_err_to_mcp, locked_read, locked_write,
    parse_archived_selector, parse_datetime, parse_priority, parse_sort_field, parse_sort_order,
    parse_status, to_call_tool_result, to_call_tool_result_json,
};
use crate::requests::card::{
    ArchiveCardRequest, CreateCardParams, DeleteCardRequest, GetCardBranchNameRequest,
    GetCardGitCheckoutRequest, GetCardRequest, ListCardsRequest, MoveCardRequest,
    RestoreCardRequest, UpdateCardRequest,
};
use crate::scope::{Ref, ToolScope, ToolScoped};
use crate::KanbanMcpServer;
use kanban_core::resolve_page_params;
use kanban_domain::{CardListFilter, CardUpdate, FieldUpdate, KanbanOperations};
use kanban_service::api::CardResponse;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ErrorData as McpError},
    tool, tool_router,
};

impl ToolScoped for CreateCardParams {
    fn scope(&self) -> ToolScope {
        let column_ref = Ref::of(&self.column);
        let sprint_ref = self.sprint.as_deref().map(Ref::of);
        ToolScope {
            board: Some(Ref::of(&self.board)),
            wants_board_columns: matches!(column_ref, Ref::Name),
            wants_board_sprints: matches!(sprint_ref, Some(Ref::Name)),
            ..Default::default()
        }
    }
}

impl ToolScoped for ListCardsRequest {
    fn scope(&self) -> ToolScope {
        let board_ref = self.board.as_deref().map(Ref::of);
        let column_ref = self.column.as_deref().map(Ref::of);
        let sprint_ref = self.sprint.as_deref().map(Ref::of);
        ToolScope {
            board: board_ref,
            wants_board_columns: self.board.is_some() && matches!(column_ref, Some(Ref::Name)),
            wants_board_sprints: self.board.is_some() && matches!(sprint_ref, Some(Ref::Name)),
            ..Default::default()
        }
    }
}

impl ToolScoped for UpdateCardRequest {
    fn scope(&self) -> ToolScope {
        ToolScope::default()
    }
}

impl ToolScoped for MoveCardRequest {
    fn scope(&self) -> ToolScope {
        let column_ref = Ref::of(&self.column);
        ToolScope {
            wants_board_columns: matches!(column_ref, Ref::Name),
            ..Default::default()
        }
    }
}

impl ToolScoped for ArchiveCardRequest {
    fn scope(&self) -> ToolScope {
        ToolScope::default()
    }
}

impl ToolScoped for RestoreCardRequest {
    fn scope(&self) -> ToolScope {
        let column_ref = self.column.as_deref().map(Ref::of);
        ToolScope {
            wants_board_columns: matches!(column_ref, Some(Ref::Name)),
            ..Default::default()
        }
    }
}

impl ToolScoped for DeleteCardRequest {
    fn scope(&self) -> ToolScope {
        ToolScope::default()
    }
}

impl ToolScoped for GetCardBranchNameRequest {
    fn scope(&self) -> ToolScope {
        ToolScope::default()
    }
}

impl ToolScoped for GetCardGitCheckoutRequest {
    fn scope(&self) -> ToolScope {
        ToolScope::default()
    }
}

#[tool_router(router = card_crud_router, vis = "pub(crate)")]
impl KanbanMcpServer {
    #[tool(description = "Create a new card in a column")]
    pub async fn tool_create_card(
        &self,
        Parameters(mut req): Parameters<CreateCardParams>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let card = locked_write(&self.ctx, |ctx| {
            // Resolve the parent FKs (board/column name→id) and the optional
            // loose `sprint` name-or-id, threading the latter into the shared
            // content's typed `sprint_id` before conversion. Then funnel through
            // the Card factory: split the shared DTO into its optional client id
            // + content spec via `into_new_card(column_id)`, and
            // create-from-spec. The JSON edge projects via CardResponse.
            let mut model = ctx.model_for(&scope);
            let board_id = resolve_board(&model, &req.board)?;
            ctx.sync_into(&req.scope().for_board(board_id), &mut model);
            let column_id = resolve_column_in_board(&model, &req.column, board_id)?;
            if let Some(raw) = req.sprint.as_deref() {
                let board = board_head(ctx, &model, board_id)?;
                req.content.sprint_id = Some(resolve_sprint_in_board(&model, raw, &board)?);
            }
            let (id, spec) = req
                .content
                .into_new_card(column_id)
                .map_err(kanban_err_to_mcp)?;
            ctx.mutate(|c| c.create_card_from_spec(id, spec))
                .map(|(card, _inv)| card)
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
        let scope = req.scope();
        let result = locked_read(&self.ctx, |ctx| {
            let mut model = ctx.model_for(&scope);
            let board_id = match &req.board {
                Some(raw) => Some(resolve_board(&model, raw)?),
                None => None,
            };
            if let Some(bid) = board_id {
                ctx.sync_into(&req.scope().for_board(bid), &mut model);
            }
            let column_id = match &req.column {
                Some(raw) => Some(match board_id {
                    Some(bid) => resolve_column_in_board(&model, raw, bid)?,
                    None => resolve_column_global(ctx, raw)?,
                }),
                None => None,
            };
            let sprint_id = match &req.sprint {
                Some(raw) => Some(match board_id {
                    Some(bid) => {
                        let board = board_head(ctx, &model, bid)?;
                        resolve_sprint_in_board(&model, raw, &board)?
                    }
                    None => resolve_sprint_global(ctx, raw)?,
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
            let id = ctx.resolve_card_id(&req.card).map_err(kanban_err_to_mcp)?;
            ctx.mutate(|c| c.update_card_impl(id, updates))
                .map(|(card, _inv)| card)
                .map_err(kanban_err_to_mcp)
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
            let id = ctx.resolve_card_id(&req.card).map_err(kanban_err_to_mcp)?;
            let board_id = card_board(ctx, id)?;
            let model = ctx.model_for(&req.scope().for_board(board_id));
            let column_id = resolve_column_in_board(&model, &req.column, board_id)?;
            ctx.mutate(|c| c.move_card_impl(id, column_id, req.position))
                .map(|(card, _inv)| card)
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
            let id = ctx.resolve_card_id(&req.card).map_err(kanban_err_to_mcp)?;
            let (_val, _inv) = ctx
                .mutate(|c| c.archive_card_impl(id))
                .map_err(kanban_err_to_mcp)?;
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
            let id = ctx.resolve_card_id(&req.card).map_err(kanban_err_to_mcp)?;
            let column_id = match req.column.as_deref() {
                Some(raw) => {
                    let board_id = card_board(ctx, id)?;
                    let model = ctx.model_for(&req.scope().for_board(board_id));
                    Some(resolve_column_in_board(&model, raw, board_id)?)
                }
                None => None,
            };
            ctx.mutate(|c| c.restore_card_impl(id, column_id))
                .map(|(card, _inv)| card)
                .map_err(kanban_err_to_mcp)
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
            let id = ctx.resolve_card_id(&req.card).map_err(kanban_err_to_mcp)?;
            let _inv = ctx
                .mutate_unit(|c| c.delete_card_impl(id))
                .map_err(kanban_err_to_mcp)?;
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
            let id = ctx.resolve_card_id(&req.card).map_err(kanban_err_to_mcp)?;
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
            let id = ctx.resolve_card_id(&req.card).map_err(kanban_err_to_mcp)?;
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
    use crate::requests::sprint::CreateSprintParams;
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
        board_id: String,
        column_id: String,
        card_id: String,
        card_identifier: String,
        sprint_id: String,
        sprint_identifier: String,
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

        let sprint = text_payload(
            &server
                .tool_create_sprint(Parameters(CreateSprintParams {
                    board: board_id.clone(),
                    content: kanban_service::api::CreateSprintRequest {
                        id: None,
                        name: Some("Sprint One".to_string()),
                        prefix: None,
                        card_prefix: None,
                    },
                }))
                .await
                .unwrap(),
        );
        let sprint_id = sprint["id"].as_str().unwrap().to_string();
        let sprint_identifier = "Sprint One".to_string();

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
            sprint_id,
            sprint_identifier,
        }
    }

    #[tokio::test]
    async fn test_card_crud_tools_resolve_a_card_identifier_via_the_index_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();

        let updated = text_payload(
            &seeded
                .server
                .tool_update_card(Parameters(UpdateCardRequest {
                    card: seeded.card_identifier.clone(),
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
        assert_eq!(seeded.handle.op_count("list_cards_by_prefix_and_number"), 1);

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
    async fn test_card_crud_tools_resolve_a_card_identifier_via_the_index_on_sqlite() {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();

        let updated = text_payload(
            &seeded
                .server
                .tool_update_card(Parameters(UpdateCardRequest {
                    card: seeded.card_identifier.clone(),
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
        assert_eq!(seeded.handle.op_count("list_cards_by_prefix_and_number"), 1);

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
    async fn test_card_crud_tool_with_an_unloadable_card_index_errors_instead_of_reporting_not_found_on_json(
    ) {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_cards_by_prefix_and_number");

        let err = seeded
            .server
            .tool_archive_card(Parameters(ArchiveCardRequest {
                card: seeded.card_identifier.clone(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_card_crud_tool_with_an_unloadable_card_index_errors_instead_of_reporting_not_found_on_sqlite(
    ) {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_cards_by_prefix_and_number");

        let err = seeded
            .server
            .tool_archive_card(Parameters(ArchiveCardRequest {
                card: seeded.card_identifier.clone(),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
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

    #[tokio::test]
    async fn test_list_cards_by_global_sprint_name_without_a_board_resolves_on_json() {
        let seeded = seeded_server("test.json").await;
        let in_sprint = text_payload(
            &seeded
                .server
                .tool_create_card(Parameters(CreateCardParams {
                    board: seeded.board_id.clone(),
                    column: seeded.column_id.clone(),
                    sprint: Some(seeded.sprint_id.clone()),
                    content: kanban_service::api::CreateCardRequest {
                        id: None,
                        title: "In the sprint".to_string(),
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
        let in_sprint_id = in_sprint["id"].as_str().unwrap().to_string();

        let result = text_payload(
            &seeded
                .server
                .tool_list_cards(Parameters(ListCardsRequest {
                    board: None,
                    column: None,
                    sprint: Some(seeded.sprint_identifier.clone()),
                    status: None,
                    archived: None,
                    sort: None,
                    order: None,
                    page: None,
                    page_size: None,
                }))
                .await
                .unwrap(),
        );

        let ids: Vec<&str> = result["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec![in_sprint_id.as_str()]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_list_cards_by_global_sprint_name_without_a_board_resolves_on_sqlite() {
        let seeded = seeded_server("test.sqlite").await;
        let in_sprint = text_payload(
            &seeded
                .server
                .tool_create_card(Parameters(CreateCardParams {
                    board: seeded.board_id.clone(),
                    column: seeded.column_id.clone(),
                    sprint: Some(seeded.sprint_id.clone()),
                    content: kanban_service::api::CreateCardRequest {
                        id: None,
                        title: "In the sprint".to_string(),
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
        let in_sprint_id = in_sprint["id"].as_str().unwrap().to_string();

        let result = text_payload(
            &seeded
                .server
                .tool_list_cards(Parameters(ListCardsRequest {
                    board: None,
                    column: None,
                    sprint: Some(seeded.sprint_identifier.clone()),
                    status: None,
                    archived: None,
                    sort: None,
                    order: None,
                    page: None,
                    page_size: None,
                }))
                .await
                .unwrap(),
        );

        let ids: Vec<&str> = result["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec![in_sprint_id.as_str()]);
    }

    #[tokio::test]
    async fn test_list_cards_on_archived_board_by_uuid_with_named_sprint_resolves_on_json() {
        test_list_cards_on_archived_board_by_uuid_with_named_sprint_resolves("test.json").await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_list_cards_on_archived_board_by_uuid_with_named_sprint_resolves_on_sqlite() {
        test_list_cards_on_archived_board_by_uuid_with_named_sprint_resolves("test.sqlite").await;
    }

    async fn test_list_cards_on_archived_board_by_uuid_with_named_sprint_resolves(file_name: &str) {
        let seeded = seeded_server(file_name).await;

        let in_sprint = text_payload(
            &seeded
                .server
                .tool_create_card(Parameters(CreateCardParams {
                    board: seeded.board_id.clone(),
                    column: seeded.column_id.clone(),
                    sprint: Some(seeded.sprint_id.clone()),
                    content: kanban_service::api::CreateCardRequest {
                        id: None,
                        title: "In the sprint".to_string(),
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
        let in_sprint_id = in_sprint["id"].as_str().unwrap().to_string();

        seeded
            .server
            .tool_archive_board(Parameters(crate::requests::board::ArchiveBoardRequest {
                board: seeded.board_id.clone(),
            }))
            .await
            .unwrap();

        let result = text_payload(
            &seeded
                .server
                .tool_list_cards(Parameters(ListCardsRequest {
                    board: Some(seeded.board_id.clone()),
                    column: None,
                    sprint: Some(seeded.sprint_identifier.clone()),
                    status: None,
                    archived: None,
                    sort: None,
                    order: None,
                    page: None,
                    page_size: None,
                }))
                .await
                .unwrap(),
        );
        let ids: Vec<&str> = result["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec![in_sprint_id.as_str()]);

        let result_by_number = text_payload(
            &seeded
                .server
                .tool_list_cards(Parameters(ListCardsRequest {
                    board: Some(seeded.board_id.clone()),
                    column: None,
                    sprint: Some("1".to_string()),
                    status: None,
                    archived: None,
                    sort: None,
                    order: None,
                    page: None,
                    page_size: None,
                }))
                .await
                .unwrap(),
        );
        let ids_by_number: Vec<&str> = result_by_number["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids_by_number, vec![in_sprint_id.as_str()]);
    }

    #[tokio::test]
    async fn test_list_cards_by_global_sprint_name_with_an_unloadable_sprint_list_errors_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_all_sprints");

        let err = seeded
            .server
            .tool_list_cards(Parameters(ListCardsRequest {
                board: None,
                column: None,
                sprint: Some(seeded.sprint_identifier.clone()),
                status: None,
                archived: None,
                sort: None,
                order: None,
                page: None,
                page_size: None,
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_list_cards_by_global_sprint_name_with_an_unloadable_sprint_list_errors_on_sqlite()
    {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_all_sprints");

        let err = seeded
            .server
            .tool_list_cards(Parameters(ListCardsRequest {
                board: None,
                column: None,
                sprint: Some(seeded.sprint_identifier.clone()),
                status: None,
                archived: None,
                sort: None,
                order: None,
                page: None,
                page_size: None,
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    async fn assert_card_ref_by_identifier_errors_on_an_unloadable_card_index(
        seeded: &Seeded,
        label: &str,
        result: Result<CallToolResult, McpError>,
    ) {
        let err = result.unwrap_err();
        assert_eq!(
            err.code,
            ErrorCode::INTERNAL_ERROR,
            "{label} did not surface INTERNAL_ERROR"
        );
        assert!(
            err.message.contains("injected fault"),
            "{label}: {}",
            err.message
        );
        assert!(
            !err.message.to_lowercase().contains("not found"),
            "{label}: {}",
            err.message
        );
        let _ = seeded;
    }

    #[tokio::test]
    async fn test_card_crud_tools_error_on_an_unloadable_card_index_by_identifier_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_cards_by_prefix_and_number");

        assert_card_ref_by_identifier_errors_on_an_unloadable_card_index(
            &seeded,
            "tool_update_card",
            seeded
                .server
                .tool_update_card(Parameters(UpdateCardRequest {
                    card: seeded.card_identifier.clone(),
                    title: Some("Renamed".into()),
                    description: None,
                    priority: None,
                    status: None,
                    due_date: None,
                    clear_due_date: None,
                    points: None,
                }))
                .await,
        )
        .await;

        assert_card_ref_by_identifier_errors_on_an_unloadable_card_index(
            &seeded,
            "tool_move_card",
            seeded
                .server
                .tool_move_card(Parameters(MoveCardRequest {
                    card: seeded.card_identifier.clone(),
                    column: "TODO".to_string(),
                    position: None,
                }))
                .await,
        )
        .await;

        assert_card_ref_by_identifier_errors_on_an_unloadable_card_index(
            &seeded,
            "tool_restore_card",
            seeded
                .server
                .tool_restore_card(Parameters(RestoreCardRequest {
                    card: seeded.card_identifier.clone(),
                    column: None,
                }))
                .await,
        )
        .await;

        assert_card_ref_by_identifier_errors_on_an_unloadable_card_index(
            &seeded,
            "tool_delete_card",
            seeded
                .server
                .tool_delete_card(Parameters(DeleteCardRequest {
                    card: seeded.card_identifier.clone(),
                }))
                .await,
        )
        .await;

        assert_card_ref_by_identifier_errors_on_an_unloadable_card_index(
            &seeded,
            "tool_get_card_branch_name",
            seeded
                .server
                .tool_get_card_branch_name(Parameters(GetCardBranchNameRequest {
                    card: seeded.card_identifier.clone(),
                }))
                .await,
        )
        .await;

        assert_card_ref_by_identifier_errors_on_an_unloadable_card_index(
            &seeded,
            "tool_get_card_git_checkout",
            seeded
                .server
                .tool_get_card_git_checkout(Parameters(GetCardGitCheckoutRequest {
                    card: seeded.card_identifier.clone(),
                }))
                .await,
        )
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_card_crud_tools_error_on_an_unloadable_card_index_by_identifier_on_sqlite() {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_cards_by_prefix_and_number");

        assert_card_ref_by_identifier_errors_on_an_unloadable_card_index(
            &seeded,
            "tool_update_card",
            seeded
                .server
                .tool_update_card(Parameters(UpdateCardRequest {
                    card: seeded.card_identifier.clone(),
                    title: Some("Renamed".into()),
                    description: None,
                    priority: None,
                    status: None,
                    due_date: None,
                    clear_due_date: None,
                    points: None,
                }))
                .await,
        )
        .await;

        assert_card_ref_by_identifier_errors_on_an_unloadable_card_index(
            &seeded,
            "tool_move_card",
            seeded
                .server
                .tool_move_card(Parameters(MoveCardRequest {
                    card: seeded.card_identifier.clone(),
                    column: "TODO".to_string(),
                    position: None,
                }))
                .await,
        )
        .await;

        assert_card_ref_by_identifier_errors_on_an_unloadable_card_index(
            &seeded,
            "tool_restore_card",
            seeded
                .server
                .tool_restore_card(Parameters(RestoreCardRequest {
                    card: seeded.card_identifier.clone(),
                    column: None,
                }))
                .await,
        )
        .await;

        assert_card_ref_by_identifier_errors_on_an_unloadable_card_index(
            &seeded,
            "tool_delete_card",
            seeded
                .server
                .tool_delete_card(Parameters(DeleteCardRequest {
                    card: seeded.card_identifier.clone(),
                }))
                .await,
        )
        .await;

        assert_card_ref_by_identifier_errors_on_an_unloadable_card_index(
            &seeded,
            "tool_get_card_branch_name",
            seeded
                .server
                .tool_get_card_branch_name(Parameters(GetCardBranchNameRequest {
                    card: seeded.card_identifier.clone(),
                }))
                .await,
        )
        .await;

        assert_card_ref_by_identifier_errors_on_an_unloadable_card_index(
            &seeded,
            "tool_get_card_git_checkout",
            seeded
                .server
                .tool_get_card_git_checkout(Parameters(GetCardGitCheckoutRequest {
                    card: seeded.card_identifier.clone(),
                }))
                .await,
        )
        .await;
    }

    #[tokio::test]
    async fn test_move_card_with_an_unloadable_column_collection_errors_naming_the_collection_on_json(
    ) {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_columns_by_board");

        let err = seeded
            .server
            .tool_move_card(Parameters(MoveCardRequest {
                card: seeded.card_id.clone(),
                column: "TODO".to_string(),
                position: None,
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("columns of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_move_card_with_an_unloadable_column_collection_errors_naming_the_collection_on_sqlite(
    ) {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_columns_by_board");

        let err = seeded
            .server
            .tool_move_card(Parameters(MoveCardRequest {
                card: seeded.card_id.clone(),
                column: "TODO".to_string(),
                position: None,
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("columns of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test]
    async fn test_move_card_by_column_name_resolves_via_the_board_scoped_column_tier_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded
            .server
            .tool_create_column(Parameters(CreateColumnParams {
                board: seeded.board_id.clone(),
                content: kanban_service::api::CreateColumnRequest {
                    id: None,
                    name: "DOING".to_string(),
                    wip_limit: None,
                    default_status: None,
                },
            }))
            .await
            .unwrap();
        seeded.handle.clear_ops();

        let card = text_payload(
            &seeded
                .server
                .tool_move_card(Parameters(MoveCardRequest {
                    card: seeded.card_id.clone(),
                    column: "DOING".to_string(),
                    position: None,
                }))
                .await
                .unwrap(),
        );

        assert!(card["column_id"].as_str().is_some());
        assert_eq!(seeded.handle.op_count("list_all_columns"), 0);
        assert!(seeded.handle.op_count("list_columns_by_board") >= 1);
    }

    #[tokio::test]
    async fn test_restore_card_with_an_unloadable_column_collection_errors_naming_the_collection_on_json(
    ) {
        let seeded = seeded_server("test.json").await;
        seeded
            .server
            .tool_archive_card(Parameters(ArchiveCardRequest {
                card: seeded.card_id.clone(),
            }))
            .await
            .unwrap();
        seeded.handle.clear_ops();
        seeded.handle.fail("list_columns_by_board");

        let err = seeded
            .server
            .tool_restore_card(Parameters(RestoreCardRequest {
                card: seeded.card_id.clone(),
                column: Some("TODO".to_string()),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("columns of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_restore_card_with_an_unloadable_column_collection_errors_naming_the_collection_on_sqlite(
    ) {
        let seeded = seeded_server("test.sqlite").await;
        seeded
            .server
            .tool_archive_card(Parameters(ArchiveCardRequest {
                card: seeded.card_id.clone(),
            }))
            .await
            .unwrap();
        seeded.handle.clear_ops();
        seeded.handle.fail("list_columns_by_board");

        let err = seeded
            .server
            .tool_restore_card(Parameters(RestoreCardRequest {
                card: seeded.card_id.clone(),
                column: Some("TODO".to_string()),
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("columns of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test]
    async fn test_restore_card_by_column_name_resolves_via_the_board_scoped_column_tier_on_json() {
        let seeded = seeded_server("test.json").await;
        seeded
            .server
            .tool_archive_card(Parameters(ArchiveCardRequest {
                card: seeded.card_id.clone(),
            }))
            .await
            .unwrap();
        seeded.handle.clear_ops();

        let card = text_payload(
            &seeded
                .server
                .tool_restore_card(Parameters(RestoreCardRequest {
                    card: seeded.card_id.clone(),
                    column: Some("TODO".to_string()),
                }))
                .await
                .unwrap(),
        );

        assert_eq!(card["column_id"].as_str().unwrap(), seeded.column_id);
        assert_eq!(seeded.handle.op_count("list_all_columns"), 0);
        assert!(seeded.handle.op_count("list_columns_by_board") >= 1);
    }

    #[tokio::test]
    async fn test_list_cards_by_board_and_column_name_with_an_unloadable_column_collection_errors_on_json(
    ) {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_columns_by_board");

        let err = seeded
            .server
            .tool_list_cards(Parameters(ListCardsRequest {
                board: Some("Alpha".to_string()),
                column: Some("TODO".to_string()),
                sprint: None,
                status: None,
                archived: None,
                sort: None,
                order: None,
                page: None,
                page_size: None,
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("columns of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_list_cards_by_board_and_column_name_with_an_unloadable_column_collection_errors_on_sqlite(
    ) {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_columns_by_board");

        let err = seeded
            .server
            .tool_list_cards(Parameters(ListCardsRequest {
                board: Some("Alpha".to_string()),
                column: Some("TODO".to_string()),
                sprint: None,
                status: None,
                archived: None,
                sort: None,
                order: None,
                page: None,
                page_size: None,
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("columns of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test]
    async fn test_create_card_by_board_column_and_sprint_name_with_an_unloadable_sprint_collection_errors_on_json(
    ) {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_sprints_by_board");

        let err = seeded
            .server
            .tool_create_card(Parameters(CreateCardParams {
                board: "Alpha".to_string(),
                column: "TODO".to_string(),
                sprint: Some(seeded.sprint_identifier.clone()),
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
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("sprints of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_create_card_by_board_column_and_sprint_name_with_an_unloadable_sprint_collection_errors_on_sqlite(
    ) {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_sprints_by_board");

        let err = seeded
            .server
            .tool_create_card(Parameters(CreateCardParams {
                board: "Alpha".to_string(),
                column: "TODO".to_string(),
                sprint: Some(seeded.sprint_identifier.clone()),
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
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("sprints of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test]
    async fn test_list_cards_by_board_and_sprint_name_with_an_unloadable_sprint_collection_errors_on_json(
    ) {
        let seeded = seeded_server("test.json").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_sprints_by_board");

        let err = seeded
            .server
            .tool_list_cards(Parameters(ListCardsRequest {
                board: Some("Alpha".to_string()),
                column: None,
                sprint: Some(seeded.sprint_identifier.clone()),
                status: None,
                archived: None,
                sort: None,
                order: None,
                page: None,
                page_size: None,
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("sprints of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_list_cards_by_board_and_sprint_name_with_an_unloadable_sprint_collection_errors_on_sqlite(
    ) {
        let seeded = seeded_server("test.sqlite").await;
        seeded.handle.clear_ops();
        seeded.handle.fail("list_sprints_by_board");

        let err = seeded
            .server
            .tool_list_cards(Parameters(ListCardsRequest {
                board: Some("Alpha".to_string()),
                column: None,
                sprint: Some(seeded.sprint_identifier.clone()),
                status: None,
                archived: None,
                sort: None,
                order: None,
                page: None,
                page_size: None,
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INTERNAL_ERROR);
        assert!(err.message.contains("sprints of the board"));
        assert!(err.message.contains("injected fault"));
        assert!(!err.message.to_lowercase().contains("not found"));
    }

    async fn assert_ambiguous_bare_number_reports_ambiguity(file_name: &str) {
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

        text_payload(
            &seeded
                .server
                .tool_create_card(Parameters(CreateCardParams {
                    board: second_board_id,
                    column: second_column_id,
                    sprint: None,
                    content: kanban_service::api::CreateCardRequest {
                        id: None,
                        title: "Other board's card".to_string(),
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

        let err = seeded
            .server
            .tool_update_card(Parameters(UpdateCardRequest {
                card: "1".to_string(),
                title: Some("Should not apply".into()),
                description: None,
                priority: None,
                status: None,
                due_date: None,
                clear_due_date: None,
                points: None,
            }))
            .await
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("Do the thing"));
        assert!(err.message.contains("Other board's card"));

        let card = text_payload(
            &seeded
                .server
                .tool_get_card(Parameters(GetCardRequest {
                    card: seeded.card_id.clone(),
                }))
                .await
                .unwrap(),
        );
        assert_eq!(card["title"], "Do the thing");
    }

    #[tokio::test]
    async fn test_update_card_by_an_ambiguous_bare_number_reports_ambiguity_on_json() {
        assert_ambiguous_bare_number_reports_ambiguity("test.json").await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_update_card_by_an_ambiguous_bare_number_reports_ambiguity_on_sqlite() {
        assert_ambiguous_bare_number_reports_ambiguity("test.sqlite").await;
    }

    async fn assert_update_card_on_an_archived_board_still_mutates_it(file_name: &str) {
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

        let second_card = text_payload(
            &seeded
                .server
                .tool_create_card(Parameters(CreateCardParams {
                    board: second_board_id.clone(),
                    column: second_column_id,
                    sprint: None,
                    content: kanban_service::api::CreateCardRequest {
                        id: None,
                        title: "Card on archived board".to_string(),
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
        let second_card_identifier = format!(
            "{}-{}",
            second_card["prefix"].as_str().unwrap(),
            second_card["card_number"].as_u64().unwrap()
        );

        seeded
            .server
            .tool_archive_board(Parameters(crate::requests::board::ArchiveBoardRequest {
                board: second_board_id,
            }))
            .await
            .unwrap();

        let updated = text_payload(
            &seeded
                .server
                .tool_update_card(Parameters(UpdateCardRequest {
                    card: second_card_identifier,
                    title: Some("Renamed on an archived board".into()),
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
        assert_eq!(updated["title"], "Renamed on an archived board");
    }

    #[tokio::test]
    async fn test_update_card_by_identifier_on_an_archived_board_still_mutates_it_on_json() {
        assert_update_card_on_an_archived_board_still_mutates_it("test.json").await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_update_card_by_identifier_on_an_archived_board_still_mutates_it_on_sqlite() {
        assert_update_card_on_an_archived_board_still_mutates_it("test.sqlite").await;
    }

    struct ArchivedBoardWithColumns {
        seeded: Seeded,
        column_ids: Vec<String>,
        card_identifier: String,
    }

    async fn seed_archived_board_with_columns(
        file_name: &str,
        column_count: usize,
    ) -> ArchivedBoardWithColumns {
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

        let mut column_ids = Vec::with_capacity(column_count);
        for i in 0..column_count {
            let column = text_payload(
                &seeded
                    .server
                    .tool_create_column(Parameters(CreateColumnParams {
                        board: second_board_id.clone(),
                        content: kanban_service::api::CreateColumnRequest {
                            id: None,
                            name: format!("Column {i}"),
                            wip_limit: None,
                            default_status: None,
                        },
                    }))
                    .await
                    .unwrap(),
            );
            column_ids.push(column["id"].as_str().unwrap().to_string());
        }

        let card = text_payload(
            &seeded
                .server
                .tool_create_card(Parameters(CreateCardParams {
                    board: second_board_id.clone(),
                    column: column_ids[0].clone(),
                    sprint: None,
                    content: kanban_service::api::CreateCardRequest {
                        id: None,
                        title: "Card on archived board".to_string(),
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
        let card_identifier = format!(
            "{}-{}",
            card["prefix"].as_str().unwrap(),
            card["card_number"].as_u64().unwrap()
        );

        seeded
            .server
            .tool_archive_board(Parameters(crate::requests::board::ArchiveBoardRequest {
                board: second_board_id,
            }))
            .await
            .unwrap();

        ArchivedBoardWithColumns {
            seeded,
            column_ids,
            card_identifier,
        }
    }

    async fn assert_move_card_on_an_archived_board_still_moves_it(file_name: &str) {
        let fixture = seed_archived_board_with_columns(file_name, 2).await;

        let moved = text_payload(
            &fixture
                .seeded
                .server
                .tool_move_card(Parameters(MoveCardRequest {
                    card: fixture.card_identifier,
                    column: fixture.column_ids[1].clone(),
                    position: None,
                }))
                .await
                .unwrap(),
        );
        assert_eq!(moved["column_id"], fixture.column_ids[1]);
    }

    #[tokio::test]
    async fn test_move_card_by_identifier_on_an_archived_board_still_moves_it_on_json() {
        assert_move_card_on_an_archived_board_still_moves_it("test.json").await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_move_card_by_identifier_on_an_archived_board_still_moves_it_on_sqlite() {
        assert_move_card_on_an_archived_board_still_moves_it("test.sqlite").await;
    }

    async fn assert_archive_card_on_an_archived_board_still_archives_it(file_name: &str) {
        let fixture = seed_archived_board_with_columns(file_name, 1).await;

        let archived = text_payload(
            &fixture
                .seeded
                .server
                .tool_archive_card(Parameters(ArchiveCardRequest {
                    card: fixture.card_identifier,
                }))
                .await
                .unwrap(),
        );
        assert!(archived["archived"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_archive_card_by_identifier_on_an_archived_board_still_archives_it_on_json() {
        assert_archive_card_on_an_archived_board_still_archives_it("test.json").await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_archive_card_by_identifier_on_an_archived_board_still_archives_it_on_sqlite() {
        assert_archive_card_on_an_archived_board_still_archives_it("test.sqlite").await;
    }

    async fn assert_delete_card_on_an_archived_board_still_deletes_it(file_name: &str) {
        let fixture = seed_archived_board_with_columns(file_name, 1).await;

        let deleted = text_payload(
            &fixture
                .seeded
                .server
                .tool_delete_card(Parameters(DeleteCardRequest {
                    card: fixture.card_identifier.clone(),
                }))
                .await
                .unwrap(),
        );
        assert!(deleted["deleted"].as_str().is_some());

        let err = fixture
            .seeded
            .server
            .tool_delete_card(Parameters(DeleteCardRequest {
                card: fixture.card_identifier,
            }))
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
    }

    #[tokio::test]
    async fn test_delete_card_by_identifier_on_an_archived_board_still_deletes_it_on_json() {
        assert_delete_card_on_an_archived_board_still_deletes_it("test.json").await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_delete_card_by_identifier_on_an_archived_board_still_deletes_it_on_sqlite() {
        assert_delete_card_on_an_archived_board_still_deletes_it("test.sqlite").await;
    }
}
