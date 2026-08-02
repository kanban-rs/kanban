use crate::helpers::{
    core_err_to_mcp, kanban_err_to_mcp, locked_read, locked_write, parse_datetime, project_sprint,
    to_call_tool_result, to_call_tool_result_json, McpResolve,
};
use crate::requests::sprint::{
    ActivateSprintRequest, CancelSprintRequest, CarryOverSprintCardsRequest, CompleteSprintRequest,
    CreateSprintParams, DeleteSprintRequest, GetSprintRequest, ListSprintsRequest,
    UpdateSprintRequest,
};
use crate::KanbanMcpServer;
use kanban_core::{resolve_page_params, PaginatedList};
use kanban_domain::{FieldUpdate, KanbanError, KanbanOperations, SprintUpdate};
use kanban_service::api::SprintResponse;
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
        Parameters(req): Parameters<CreateSprintParams>,
    ) -> Result<CallToolResult, McpError> {
        let response = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            // Resolve the parent board (name→id), then funnel the shared DTO
            // content through the Sprint factory via `create_sprint_from_spec`.
            // The JSON edge projects the domain Sprint via SprintResponse,
            // resolving the sprint name against its owning board.
            let board_id = ctx.mcp_resolve_board(&req.board)?;
            let content = req.content;
            let sprint = ctx
                .create_sprint_from_spec(board_id, content.id, content.name, content.prefix)
                .map_err(kanban_err_to_mcp)?;
            let board = ctx
                .get_board(board_id)
                .map_err(kanban_err_to_mcp)?
                .ok_or_else(|| kanban_err_to_mcp(KanbanError::not_found("Board", board_id)))?;
            Ok(SprintResponse::from_sprint(&sprint, &board))
        })
        .await?;
        to_call_tool_result(&response)
    }

    #[tool(
        description = "List sprints for a board. Use page/page_size for pagination (default: page=1, page_size=50)."
    )]
    pub async fn tool_list_sprints(
        &self,
        Parameters(req): Parameters<ListSprintsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let responses = locked_read(&self.ctx, |ctx| -> Result<_, McpError> {
            let board_id = ctx.mcp_resolve_board(&req.board)?;
            let sprints = ctx.list_sprints(board_id).map_err(kanban_err_to_mcp)?;
            let board = ctx
                .get_board(board_id)
                .map_err(kanban_err_to_mcp)?
                .ok_or_else(|| kanban_err_to_mcp(KanbanError::not_found("Board", board_id)))?;
            Ok(sprints
                .iter()
                .map(|s| SprintResponse::from_sprint(s, &board))
                .collect::<Vec<_>>())
        })
        .await?;
        let (page, page_size) =
            resolve_page_params(req.page, req.page_size).map_err(core_err_to_mcp)?;
        let paged = PaginatedList::paginate(responses, page, page_size).map_err(core_err_to_mcp)?;
        to_call_tool_result(&paged)
    }

    #[tool(description = "Get a specific sprint by UUID, name, or number")]
    pub async fn tool_get_sprint(
        &self,
        Parameters(req): Parameters<GetSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let response = locked_read(&self.ctx, |ctx| -> Result<_, McpError> {
            let id = ctx.mcp_resolve_sprint_global(&req.sprint)?;
            let Some(sprint) = ctx.get_sprint(id).map_err(kanban_err_to_mcp)? else {
                return Ok(None);
            };
            let board = ctx
                .get_board(sprint.board_id)
                .map_err(kanban_err_to_mcp)?
                .ok_or_else(|| {
                    kanban_err_to_mcp(KanbanError::not_found("Board", sprint.board_id))
                })?;
            Ok(Some(SprintResponse::from_sprint(&sprint, &board)))
        })
        .await?;
        to_call_tool_result(&response)
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
        let response = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let id = ctx.mcp_resolve_sprint_global(&req.sprint)?;
            let sprint = ctx.update_sprint(id, updates).map_err(kanban_err_to_mcp)?;
            project_sprint(ctx, sprint)
        })
        .await?;
        to_call_tool_result(&response)
    }

    #[tool(description = "Activate a sprint")]
    pub async fn tool_activate_sprint(
        &self,
        Parameters(req): Parameters<ActivateSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let response = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let id = ctx.mcp_resolve_sprint_global(&req.sprint)?;
            let sprint = ctx
                .activate_sprint(id, req.duration_days)
                .map_err(kanban_err_to_mcp)?;
            project_sprint(ctx, sprint)
        })
        .await?;
        to_call_tool_result(&response)
    }

    #[tool(description = "Complete a sprint")]
    pub async fn tool_complete_sprint(
        &self,
        Parameters(req): Parameters<CompleteSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let response = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let id = ctx.mcp_resolve_sprint_global(&req.sprint)?;
            let sprint = ctx.complete_sprint(id).map_err(kanban_err_to_mcp)?;
            project_sprint(ctx, sprint)
        })
        .await?;
        to_call_tool_result(&response)
    }

    #[tool(description = "Cancel a sprint")]
    pub async fn tool_cancel_sprint(
        &self,
        Parameters(req): Parameters<CancelSprintRequest>,
    ) -> Result<CallToolResult, McpError> {
        let response = locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let id = ctx.mcp_resolve_sprint_global(&req.sprint)?;
            let sprint = ctx.cancel_sprint(id).map_err(kanban_err_to_mcp)?;
            project_sprint(ctx, sprint)
        })
        .await?;
        to_call_tool_result(&response)
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
