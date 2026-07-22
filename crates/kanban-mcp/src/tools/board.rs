use crate::helpers::{
    core_err_to_mcp, kanban_err_to_mcp, locked_read, locked_write, mutating_op,
    parse_archived_selector, parse_board_sort_field, parse_sort_field, parse_sort_order,
    to_call_tool_result, to_call_tool_result_json, McpResolve,
};
use crate::requests::board::{
    ArchiveBoardRequest, CreateBoardRequest, DeleteArchivedBoardRequest, DeleteBoardRequest,
    GetBoardRequest, ListBoardsRequest, RestoreBoardRequest, SetBoardSortRequest,
    UpdateBoardRequest,
};
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
        description = "List kanban boards with an `archived` selector: 'exclude' (default, live only), 'only' (archived only), or 'include' (both). Archived boards carry an archived_at timestamp. Returns all boards by default; pass page/page_size to paginate."
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
        match (req.page, req.page_size) {
            (None, None) => to_call_tool_result(&responses),
            _ => {
                let (page, page_size) =
                    resolve_page_params(req.page, req.page_size).map_err(core_err_to_mcp)?;
                let paged =
                    PaginatedList::paginate(responses, page, page_size).map_err(core_err_to_mcp)?;
                to_call_tool_result(&paged)
            }
        }
    }

    #[tool(description = "Get a specific board by UUID or name")]
    pub async fn tool_get_board(
        &self,
        Parameters(req): Parameters<GetBoardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let response = locked_read(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_board(&req.board)?;
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
        locked_write(&self.ctx, |ctx| {
            ctx.set_board_sort(field, order).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({
            "board_sort_field": field.map(|f| f.to_string()),
            "board_sort_order": order.map(|o| o.to_string()),
        }))
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
            // Resolve against the ARCHIVED-only set: an archived board is not in
            // the live list, and scoping to archived-only guarantees a same-named
            // live board is never hit (KAN-894 data-loss guard).
            let id = resolve_archived_board(ctx, &req.board)?;
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
            let id = resolve_archived_board(ctx, &req.board)?;
            ctx.delete_board(id).map_err(kanban_err_to_mcp)?;
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
