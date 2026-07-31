use crate::KanbanBackend;
use kanban_core::AppConfig;
use kanban_domain::KanbanResult;
use std::sync::Arc;

/// Creates a [`KanbanBackend`] for locators belonging to one backend.
///
/// Implemented beside each backend and registered by the application, so that
/// adding a backend does not require editing the service layer.
#[async_trait::async_trait]
pub trait KanbanBackendFactory: Send + Sync {
    /// Backend this factory builds, e.g. "json", "sqlite", "http", "memory".
    /// Matches `StoreFactory::name()` one layer down.
    fn name(&self) -> &str;

    async fn create(
        &self,
        locator: &str,
        config: &AppConfig,
    ) -> KanbanResult<Arc<dyn KanbanBackend>>;
}

#[derive(Default)]
pub struct KanbanBackendRegistry {
    factories: Vec<Box<dyn KanbanBackendFactory>>,
}

impl KanbanBackendRegistry {
    pub fn new() -> Self {
        Self {
            factories: Vec::new(),
        }
    }

    pub fn register(&mut self, factory: Box<dyn KanbanBackendFactory>) {
        self.factories.push(factory);
    }

    pub fn is_empty(&self) -> bool {
        self.factories.is_empty()
    }

    pub fn names(&self) -> Vec<&str> {
        self.factories.iter().map(|f| f.name()).collect()
    }

    /// First registration wins when two factories claim the same name.
    pub fn for_name(&self, name: &str) -> Option<&dyn KanbanBackendFactory> {
        self.factories
            .iter()
            .map(|f| f.as_ref())
            .find(|f| f.name() == name)
    }
}
