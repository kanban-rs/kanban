pub mod context;
pub mod error;
pub mod server;

pub(crate) mod helpers;
pub mod requests;
pub(crate) mod tools;

pub use error::{KanbanMcpError, KanbanMcpResult};
pub use server::McpServer;

pub use requests::board::{
    ArchiveBoardRequest, CreateBoardRequest, DeleteArchivedBoardRequest, DeleteBoardRequest,
    GetBoardRequest, ListBoardsRequest, RestoreBoardRequest, SetBoardSortRequest,
    UpdateBoardRequest,
};
pub use requests::card::{
    ArchiveCardRequest, ArchiveCardsRequest, AssignCardToSprintRequest, AssignCardsToSprintRequest,
    CreateCardParams, DeleteCardRequest, GetCardBranchNameRequest, GetCardGitCheckoutRequest,
    GetCardRequest, ListCardChildrenRequest, ListCardParentsRequest, ListCardsRequest,
    MoveCardRequest, MoveCardsRequest, RemoveCardParentRequest, RestoreCardRequest,
    SetCardParentRequest, UnassignCardFromSprintRequest, UpdateCardRequest,
};
pub use requests::column::{
    CreateColumnParams, DeleteColumnRequest, GetColumnRequest, ListColumnsRequest,
    ReorderColumnRequest, UpdateColumnRequest,
};
pub use requests::sprint::{
    ActivateSprintRequest, CancelSprintRequest, CarryOverSprintCardsRequest, CompleteSprintRequest,
    CreateSprintParams, DeleteSprintRequest, GetSprintRequest, ListSprintsRequest,
    UpdateSprintRequest,
};
pub use requests::transfer::{ExportBoardRequest, ImportBoardRequest};

use context::McpContext;
use kanban_domain::KanbanResult;
use kanban_service::StoreManager;
use rmcp::{
    handler::server::router::tool::ToolRouter,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    tool_handler, ServerHandler,
};
use std::sync::Arc;
use tokio::sync::Mutex;

// ============================================================================
// MCP Server
// ============================================================================

#[derive(Clone)]
pub struct KanbanMcpServer {
    pub(crate) ctx: Arc<Mutex<McpContext>>,
    tool_router: ToolRouter<Self>,
}

impl KanbanMcpServer {
    pub async fn new(
        store_manager: &StoreManager,
        data_file: &str,
        config: kanban_core::AppConfig,
    ) -> KanbanResult<Self> {
        Ok(Self {
            ctx: Arc::new(Mutex::new(
                McpContext::new(store_manager, data_file, config).await?,
            )),
            tool_router: Self::board_router()
                + Self::column_router()
                + Self::card_crud_router()
                + Self::card_relations_router()
                + Self::card_batch_router()
                + Self::sprint_router()
                + Self::transfer_router(),
        })
    }
}

// ============================================================================
// MCP Server Handler
// ============================================================================

#[tool_handler]
impl ServerHandler for KanbanMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "Kanban MCP Server - Manage your kanban boards, columns, and cards through MCP. \
                 Operates directly on the kanban board through the in-process service layer."
                    .to_string(),
            ),
        }
    }
}
