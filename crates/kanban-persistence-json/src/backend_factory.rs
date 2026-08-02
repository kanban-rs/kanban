use crate::{JsonDataStore, JsonFileStore};
use kanban_backend::{KanbanBackend, KanbanBackendFactory};
use kanban_core::AppConfig;
use kanban_domain::KanbanResult;
use kanban_persistence::PersistenceStore;
use std::sync::Arc;

pub struct JsonBackendFactory;

#[async_trait::async_trait]
impl KanbanBackendFactory for JsonBackendFactory {
    fn name(&self) -> &str {
        "json"
    }

    fn matches_locator(&self, _locator: &str, header: &[u8]) -> bool {
        let trimmed = header.iter().find(|b| !b.is_ascii_whitespace());
        header.is_empty() || matches!(trimmed, Some(b'{') | Some(b'['))
    }

    async fn create(
        &self,
        locator: &str,
        _config: &AppConfig,
    ) -> KanbanResult<Arc<dyn KanbanBackend>> {
        let store: Arc<dyn PersistenceStore + Send + Sync> = Arc::new(JsonFileStore::new(locator));
        Ok(Arc::new(JsonDataStore::new(store)))
    }
}
