use crate::helpers::{
    kanban_err_to_mcp, locked_read, locked_write, parse_datetime, to_call_tool_result,
    to_call_tool_result_json, McpResolve,
};
use crate::requests::sprint::{
    ActivateSprintRequest, CancelSprintRequest, CarryOverSprintCardsRequest, CompleteSprintRequest,
    CreateSprintRequest, DeleteSprintRequest, GetSprintRequest, ListSprintsRequest,
    UpdateSprintRequest,
};
use crate::KanbanMcpServer;
use kanban_domain::{FieldUpdate, KanbanOperations, SprintUpdate};
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ErrorData as McpError},
    tool, tool_router,
};

#[tool_router(router = sprint_router, vis = "pub(crate)")]
impl KanbanMcpServer {
    #[tool(description = "Create a new sprint")]
    pub async fn tool_create_sprint(
        &self,
        Parameters(req): Parameters<CreateSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let sprint = locked_write(&self.ctx, |ctx| {
            let board_id = ctx.mcp_resolve_board(&req.board)?;
            ctx.create_sprint(board_id, req.prefix, req.name)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&sprint)
    }

    #[tool(description = "List sprints for a board")]
    pub async fn tool_list_sprints(
        &self,
        Parameters(req): Parameters<ListSprintsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let sprints = locked_read(&self.ctx, |ctx| {
            let board_id = ctx.mcp_resolve_board(&req.board)?;
            ctx.list_sprints(board_id).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&sprints)
    }

    #[tool(description = "Get a specific sprint by UUID, name, or number")]
    pub async fn tool_get_sprint(
        &self,
        Parameters(req): Parameters<GetSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let sprint = locked_read(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_sprint_global(&req.sprint)?;
            ctx.get_sprint(id).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&sprint)
    }

    #[tool(
        description = "Update a sprint's properties (name, prefix, card_prefix, start_date, end_date)"
    )]
    pub async fn tool_update_sprint(
        &self,
        Parameters(req): Parameters<UpdateSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let start_date = if req.clear_start_date == Some(true) {
            FieldUpdate::Clear
        } else {
            match req.start_date {
                Some(ref d) => FieldUpdate::Set(parse_datetime(d)?),
                None => FieldUpdate::NoChange,
            }
        };
        let end_date = if req.clear_end_date == Some(true) {
            FieldUpdate::Clear
        } else {
            match req.end_date {
                Some(ref d) => FieldUpdate::Set(parse_datetime(d)?),
                None => FieldUpdate::NoChange,
            }
        };
        let updates = SprintUpdate {
            name: req.name,
            name_index: FieldUpdate::NoChange,
            prefix: req
                .prefix
                .map(FieldUpdate::Set)
                .unwrap_or(FieldUpdate::NoChange),
            card_prefix: req
                .card_prefix
                .map(FieldUpdate::Set)
                .unwrap_or(FieldUpdate::NoChange),
            status: None,
            start_date,
            end_date,
        };
        let sprint = locked_write(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_sprint_global(&req.sprint)?;
            ctx.update_sprint(id, updates).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&sprint)
    }

    #[tool(description = "Activate a sprint")]
    pub async fn tool_activate_sprint(
        &self,
        Parameters(req): Parameters<ActivateSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let sprint = locked_write(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_sprint_global(&req.sprint)?;
            ctx.activate_sprint(id, req.duration_days)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&sprint)
    }

    #[tool(description = "Complete a sprint")]
    pub async fn tool_complete_sprint(
        &self,
        Parameters(req): Parameters<CompleteSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let sprint = locked_write(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_sprint_global(&req.sprint)?;
            ctx.complete_sprint(id).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&sprint)
    }

    #[tool(description = "Cancel a sprint")]
    pub async fn tool_cancel_sprint(
        &self,
        Parameters(req): Parameters<CancelSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let sprint = locked_write(&self.ctx, |ctx| {
            let id = ctx.mcp_resolve_sprint_global(&req.sprint)?;
            ctx.cancel_sprint(id).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&sprint)
    }

    #[tool(description = "Delete a sprint")]
    pub async fn tool_delete_sprint(
        &self,
        Parameters(req): Parameters<DeleteSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let id = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let id = ctx.mcp_resolve_sprint_global(&req.sprint)?;
            ctx.delete_sprint(id).map_err(kanban_err_to_mcp)?;
            Ok(id)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"deleted": id.to_string()}))
    }

    #[tool(
        description = "Carry over uncompleted cards from a completed/cancelled sprint to a planning sprint on the same board"
    )]
    pub async fn tool_carry_over_sprint_cards(
        &self,
        Parameters(req): Parameters<CarryOverSprintCardsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let count = locked_write(&self.ctx, |ctx| {
            let from_id = ctx.mcp_resolve_sprint_global(&req.from_sprint)?;
            let from_sprint = ctx
                .get_sprint(from_id)
                .map_err(kanban_err_to_mcp)?
                .ok_or_else(|| {
                    McpError::invalid_params(format!("Sprint not found: {}", from_id), None)
                })?;
            let to_id = ctx.mcp_resolve_sprint_in_board(&req.to_sprint, from_sprint.board_id)?;
            ctx.carry_over_sprint_cards(from_id, to_id)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({ "carried_over_count": count }))
    }
}
