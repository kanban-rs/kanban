use crate::error::KanbanMcpResult;
use crate::helpers::{
    locked_read, locked_write, mcp_enrich_add_error, mcp_enrich_remove_error, resolve_summaries,
    to_call_tool_result, to_call_tool_result_json,
};
use crate::requests::card::{
    ListCardChildrenRequest, ListCardParentsRequest, RemoveCardParentRequest, SetCardParentRequest,
};
use crate::KanbanMcpServer;
use kanban_domain::{GraphOperations, KanbanOperations};
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ErrorData as McpError},
    tool, tool_router,
};

#[tool_router(router = card_relations_router, vis = "pub(crate)")]
impl KanbanMcpServer {
    #[tool(
        description = "Add a parent -> child edge between two cards. Rejects cycles and self-references."
    )]
    pub async fn tool_set_card_parent(
        &self,
        Parameters(req): Parameters<SetCardParentRequest>,
    ) -> Result<CallToolResult, McpError> {
        let parent_raw = req.parent.clone();
        let child_raw = req.child.clone();
        let (child_id, parent_id) = locked_write(&self.ctx, |ctx| -> KanbanMcpResult<_> {
            let child_id = ctx.resolve_card_id(&req.child)?;
            let parent_id = ctx.resolve_card_id(&req.parent)?;
            ctx.attach_child(parent_id, child_id)
                .map_err(|e| mcp_enrich_add_error(e, &parent_raw, &child_raw))?;
            Ok((child_id, parent_id))
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({
            "parent": parent_id.to_string(),
            "child":  child_id.to_string(),
        }))
    }

    #[tool(description = "Remove a parent -> child edge between two cards.")]
    pub async fn tool_remove_card_parent(
        &self,
        Parameters(req): Parameters<RemoveCardParentRequest>,
    ) -> Result<CallToolResult, McpError> {
        let parent_raw = req.parent.clone();
        let child_raw = req.child.clone();
        let (child_id, parent_id) = locked_write(&self.ctx, |ctx| -> KanbanMcpResult<_> {
            let child_id = ctx.resolve_card_id(&req.child)?;
            let parent_id = ctx.resolve_card_id(&req.parent)?;
            ctx.detach_child(parent_id, child_id)
                .map_err(|e| mcp_enrich_remove_error(e, &parent_raw, &child_raw))?;
            Ok((child_id, parent_id))
        })
        .await?;
        to_call_tool_result_json(serde_json::json!({
            "parent": parent_id.to_string(),
            "child":  child_id.to_string(),
        }))
    }

    #[tool(description = "List direct parents of a card.")]
    pub async fn tool_list_card_parents(
        &self,
        Parameters(req): Parameters<ListCardParentsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let parents = locked_read(&self.ctx, |ctx| -> KanbanMcpResult<_> {
            let id = ctx.resolve_card_id(&req.card)?;
            let ids = ctx.list_parents_of(id)?;
            Ok(resolve_summaries(ctx, ids))
        })
        .await?;
        to_call_tool_result(&parents)
    }

    #[tool(description = "List direct children of a card.")]
    pub async fn tool_list_card_children(
        &self,
        Parameters(req): Parameters<ListCardChildrenRequest>,
    ) -> Result<CallToolResult, McpError> {
        let children = locked_read(&self.ctx, |ctx| -> KanbanMcpResult<_> {
            let id = ctx.resolve_card_id(&req.card)?;
            let ids = ctx.list_children_of(id)?;
            Ok(resolve_summaries(ctx, ids))
        })
        .await?;
        to_call_tool_result(&children)
    }
}
