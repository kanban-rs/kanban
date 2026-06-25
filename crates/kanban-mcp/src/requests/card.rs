use rmcp::schemars;
use serde::Deserialize;

// KAN-796: the bespoke card-create content DTO is gone. The MCP create tool
// resolves the `board`/`column`/`sprint` name-or-id references (the column FK is
// path-supplied on the HTTP edge, but MCP takes names), then funnels the shared
// `kanban_service::api::CreateCardRequest` content (id + title + description +
// priority + due_date + points + sprint_id) through `into_new_card(column_id)` +
// `Card::create`. The content is flattened in so the create fields are not
// re-derived; the loose `sprint` name-or-id, when present, resolves to the
// shared content's `sprint_id` before conversion.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateCardRequest {
    #[schemars(description = "UUID or name of the board")]
    pub board: String,
    #[schemars(description = "UUID or name of the column to create the card in")]
    pub column: String,
    #[schemars(
        description = "UUID, name, or number of the sprint to assign the new card to (optional). \
            If the board has exactly one Active (non-ended) sprint, prefer passing that \
            sprint's id here so the card lands in the active sprint in a single call."
    )]
    pub sprint: Option<String>,
    #[serde(flatten)]
    pub content: kanban_service::api::CreateCardRequest,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListCardsRequest {
    #[schemars(description = "Filter cards by board UUID or name")]
    pub board: Option<String>,
    #[schemars(
        description = "Filter cards by column UUID or name (scoped to board if given, else global)"
    )]
    pub column: Option<String>,
    #[schemars(
        description = "Filter cards by sprint UUID, name, or number (scoped to board if given, else global)"
    )]
    pub sprint: Option<String>,
    #[schemars(description = "Filter by status: 'todo', 'in_progress', 'blocked', or 'done'")]
    pub status: Option<String>,
    #[schemars(
        description = "Sort field. Valid: points, priority, created_at, updated_at, due_date, status, position, default. 'default' orders by card number; date fields and points place None values last in ascending order. When omitted, falls back to the board's task_sort_field (requires `board`)."
    )]
    pub sort: Option<String>,
    #[schemars(
        description = "Sort direction: 'asc' or 'desc'. Defaults to the board's task_sort_order."
    )]
    pub order: Option<String>,
    #[schemars(description = "Page number, 1-based (default: 1)")]
    pub page: Option<u32>,
    #[schemars(description = "Items per page (default: 50)")]
    pub page_size: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListArchivedCardsRequest {
    #[schemars(
        description = "Filter archives by board UUID or name (also drives the default sort field)"
    )]
    pub board: Option<String>,
    #[schemars(
        description = "Sort field. Valid: points, priority, created_at, updated_at, due_date, status, position, default. 'default' orders by card number; date fields and points place None values last in ascending order. Falls back to the board's task_sort_field when omitted."
    )]
    pub sort: Option<String>,
    #[schemars(
        description = "Sort direction: 'asc' or 'desc'. Defaults to the board's task_sort_order."
    )]
    pub order: Option<String>,
    #[schemars(description = "Page number, 1-based (default: 1)")]
    pub page: Option<u32>,
    #[schemars(description = "Items per page (default: 50)")]
    pub page_size: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetCardRequest {
    #[schemars(description = "UUID or identifier of the card to retrieve (e.g. 'KAN-5' or '5')")]
    pub card: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UpdateCardRequest {
    #[schemars(description = "UUID or identifier of the card to update (e.g. 'KAN-5' or '5')")]
    pub card: String,
    #[schemars(description = "New title (optional)")]
    pub title: Option<String>,
    #[schemars(description = "New description (optional)")]
    pub description: Option<String>,
    #[schemars(description = "Priority: 'low', 'medium', 'high', or 'critical' (optional)")]
    pub priority: Option<String>,
    #[schemars(description = "Status: 'todo', 'in_progress', 'blocked', or 'done' (optional)")]
    pub status: Option<String>,
    #[schemars(
        description = "Due date in YYYY-MM-DD or RFC 3339 format (e.g. 2024-06-15 or 2024-06-15T10:30:00Z), use clear_due_date to remove"
    )]
    pub due_date: Option<String>,
    #[schemars(description = "Clear due date (set to true to remove due date)")]
    pub clear_due_date: Option<bool>,
    #[schemars(description = "Story points (optional, 0-255)")]
    pub points: Option<u8>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MoveCardRequest {
    #[schemars(description = "UUID or identifier of the card to move (e.g. 'KAN-5' or '5')")]
    pub card: String,
    #[schemars(
        description = "UUID or name of the destination column (resolved within the card's board)"
    )]
    pub column: String,
    #[schemars(description = "Position in the new column (optional)")]
    pub position: Option<i32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ArchiveCardRequest {
    #[schemars(description = "UUID or identifier of the card to archive (e.g. 'KAN-5' or '5')")]
    pub card: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RestoreCardRequest {
    #[schemars(
        description = "UUID or identifier of the archived card to restore (e.g. 'KAN-5' or '5')"
    )]
    pub card: String,
    #[schemars(
        description = "UUID or name of the column to restore the card to (optional; resolved within the card's board)"
    )]
    pub column: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteCardRequest {
    #[schemars(description = "UUID or identifier of the card to delete (e.g. 'KAN-5' or '5')")]
    pub card: String,
}

// Card Sprint

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AssignCardToSprintRequest {
    #[schemars(description = "UUID or identifier of the card (e.g. 'KAN-5' or '5')")]
    pub card: String,
    #[schemars(
        description = "UUID, name, or number of the sprint to assign to (resolved within the card's board)"
    )]
    pub sprint: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct UnassignCardFromSprintRequest {
    #[schemars(
        description = "UUID or identifier of the card to unassign from its sprint (e.g. 'KAN-5' or '5')"
    )]
    pub card: String,
}

// Card Utilities

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetCardBranchNameRequest {
    #[schemars(description = "UUID or identifier of the card (e.g. 'KAN-5' or '5')")]
    pub card: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetCardGitCheckoutRequest {
    #[schemars(description = "UUID or identifier of the card (e.g. 'KAN-5' or '5')")]
    pub card: String,
}

// Card relations (parent/child)

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetCardParentRequest {
    #[schemars(description = "UUID or identifier of the child card (e.g. 'KAN-5')")]
    pub child: String,
    #[schemars(description = "UUID or identifier of the parent card (e.g. 'KAN-2')")]
    pub parent: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveCardParentRequest {
    #[schemars(description = "UUID or identifier of the child card")]
    pub child: String,
    #[schemars(description = "UUID or identifier of the parent card")]
    pub parent: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListCardParentsRequest {
    #[schemars(description = "UUID or identifier of the card whose parents to list")]
    pub card: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListCardChildrenRequest {
    #[schemars(description = "UUID or identifier of the card whose children to list")]
    pub card: String,
}

// Multi-card operations

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ArchiveCardsRequest {
    #[schemars(description = "Card UUIDs or identifiers (e.g. ['KAN-1', 'KAN-2', '42'])")]
    pub cards: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MoveCardsRequest {
    #[schemars(
        description = "Card UUIDs or identifiers (e.g. ['KAN-1', 'KAN-2']); all cards must share a board"
    )]
    pub cards: Vec<String>,
    #[schemars(
        description = "UUID or name of the destination column (resolved within the cards' shared board)"
    )]
    pub column: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AssignCardsToSprintRequest {
    #[schemars(
        description = "Card UUIDs or identifiers (e.g. ['KAN-1', 'KAN-2']); all cards must share a board"
    )]
    pub cards: Vec<String>,
    #[schemars(
        description = "UUID, name, or number of the sprint to assign to (resolved within the cards' shared board)"
    )]
    pub sprint: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_cards_request_accepts_sort_and_order() {
        let json = serde_json::json!({
            "sort": "due-date",
            "order": "asc",
        });
        let req: ListCardsRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.sort.as_deref(), Some("due-date"));
        assert_eq!(req.order.as_deref(), Some("asc"));
    }

    #[test]
    fn list_archived_cards_request_accepts_sort_and_order() {
        let json = serde_json::json!({
            "sort": "due-date",
            "order": "asc",
        });
        let req: ListArchivedCardsRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.sort.as_deref(), Some("due-date"));
        assert_eq!(req.order.as_deref(), Some("asc"));
    }
}
