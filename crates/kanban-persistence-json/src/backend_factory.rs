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

    async fn create(
        &self,
        locator: &str,
        _config: &AppConfig,
    ) -> KanbanResult<Arc<dyn KanbanBackend>> {
        let store: Arc<dyn PersistenceStore + Send + Sync> = Arc::new(JsonFileStore::new(locator));
        Ok(Arc::new(JsonDataStore::new(store)))
    }
}
