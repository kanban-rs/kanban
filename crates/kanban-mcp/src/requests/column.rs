use rmcp::schemars;
use serde::Deserialize;

// KAN-794: the bespoke column-create content DTO is gone. The MCP create tool
// resolves the `board` name→id via the shared resolver (the FK is path-supplied
// on the HTTP edge, but MCP takes a name), then funnels the shared
// `kanban_service::api::CreateColumnRequest` content (id + name + wip_limit)
// through `into_new_column` + `Column::create`. The content is flattened in so
// the create fields are not re-derived; `position` is dropped (server-assigned
// append, never client-set at create).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateColumnRequest {
    #[schemars(description = "UUID or name of the board to create the column in")]
    pub board: String,
    #[serde(flatten)]
    pub content: kanban_service::api::CreateColumnRequest,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListColumnsRequest {
    #[schemars(description = "UUID or name of the board to list columns for")]
    pub board: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetColumnRequest {
    #[schemars(description = "UUID or name of the column to retrieve")]
    pub column: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateColumnRequest {
    #[schemars(description = "UUID or name of the column to update")]
    pub column: String,
    #[schemars(description = "New name (optional)")]
    pub name: Option<String>,
    #[schemars(description = "New position (optional)")]
    pub position: Option<i32>,
    #[schemars(description = "WIP limit (optional)")]
    pub wip_limit: Option<u32>,
    #[schemars(description = "Clear the WIP limit")]
    pub clear_wip_limit: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteColumnRequest {
    #[schemars(description = "UUID or name of the column to delete")]
    pub column: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReorderColumnRequest {
    #[schemars(description = "UUID or name of the column to reorder")]
    pub column: String,
    #[schemars(description = "New position")]
    pub position: i32,
}
