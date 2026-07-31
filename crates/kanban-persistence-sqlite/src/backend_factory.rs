use crate::SqliteBackend;
use kanban_backend::{KanbanBackend, KanbanBackendFactory};
use kanban_core::AppConfig;
use kanban_domain::KanbanResult;
use std::sync::Arc;

pub struct SqliteBackendFactory;

#[async_trait::async_trait]
impl KanbanBackendFactory for SqliteBackendFactory {
    fn name(&self) -> &str {
        "sqlite"
    }

    async fn create(
        &self,
        locator: &str,
        _config: &AppConfig,
    ) -> KanbanResult<Arc<dyn KanbanBackend>> {
        Ok(Arc::new(SqliteBackend::open(locator).await?))
    }
}
