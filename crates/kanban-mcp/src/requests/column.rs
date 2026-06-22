use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateColumnRequest {
    #[schemars(description = "UUID or name of the board to create the column in")]
    pub board: String,
    #[schemars(description = "Name of the column")]
    pub name: String,
    #[schemars(description = "Position of the column (optional, appends to end if not specified)")]
    pub position: Option<i32>,
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
