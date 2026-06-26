use crate::helpers::{
    card_board, kanban_err_to_mcp, locked_write, to_call_tool_result, to_call_tool_result_json,
    McpResolve,
};
use crate::requests::card::{
    ArchiveCardsRequest, AssignCardToSprintRequest, AssignCardsToSprintRequest, MoveCardsRequest,
    UnassignCardFromSprintRequest,
};
use crate::KanbanMcpServer;
use kanban_domain::KanbanOperations;
use kanban_service::api::CardResponse;
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ErrorData as McpError},
    tool, tool_router,
};

#[tool_router(router = card_batch_router, vis = "pub(crate)")]
impl KanbanMcpServer {
    // Card Sprint Operations

    #[tool(description = "Assign a card to a sprint on the same board")]
    pub async fn tool_assign_card_to_sprint(
        &self,
        Parameters(req): Parameters<AssignCardToSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let card = locked_write(&self.ctx, |ctx| {
            let card_id = ctx.mcp_resolve_card(&req.card)?;
            let board_id = card_board(ctx, card_id)?;
            let sprint_id = ctx.mcp_resolve_sprint_in_board(&req.sprint, board_id)?;
            ctx.assign_card_to_sprint(card_id, sprint_id)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&CardResponse::from(&card))
    }

    #[tool(description = "Unassign a card from its sprint")]
    pub async fn tool_unassign_card_from_sprint(
        &self,
        Parameters(req): Parameters<UnassignCardFromSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let card = locked_write(&self.ctx, |ctx| {
            let card_id = ctx.mcp_resolve_card(&req.card)?;
            ctx.unassign_card_from_sprint(card_id)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&CardResponse::from(&card))
    }

    // Multi-card operations

    #[tool(
        description = "Archive multiple cards at once. IDs may be UUIDs or identifiers (e.g. 'KAN-1', '42')."
    )]
    pub async fn tool_archive_cards(
        &self,
        Parameters(req): Parameters<ArchiveCardsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let count = locked_write(&self.ctx, |ctx| {
            let ids = ctx.mcp_resolve_cards(&req.cards)?;
            ctx.archive_cards(ids).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"archived_count": count}))
    }

    #[tool(
        description = "Move multiple cards to a column. All cards must share a board; the column is resolved on that board."
    )]
    pub async fn tool_move_cards(
        &self,
        Parameters(req): Parameters<MoveCardsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let count = locked_write(&self.ctx, |ctx| {
            let ids = ctx.mcp_resolve_cards(&req.cards)?;
            let board_id = ctx.mcp_require_same_board(&ids)?;
            let column_id = ctx.mcp_resolve_column_in_board(&req.column, board_id)?;
            ctx.move_cards(ids, column_id).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"moved_count": count}))
    }

    #[tool(
        description = "Assign multiple cards to a sprint. All cards must share a board; the sprint is resolved on that board."
    )]
    pub async fn tool_assign_cards_to_sprint(
        &self,
        Parameters(req): Parameters<AssignCardsToSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let count = locked_write(&self.ctx, |ctx| {
            let ids = ctx.mcp_resolve_cards(&req.cards)?;
            let board_id = ctx.mcp_require_same_board(&ids)?;
            let sprint_id = ctx.mcp_resolve_sprint_in_board(&req.sprint, board_id)?;
            ctx.assign_cards_to_sprint(ids, sprint_id)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"assigned_count": count}))
    }
}
