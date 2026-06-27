use crate::helpers::{
    kanban_err_to_mcp, locked_read, mutating_op, to_call_tool_result, McpResolve,
};
use crate::requests::transfer::{ExportBoardRequest, ImportBoardRequest};
use crate::KanbanMcpServer;
use kanban_domain::KanbanOperations;
use kanban_service::api::BoardResponse;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    tool, tool_router,
};

#[tool_router(router = transfer_router, vis = "pub(crate)")]
impl KanbanMcpServer {
    #[tool(description = "Export board data as JSON")]
    pub async fn tool_export_board(
        &self,
        Parameters(req): Parameters<ExportBoardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let json = locked_read(&self.ctx, |ctx| {
            let board_id = match req.board.as_deref() {
                Some(raw) => Some(ctx.mcp_resolve_board(raw)?),
                None => None,
            };
            ctx.export_board(board_id).map_err(kanban_err_to_mcp)
        })
        .await?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Import board data from JSON")]
    pub async fn tool_import_board(
        &self,
        Parameters(req): Parameters<ImportBoardRequest>,
    ) -> Result<CallToolResult, McpError> {
        let data = req.data;
        let board = mutating_op!(self.ctx, import_board, &data)?;
        to_call_tool_result(&BoardResponse::from(&board))
    }

    #[tool(description = "Undo the last operation")]
    pub async fn tool_undo(&self) -> Result<CallToolResult, McpError> {
        let mut guard = self.ctx.lock().await;
        if guard.undo().map_err(kanban_err_to_mcp)? {
            guard.save().await.map_err(kanban_err_to_mcp)?;
            Ok(CallToolResult::success(vec![Content::text(
                "Undo successful",
            )]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(
                "Nothing to undo",
            )]))
        }
    }

    #[tool(description = "Redo the last undone operation")]
    pub async fn tool_redo(&self) -> Result<CallToolResult, McpError> {
        let mut guard = self.ctx.lock().await;
        if guard.redo().map_err(kanban_err_to_mcp)? {
            guard.save().await.map_err(kanban_err_to_mcp)?;
            Ok(CallToolResult::success(vec![Content::text(
                "Redo successful",
            )]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(
                "Nothing to redo",
            )]))
        }
    }
}
