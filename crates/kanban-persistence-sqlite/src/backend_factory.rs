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

    fn matches_locator(&self, locator: &str, header: &[u8]) -> bool {
        header.starts_with(b"SQLite format 3\0")
            || (header.is_empty()
                && (locator.ends_with(".sqlite")
                    || locator.ends_with(".sqlite3")
                    || locator.ends_with(".db")))
    }

    async fn create(
        &self,
        locator: &str,
        _config: &AppConfig,
    ) -> KanbanResult<Arc<dyn KanbanBackend>> {
        Ok(Arc::new(SqliteBackend::open(locator).await?))
    }
}
