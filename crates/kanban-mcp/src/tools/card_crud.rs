use crate::helpers::{
    card_board, core_err_to_mcp, kanban_err_to_mcp, locked_read, locked_write,
    parse_archived_selector, parse_datetime, parse_priority, parse_sort_field, parse_sort_order,
    parse_status, read_op, to_call_tool_result, to_call_tool_result_json, McpResolve,
};
use crate::requests::card::{
    ArchiveCardRequest, CreateCardParams, DeleteCardRequest, GetCardBranchNameRequest,
    GetCardGitCheckoutRequest, GetCardRequest, ListArchivedCardsRequest, ListCardsRequest,
    MoveCardRequest, RestoreCardRequest, UpdateCardRequest,
};
use crate::KanbanMcpServer;
use kanban_core::resolve_page_params;
use kanban_domain::{ArchivedFilter, CardListFilter, CardUpdate, FieldUpdate, KanbanOperations};
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
        if let Ok(uuid) = uuid::Uuid::parse_str(&req.card) {
            let card = read_op!(self.ctx, get_card, uuid)?;
            let response = card.as_ref().map(CardResponse::from);
            return to_call_tool_result(&response);
        }
        let cards = {
            let guard = self.ctx.lock().await;
            guard
                .find_cards_by_identifier(&req.card)
                .map_err(kanban_err_to_mcp)?
        };
        match cards.as_slice() {
            [] => Err(McpError::invalid_params(
                format!("Card not found: '{}'", req.card),
                None,
            )),
            [card] => to_call_tool_result(&CardResponse::from(card)),
            _ => {
                let responses: Vec<CardResponse> = cards.iter().map(CardResponse::from).collect();
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

    #[tool(
        description = "DEPRECATED: use list_cards with archived='only'. Lists archived cards as the unified CardSummary (each carrying archived_at). Kept as a thin wrapper for existing clients. Use page/page_size for pagination (default: page=1, page_size=50)."
    )]
    pub async fn tool_list_archived_cards(
        &self,
        Parameters(req): Parameters<ListArchivedCardsRequest>,
    ) -> Result<CallToolResult, McpError> {
        // I2 (KAN-882): one code path. This deprecated tool routes to the unified
        // list with the archived-only selector, so its output matches
        // `list_cards` with archived='only'.
        let sort = req.sort.as_deref().map(parse_sort_field).transpose()?;
        let sort_order = req.order.as_deref().map(parse_sort_order).transpose()?;
        let (page, page_size) =
            resolve_page_params(req.page, req.page_size).map_err(core_err_to_mcp)?;
        let result = locked_read(&self.ctx, |ctx| {
            let board_id = match &req.board {
                Some(raw) => Some(ctx.mcp_resolve_board(raw)?),
                None => None,
            };
            let filter = CardListFilter {
                board_id,
                sort,
                sort_order,
                archived: ArchivedFilter::ArchivedOnly,
                ..Default::default()
            };
            ctx.list_cards_paged(filter, page, page_size)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&result)
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
