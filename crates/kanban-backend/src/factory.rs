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

    /// True if this factory should handle `locator`. `header` is up to the
    /// first 32 bytes of the file at `locator` if it exists and is readable
    /// (empty otherwise) — mirrors `StoreFactory::matches_content` one layer
    /// down (`kanban_persistence::registry`). Default: never matches by
    /// content; only reachable via `for_name`.
    fn matches_locator(&self, _locator: &str, _header: &[u8]) -> bool {
        false
    }

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

    /// First-registration-wins, same contract as `for_name`. Reads the
    /// locator's header once and asks each factory in registration order.
    pub fn for_locator(&self, locator: &str) -> Option<&dyn KanbanBackendFactory> {
        let header = read_header_best_effort(locator, 32);
        self.factories
            .iter()
            .map(|f| f.as_ref())
            .find(|f| f.matches_locator(locator, &header))
    }
}

fn read_header_best_effort(locator: &str, n: usize) -> Vec<u8> {
    let path = std::path::Path::new(locator);
    if !path.exists() {
        return Vec::new();
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    use std::io::Read;
    let mut buf = vec![0u8; n];
    let read = file.read(&mut buf).unwrap_or(0);
    buf.truncate(read);
    buf
}
