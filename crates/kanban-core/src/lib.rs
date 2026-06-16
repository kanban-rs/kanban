pub mod app_type;
pub use app_type::AppType;

pub mod client_id;
pub use client_id::ClientId;

pub mod config;
pub mod datetime_input;
pub mod error;
pub mod graph;
pub mod health;
pub mod input;
pub mod logging;
pub mod paginated_list;
pub mod pagination;
pub mod selection;
pub mod traits;
pub mod version;

pub use config::{
    validate_branch_prefix, AppConfig, DEFAULT_JSON_FILENAME, DEFAULT_SQLITE_FILENAME,
    DEFAULT_STORAGE_BACKEND,
};
pub use datetime_input::parse_datetime_input;
pub use error::{CoreError, CoreResult};
pub use graph::{
    Cascadable, DagGraph, Directed, Edge, EdgeBase, EdgeSet, EdgeStore, Graph, GraphError,
    GraphNode, Undirected, UndirectedGraph,
};
pub use health::{HealthChecker, HealthStatus};
pub use input::InputState;
pub use logging::{LogEntry, Loggable};
pub use paginated_list::{
    resolve_page_params, PaginatedList, DEFAULT_PAGE, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE,
};
pub use pagination::{Page, PageInfo};
pub use selection::SelectionState;
pub use traits::Editable;
pub use version::{CLI_VERSION_DISPLAY, KANBAN_COMMIT, KANBAN_VERSION};
