use kanban_backend::{KanbanBackend, KanbanBackendFactory, KanbanBackendRegistry};
use kanban_backend_memory::InMemoryStore;
use kanban_core::AppConfig;
use kanban_domain::KanbanResult;
use std::sync::Arc;

struct MemoryFactory;

#[async_trait::async_trait]
impl KanbanBackendFactory for MemoryFactory {
    fn scheme(&self) -> &str {
        "memory"
    }

    async fn create(
        &self,
        _locator: &str,
        _config: &AppConfig,
    ) -> KanbanResult<Arc<dyn KanbanBackend>> {
        Ok(Arc::new(InMemoryStore::new()))
    }
}

/// Lives here rather than in `kanban-backend` because that crate has no
/// `KanbanBackend` implementation to hand back, and cannot depend on one.
#[tokio::test]
async fn test_factory_creates_backend_for_its_scheme() -> KanbanResult<()> {
    let mut registry = KanbanBackendRegistry::new();
    registry.register(Box::new(MemoryFactory));

    let factory = registry
        .for_scheme("memory")
        .expect("memory factory registered");
    let backend = factory.create("ignored", &AppConfig::default()).await?;

    assert!(backend.list_boards()?.is_empty());
    assert!(backend.remote_writes().is_none());
    Ok(())
}
