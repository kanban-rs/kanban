use crate::context::McpContext;
use crate::helpers::error_mapping::kanban_err_to_mcp;
use kanban_domain::{CardSummary, KanbanError, KanbanOperations, Sprint};
use kanban_service::api::SprintResponse;
use rmcp::model::ErrorData as McpError;
use uuid::Uuid;

pub(crate) fn resolve_summaries(ctx: &McpContext, ids: Vec<Uuid>) -> Vec<CardSummary> {
    ids.into_iter()
        .filter_map(|id| match ctx.get_card(id) {
            Ok(Some(c)) => Some(CardSummary::from(&c)),
            Ok(None) => {
                // require_card_exists guards add/remove, so reaching
                // this branch in production indicates a dangling edge:
                // the graph references a card that no longer exists.
                // Log so an operator can see and investigate.
                tracing::warn!(
                    "graph references unknown card id {id}; dropping from summary list (possible corruption)"
                );
                None
            }
            Err(e) => {
                tracing::warn!(
                    "failed to resolve card id {id} for summary: {e}; dropping from list"
                );
                None
            }
        })
        .collect()
}

/// Helper trait: gives `&McpContext` access to MCP-flavoured error mapping for
/// the resolvers it inherits via `KanbanOperations`. Each method is a thin
/// `kanban_err_to_mcp` shim so closure bodies inside `locked_read` /
/// `locked_write` stay readable.
pub(crate) trait McpResolve {
    fn mcp_resolve_board(&self, raw: &str) -> Result<Uuid, McpError>;
    /// Resolve a board id from EITHER the live or the archived view. UUIDs
    /// resolve immediately; a live-board name resolves via the standard resolver;
    /// otherwise fall back to matching an archived board by name (its head is
    /// unfiltered via `get_board`). Mirrors the CLI's `resolve_board_id_any` (I4).
    fn mcp_resolve_board_any(&self, raw: &str) -> Result<Uuid, McpError>;
    fn mcp_resolve_column_in_board(&self, raw: &str, board_id: Uuid) -> Result<Uuid, McpError>;
    fn mcp_resolve_column_global(&self, raw: &str) -> Result<Uuid, McpError>;
    fn mcp_resolve_sprint_in_board(&self, raw: &str, board_id: Uuid) -> Result<Uuid, McpError>;
    fn mcp_resolve_sprint_global(&self, raw: &str) -> Result<Uuid, McpError>;
    fn mcp_resolve_card(&self, raw: &str) -> Result<Uuid, McpError>;
    fn mcp_resolve_cards(&self, raws: &[String]) -> Result<Vec<Uuid>, McpError>;
    fn mcp_require_same_board(&self, card_ids: &[Uuid]) -> Result<Uuid, McpError>;
}

impl McpResolve for McpContext {
    fn mcp_resolve_board(&self, raw: &str) -> Result<Uuid, McpError> {
        self.resolve_board_id(raw).map_err(kanban_err_to_mcp)
    }
    fn mcp_resolve_board_any(&self, raw: &str) -> Result<Uuid, McpError> {
        if let Ok(uuid) = Uuid::parse_str(raw) {
            return Ok(uuid);
        }
        // Live boards first (the domain resolver is live-only).
        if let Ok(id) = self.resolve_board_id(raw) {
            return Ok(id);
        }
        // Fall back to archived boards: resolve each marker's live head (get_board
        // is unfiltered) and match by name.
        let mut matches: Vec<Uuid> = Vec::new();
        for marker in self.list_archived_boards().map_err(kanban_err_to_mcp)? {
            if let Some(board) = self
                .get_board(marker.entity_id)
                .map_err(kanban_err_to_mcp)?
            {
                if board.name == raw {
                    matches.push(board.id);
                }
            }
        }
        match matches.as_slice() {
            [id] => Ok(*id),
            [] => Err(McpError::invalid_params(
                format!("Board not found: '{raw}'"),
                None,
            )),
            _ => Err(McpError::invalid_params(
                format!("Ambiguous archived board name: '{raw}'"),
                None,
            )),
        }
    }
    fn mcp_resolve_column_in_board(&self, raw: &str, board_id: Uuid) -> Result<Uuid, McpError> {
        self.resolve_column_id(raw, board_id)
            .map_err(kanban_err_to_mcp)
    }
    fn mcp_resolve_column_global(&self, raw: &str) -> Result<Uuid, McpError> {
        self.resolve_column_id_global(raw)
            .map_err(kanban_err_to_mcp)
    }
    fn mcp_resolve_sprint_in_board(&self, raw: &str, board_id: Uuid) -> Result<Uuid, McpError> {
        self.resolve_sprint_id(raw, board_id)
            .map_err(kanban_err_to_mcp)
    }
    fn mcp_resolve_sprint_global(&self, raw: &str) -> Result<Uuid, McpError> {
        self.resolve_sprint_id_global(raw)
            .map_err(kanban_err_to_mcp)
    }
    fn mcp_resolve_card(&self, raw: &str) -> Result<Uuid, McpError> {
        self.resolve_card_id(raw).map_err(kanban_err_to_mcp)
    }
    fn mcp_resolve_cards(&self, raws: &[String]) -> Result<Vec<Uuid>, McpError> {
        self.resolve_card_ids(raws).map_err(kanban_err_to_mcp)
    }
    fn mcp_require_same_board(&self, card_ids: &[Uuid]) -> Result<Uuid, McpError> {
        self.require_same_board(card_ids).map_err(kanban_err_to_mcp)
    }
}

/// Derive a card's board via card → column → board, with MCP-flavoured error
/// mapping. Standalone (not on the resolver trait) because it composes
/// multiple trait calls rather than being a simple error-mapping shim.
pub(crate) fn card_board(ctx: &McpContext, card_id: Uuid) -> Result<Uuid, McpError> {
    let card = ctx
        .get_card(card_id)
        .map_err(kanban_err_to_mcp)?
        .ok_or_else(|| McpError::invalid_params(format!("Card not found: {}", card_id), None))?;
    let column = ctx
        .get_column(card.column_id)
        .map_err(kanban_err_to_mcp)?
        .ok_or_else(|| {
            McpError::invalid_params(format!("Column not found: {}", card.column_id), None)
        })?;
    Ok(column.board_id)
}

/// Project a domain `Sprint` into its v1 `SprintResponse`, resolving the
/// owning board so the wire `name` can be looked up against the board's name
/// pool (the internal `name_index` is never exposed). Used by the sprint
/// mutating tools, whose service calls return a raw `Sprint`.
pub(crate) fn project_sprint(ctx: &McpContext, sprint: Sprint) -> Result<SprintResponse, McpError> {
    let board = ctx
        .get_board(sprint.board_id)
        .map_err(kanban_err_to_mcp)?
        .ok_or_else(|| kanban_err_to_mcp(KanbanError::not_found("Board", sprint.board_id)))?;
    Ok(SprintResponse::from_sprint(&sprint, &board))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{CreateCardOptions, KanbanOperations};
    use kanban_service::StoreManager;

    // resolve_summaries: graph-vs-store divergence

    /// `resolve_summaries` silently filters ids whose card no longer
    /// exists in the store. This documents the behaviour: if the
    /// graph references a dangling id (e.g. a cascade race or a
    /// hand-edited file), the list_card_* tools return fewer entries
    /// than the graph reports rather than erroring. The caller may
    /// see N parents on a child while only M < N appear in the
    /// rendered summaries.
    #[tokio::test]
    async fn resolve_summaries_silently_drops_ids_not_present_in_store() {
        use kanban_core::AppConfig;
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.json");
        let store_manager = StoreManager::new(kanban_service::default_registry());
        let mut ctx = McpContext::new(
            &store_manager,
            &path.to_string_lossy(),
            AppConfig::default(),
        )
        .await
        .unwrap();

        let board = ctx.create_board("Test".into(), Some("TST".into())).unwrap();
        let column = ctx.create_column(board.id, "TODO".into(), None).unwrap();
        let card = ctx
            .create_card(
                board.id,
                column.id,
                "Real".into(),
                CreateCardOptions::default(),
            )
            .unwrap();

        let ghost = Uuid::new_v4();
        let summaries = resolve_summaries(&ctx, vec![card.id, ghost]);

        assert_eq!(
            summaries.len(),
            1,
            "ghost id with no backing card should be silently filtered; got {summaries:?}"
        );
        assert_eq!(summaries[0].id, card.id);
    }
}
