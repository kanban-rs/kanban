use crate::helpers::{
    core_err_to_mcp, kanban_err_to_mcp, locked_read, locked_write, mutating_op,
    parse_archived_selector, parse_sort_field, parse_sort_order, to_call_tool_result,
    to_call_tool_result_json, McpResolve,
};
use crate::requests::board::{
    ArchiveBoardRequest, CreateBoardRequest, DeleteArchivedBoardRequest, DeleteBoardRequest,
    GetBoardRequest, ListBoardsRequest, RestoreBoardRequest, UpdateBoardRequest,
};
use crate::KanbanMcpServer;
use kanban_core::{resolve_page_params, PaginatedList};
use kanban_domain::{ArchivedFilter, BoardUpdate, FieldUpdate, KanbanOperations};
use kanban_service::api::BoardResponse;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ErrorData as McpError},
    tool, tool_router,
};

#[tool_router(router = board_router, vis = "pub(crate)")]
impl KanbanMcpServer {
    #[tool(description = "Create a new kanban board")]
    pub async fn tool_create_board(
        &self,
        Parameters(req): Parameters<CreateBoardRequest>,
    ) -> Result<CallToolResult, McpError> {
        // Funnel through the Board factory: split the shared DTO into its
        // optional client id + content spec, then create-from-spec. The JSON
        // edge projects the resulting domain Board via BoardResponse.
        let (id, spec) = req.into_new_board();
        let board = mutating_op!(self.ctx, create_board_from_spec, id, spec)?;
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
        let (page, page_size) =
            resolve_page_params(req.page, req.page_size).map_err(core_err_to_mcp)?;
        // Boards have no domain list-filter, so the three states are composed here
        // from the service ops (mirroring the CLI's I4 `build_board_list`).
        let responses = locked_read(&self.ctx, |ctx| -> Result<Vec<BoardResponse>, McpError> {
            let mut out: Vec<BoardResponse> = Vec::new();
            if archived != ArchivedFilter::ArchivedOnly {
                out.extend(
                    ctx.list_boards()
                        .map_err(kanban_err_to_mcp)?
                        .iter()
                        .map(BoardResponse::from),
                );
            }
            if archived != ArchivedFilter::LiveOnly {
                for marker in ctx.list_archived_boards().map_err(kanban_err_to_mcp)? {
                    // `get_board` is unfiltered: it resolves the still-live head.
                    if let Some(board) =
                        ctx.get_board(marker.entity_id).map_err(kanban_err_to_mcp)?
                    {
                        out.push(BoardResponse::archived(&board, marker.metadata.archived_at));
                    }
                }
            }
            Ok(out)
        })
        .await?;
        let paged = PaginatedList::paginate(responses, page, page_size).map_err(core_err_to_mcp)?;
        to_call_tool_result(&paged)
    }

    #[tool(description = "Get a specific board by UUID or name")]
    pub async fn tool_get_board(
        &self,
        Parameters(req): Parameters<GetBoardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let board = locked_read(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_board(&req.board)?;
            ctx.get_board(id).map_err(kanban_err_to_mcp)
        })
        .await?;
        let response = board.as_ref().map(BoardResponse::from);
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
        let board = locked_write(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_board(&req.board)?;
            ctx.update_board(id, updates).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&BoardResponse::from(&board))
    }

    #[tool(description = "Delete a board and all its columns, cards, and sprints")]
    pub async fn tool_delete_board(
        &self,
        Parameters(req): Parameters<DeleteBoardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let id = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let id = ctx.mcp_resolve_board(&req.board)?;
            ctx.delete_board(id).map_err(kanban_err_to_mcp)?;
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
        let id = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            // Live board (still in the live list) — the standard resolver suffices.
            let id = ctx.mcp_resolve_board(&req.board)?;
            ctx.archive_board(id).map_err(kanban_err_to_mcp)?;
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
            // Resolve from either view: an archived board is not in the live list.
            let id = ctx.mcp_resolve_board_any(&req.board)?;
            ctx.restore_board(id).map_err(kanban_err_to_mcp)?;
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
            let id = ctx.mcp_resolve_board_any(&req.board)?;
            ctx.delete_board(id).map_err(kanban_err_to_mcp)?;
            Ok(id)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"deleted": id.to_string()}))
    }
}
