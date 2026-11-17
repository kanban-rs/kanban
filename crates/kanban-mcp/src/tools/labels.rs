use crate::helpers::{
    core_err_to_mcp, kanban_err_to_mcp, locked_read, locked_write, to_call_tool_result,
    to_call_tool_result_json,
};
use crate::requests::labels::{
    AssignLabelToCardsRequest, DeleteLabelRequest, GetLabelRequest, ListLabelsRequest,
    RemoveLabelFromCardsRequest, UpsertLabelRequest,
};
use crate::scope::{ToolScope, ToolScoped};
use crate::KanbanMcpServer;
use kanban_core::{resolve_page_params, PaginatedList};
use kanban_domain::{KanbanOperations, Label};
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ErrorData as McpError},
    tool, tool_router,
};

impl ToolScoped for ListLabelsRequest {
    fn scope(&self) -> ToolScope {
        ToolScope::default()
    }
}

impl ToolScoped for GetLabelRequest {
    fn scope(&self) -> ToolScope {
        ToolScope::default()
    }
}

impl ToolScoped for UpsertLabelRequest {
    fn scope(&self) -> ToolScope {
        ToolScope::default()
    }
}

impl ToolScoped for DeleteLabelRequest {
    fn scope(&self) -> ToolScope {
        ToolScope::default()
    }
}

impl ToolScoped for AssignLabelToCardsRequest {
    fn scope(&self) -> ToolScope {
        ToolScope::default()
    }
}

impl ToolScoped for RemoveLabelFromCardsRequest {
    fn scope(&self) -> ToolScope {
        ToolScope::default()
    }
}

#[tool_router(router = label_router, vis = "pub(crate)")]
impl KanbanMcpServer {
    #[tool(
        description = "List all labels in the global registry. Use page/page_size for pagination (default: page=1, page_size=50)."
    )]
    pub async fn tool_list_labels(
        &self,
        Parameters(req): Parameters<ListLabelsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let labels = locked_read(&self.ctx, |ctx| {
            ctx.model_for(&scope);
            ctx.list_labels().map_err(kanban_err_to_mcp)
        })
        .await?;
        let (page, page_size) =
            resolve_page_params(req.page, req.page_size).map_err(core_err_to_mcp)?;
        let paged = PaginatedList::paginate(labels, page, page_size).map_err(core_err_to_mcp)?;
        to_call_tool_result(&paged)
    }

    #[tool(description = "Get a label by its exact name (case-sensitive)")]
    pub async fn tool_get_label(
        &self,
        Parameters(req): Parameters<GetLabelRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let label = locked_read(&self.ctx, |ctx| {
            ctx.model_for(&scope);
            ctx.get_label(&req.name).map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&label)
    }

    #[tool(
        description = "Create or update a label in the global registry (upsert by name)"
    )]
    pub async fn tool_upsert_label(
        &self,
        Parameters(req): Parameters<UpsertLabelRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let label = locked_write(&self.ctx, |ctx| {
            let _ = ctx.model_for(&scope);
            let label = Label {
                name: req.name,
                color: req.color,
                description: req.description,
            };
            ctx.mutate(|c| c.upsert_label_impl(label))
                .map(|(label, _)| label)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result(&label)
    }

    #[tool(description = "Delete a label by its exact name (case-sensitive)")]
    pub async fn tool_delete_label(
        &self,
        Parameters(req): Parameters<DeleteLabelRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        locked_write(&self.ctx, |ctx| -> Result<_, McpError> {
            let _ = ctx.model_for(&scope);
            let _inv = ctx
                .mutate_unit(|c| c.delete_label_impl(req.name.clone()))
                .map_err(kanban_err_to_mcp)?;
            Ok(())
        })
        .await?;
        to_call_tool_result(&())
    }

    #[tool(
        description = "Add a label to multiple cards at once. The label is auto-registered if it does not exist yet. IDs may be UUIDs or identifiers (e.g. 'KAN-1', '42')."
    )]
    pub async fn tool_assign_label_to_cards(
        &self,
        Parameters(req): Parameters<AssignLabelToCardsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let count = locked_write(&self.ctx, |ctx| {
            let _ = ctx.model_for(&scope);
            let ids = ctx
                .resolve_card_ids(&req.card_ids)
                .map_err(kanban_err_to_mcp)?;
            ctx.mutate(|c| c.assign_label_to_cards_impl(req.label_name.clone(), ids))
                .map(|(count, _inv)| count)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"assigned_count": count}))
    }

    #[tool(
        description = "Remove a label from multiple cards at once. The label itself is kept in the registry. IDs may be UUIDs or identifiers (e.g. 'KAN-1', '42')."
    )]
    pub async fn tool_remove_label_from_cards(
        &self,
        Parameters(req): Parameters<RemoveLabelFromCardsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let scope = req.scope();
        let count = locked_write(&self.ctx, |ctx| {
            let _ = ctx.model_for(&scope);
            let ids = ctx
                .resolve_card_ids(&req.card_ids)
                .map_err(kanban_err_to_mcp)?;
            ctx.mutate(|c| c.remove_label_from_cards_impl(req.label_name.clone(), ids))
                .map(|(count, _inv)| count)
                .map_err(kanban_err_to_mcp)
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({"removed_count": count}))
    }
}
