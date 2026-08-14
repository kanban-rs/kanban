use rmcp::schemars;
use serde::Deserialize;

// KAN-794: there is no bespoke column-create content DTO. The shared
// `kanban_service::api::CreateColumnRequest` (id + name + wip_limit) is the
// single source of truth for the create fields and is flattened in below, so
// none of them are re-declared here. This thin params wrapper only adds the
// MCP-only `board` name-or-id: on the HTTP edge that FK is path-supplied, but
// MCP has no path, so the create tool resolves it via the shared resolver and
// funnels the flattened content through `into_new_column` + `Column::create`.
// `position` is not on the wire (server-assigned append).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateColumnParams {
    #[schemars(description = "UUID or name of the board to create the column in")]
    pub board: String,
    #[serde(flatten)]
    pub content: kanban_service::api::CreateColumnRequest,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListColumnsRequest {
    #[schemars(description = "UUID or name of the board to list columns for")]
    pub board: String,
    #[schemars(description = "Page number, 1-based (default: 1)")]
    pub page: Option<u32>,
    #[schemars(description = "Items per page (default: 50)")]
    pub page_size: Option<u32>,
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
    #[schemars(
        description = "Status a card takes when moved into this column (optional): todo, in_progress, blocked, done"
    )]
    pub default_status: Option<kanban_service::api::CardStatusDto>,
    #[schemars(description = "Clear the default_status")]
    pub clear_default_status: Option<bool>,
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
