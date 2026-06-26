use crate::helpers::{
    kanban_err_to_mcp, locked_read, locked_write, to_call_tool_result, to_call_tool_result_json,
    McpResolve,
};
use crate::requests::column::{
    CreateColumnParams, DeleteColumnRequest, GetColumnRequest, ListColumnsRequest,
    ReorderColumnRequest, UpdateColumnRequest,
};
use crate::KanbanMcpServer;
use kanban_domain::{ColumnUpdate, FieldUpdate, KanbanOperations};
use kanban_service::api::ColumnResponse;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ErrorData as McpError},
    tool, tool_router,
};

#[tool_router(router = column_router, vis = "pub(crate)")]
impl KanbanMcpServer {
    #[tool(description = "Create a new column in a board")]
    pub async fn tool_create_column(
        &self,
        Parameters(req): Parameters<CreateColumnParams>,
    ) -> Result<CallToolResult, McpError> {
        let column = locked_write(&self.ctx, |ctx| {
            // Resolve the single parent FK (board name→id) then funnel through
            // the Column factory: split the shared DTO into its optional client
            // id + content spec (server assigns the append position), and
            // create-from-spec. The JSON edge projects via ColumnResponse.
            let board_id = ctx.mcp_resolve_board(&req.board)?;
            let (id, spec) = req
                .content
                .into_new_column(board_id)
                .map_err(kanban_err_to_mcp)?;
            ctx.create_column_from_spec(id, spec)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&ColumnResponse::from(column))
    }

    #[tool(description = "List all columns in a board")]
    pub async fn tool_list_columns(
        &self,
        Parameters(req): Parameters<ListColumnsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let columns = locked_read(&self.ctx, |ctx| {
            let board_id = ctx.mcp_resolve_board(&req.board)?;
            ctx.list_columns(board_id).map_err(kanban_err_to_mcp)
        })
        .await?;
        let responses: Vec<ColumnResponse> =
            columns.into_iter().map(ColumnResponse::from).collect();
        to_call_tool_result(&responses)
    }

    #[tool(description = "Get a specific column by UUID or name (searched across all boards)")]
    pub async fn tool_get_column(
        &self,
        Parameters(req): Parameters<GetColumnRequest>,
    ) -> Result<CallToolResult, McpError> {
        let column = locked_read(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_column_global(&req.column)?;
            ctx.get_column(id).map_err(kanban_err_to_mcp)
        })
        .await?;
        let response = column.map(ColumnResponse::from);
        to_call_tool_result(&response)
    }

    #[tool(description = "Update a column's properties (name, position, wip_limit)")]
    pub async fn tool_update_column(
        &self,
        Parameters(req): Parameters<UpdateColumnRequest>,
    ) -> Result<CallToolResult, McpError> {
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
        };
        let column = locked_write(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_column_global(&req.column)?;
            ctx.update_column(id, updates).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&ColumnResponse::from(column))
    }

    #[tool(description = "Delete a column and all its cards")]
    pub async fn tool_delete_column(
        &self,
        Parameters(req): Parameters<DeleteColumnRequest>,
    ) -> Result<CallToolResult, McpError> {
        let id = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let id = ctx.mcp_resolve_column_global(&req.column)?;
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
        let column = locked_write(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_column_global(&req.column)?;
            ctx.reorder_column(id, req.position)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&ColumnResponse::from(column))
    }
}
