use crate::helpers::{
    kanban_err_to_mcp, locked_read, locked_write, mutating_op, parse_sort_field, parse_sort_order,
    read_op, to_call_tool_result, to_call_tool_result_json, McpResolve,
};
use crate::requests::board::{
    CreateBoardRequest, DeleteBoardRequest, GetBoardRequest, UpdateBoardRequest,
};
use crate::KanbanMcpServer;
use kanban_domain::{BoardUpdate, FieldUpdate, KanbanOperations};
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

    #[tool(description = "List all kanban boards")]
    pub async fn tool_list_boards(&self) -> Result<CallToolResult, McpError> {
        let boards = read_op!(self.ctx, list_boards)?;
        to_call_tool_result(&boards)
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
        to_call_tool_result(&board)
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
        to_call_tool_result(&board)
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
}
