use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExportBoardRequest {
    #[schemars(
        description = "UUID or name of the board to export (optional, exports all if omitted)"
    )]
    pub board: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ImportBoardRequest {
    #[schemars(description = "JSON data to import (full board export format)")]
    pub data: String,
}
