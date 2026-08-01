use kanban_service::KanbanContext;
use std::sync::Arc;
use tokio::sync::Mutex;
use topcoat::router::RouterBuilderDiscoverExt;

pub type SharedCtx = Arc<Mutex<KanbanContext>>;

pub fn router(ctx: SharedCtx) -> topcoat::router::Router {
    topcoat::router::Router::builder()
        .discover()
        .app_context(ctx)
        .build()
}
