use rmcp::schemars;
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListLabelsRequest {
    #[schemars(description = "Page number, 1-based (default: 1)")]
    pub page: Option<u32>,
    #[schemars(description = "Items per page (default: 50)")]
    pub page_size: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetLabelRequest {
    #[schemars(description = "Exact name of the label to retrieve (case-sensitive)")]
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpsertLabelRequest {
    #[schemars(description = "Name of the label (identity key, case-sensitive)")]
    pub name: String,
    #[schemars(description = "CSS-style colour, e.g. '#ff0000' or 'red' (optional)")]
    pub color: Option<String>,
    #[schemars(description = "Free-text description (optional)")]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteLabelRequest {
    #[schemars(description = "Exact name of the label to delete (case-sensitive)")]
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AssignLabelToCardsRequest {
    #[schemars(description = "Exact name of the label to add (case-sensitive)")]
    pub label_name: String,
    #[schemars(description = "Card UUIDs or identifiers (e.g. ['KAN-1', 'KAN-2', '42'])")]
    pub card_ids: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveLabelFromCardsRequest {
    #[schemars(description = "Exact name of the label to remove (case-sensitive)")]
    pub label_name: String,
    #[schemars(description = "Card UUIDs or identifiers (e.g. ['KAN-1', 'KAN-2', '42'])")]
    pub card_ids: Vec<String>,
}
