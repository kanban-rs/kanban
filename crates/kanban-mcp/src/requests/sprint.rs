use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateSprintRequest {
    #[schemars(description = "UUID or name of the board")]
    pub board: String,
    #[schemars(description = "Sprint prefix (optional)")]
    pub prefix: Option<String>,
    #[schemars(description = "Sprint name (optional)")]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListSprintsRequest {
    #[schemars(description = "UUID or name of the board")]
    pub board: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSprintRequest {
    #[schemars(description = "UUID, name, or number of the sprint")]
    pub sprint: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateSprintRequest {
    #[schemars(description = "UUID, name, or number of the sprint to update")]
    pub sprint: String,
    #[schemars(description = "New sprint name (optional)")]
    pub name: Option<String>,
    #[schemars(description = "New prefix (optional)")]
    pub prefix: Option<String>,
    #[schemars(description = "New card prefix (optional)")]
    pub card_prefix: Option<String>,
    #[schemars(description = "New start date in YYYY-MM-DD or RFC 3339 format (optional)")]
    pub start_date: Option<String>,
    #[schemars(description = "New end date in YYYY-MM-DD or RFC 3339 format (optional)")]
    pub end_date: Option<String>,
    #[schemars(description = "Clear the start date")]
    pub clear_start_date: Option<bool>,
    #[schemars(description = "Clear the end date")]
    pub clear_end_date: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ActivateSprintRequest {
    #[schemars(description = "UUID, name, or number of the sprint to activate")]
    pub sprint: String,
    #[schemars(description = "Duration in days (optional)")]
    pub duration_days: Option<i32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CompleteSprintRequest {
    #[schemars(description = "UUID, name, or number of the sprint to complete")]
    pub sprint: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CancelSprintRequest {
    #[schemars(description = "UUID, name, or number of the sprint to cancel")]
    pub sprint: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteSprintRequest {
    #[schemars(description = "UUID, name, or number of the sprint to delete")]
    pub sprint: String,
}

// Carry-over

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CarryOverSprintCardsRequest {
    #[schemars(
        description = "UUID, name, or number of the completed/cancelled source sprint to carry cards from"
    )]
    pub from_sprint: String,
    #[schemars(
        description = "UUID, name, or number of the planning sprint to carry cards to (must be on the same board as source)"
    )]
    pub to_sprint: String,
}
