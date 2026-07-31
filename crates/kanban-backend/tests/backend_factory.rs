use kanban_backend::{KanbanBackend, KanbanBackendFactory, KanbanBackendRegistry};
use kanban_core::AppConfig;
use kanban_domain::{KanbanError, KanbanResult};
use std::sync::Arc;

struct StubFactory {
    name: &'static str,
}

#[async_trait::async_trait]
impl KanbanBackendFactory for StubFactory {
    fn name(&self) -> &str {
        self.name
    }

    async fn create(
        &self,
        locator: &str,
        _config: &AppConfig,
    ) -> KanbanResult<Arc<dyn KanbanBackend>> {
        Err(KanbanError::validation(format!(
            "{} factory reached with {locator}",
            self.name
        )))
    }
}

fn stub(name: &'static str) -> Box<dyn KanbanBackendFactory> {
    Box::new(StubFactory { name })
}

#[test]
fn test_registry_is_empty_when_no_factories_registered() {
    let registry = KanbanBackendRegistry::new();
    assert!(registry.is_empty());
    assert!(registry.names().is_empty());
}

#[test]
fn test_registry_returns_factory_for_registered_name() {
    let mut registry = KanbanBackendRegistry::new();
    registry.register(stub("file"));

    assert!(!registry.is_empty());
    let found = registry.for_name("file").expect("file factory registered");
    assert_eq!(found.name(), "file");
}

#[test]
fn test_registry_returns_none_for_unregistered_name() {
    let mut registry = KanbanBackendRegistry::new();
    registry.register(stub("file"));

    assert!(registry.for_name("http").is_none());
}

#[test]
fn test_registry_lists_registered_names() {
    let mut registry = KanbanBackendRegistry::new();
    registry.register(stub("file"));
    registry.register(stub("http"));
    registry.register(stub("memory"));

    assert_eq!(registry.names(), vec!["file", "http", "memory"]);
}

/// Pins duplicate-name resolution so it is a decision rather than an artifact
/// of `Vec` iteration order.
#[test]
fn test_registry_first_registration_wins_for_duplicate_name() {
    struct Marked;

    #[async_trait::async_trait]
    impl KanbanBackendFactory for Marked {
        fn name(&self) -> &str {
            "file"
        }
        async fn create(
            &self,
            _locator: &str,
            _config: &AppConfig,
        ) -> KanbanResult<Arc<dyn KanbanBackend>> {
            Err(KanbanError::validation("second"))
        }
    }

    let mut registry = KanbanBackendRegistry::new();
    registry.register(stub("file"));
    registry.register(Box::new(Marked));

    let found = registry.for_name("file").expect("a file factory resolves");
    let err = tokio_test_block_on(found.create("x", &AppConfig::default()))
        .err()
        .expect("stub factories always fail");
    assert!(
        err.to_string().contains("file factory reached"),
        "the first registration must win, got: {err}"
    );
}

fn tokio_test_block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime")
        .block_on(f)
}

/// The registry must be able to dispatch a locator plus config to a factory and
/// receive the factory's own error, proving the async signature is usable as
/// `StoreManager::make_backend` needs it.
#[tokio::test]
async fn test_registry_dispatches_locator_and_config_to_the_matching_factory() {
    let mut registry = KanbanBackendRegistry::new();
    registry.register(stub("file"));

    let factory = registry.for_name("file").expect("file factory registered");
    let err = factory
        .create("/tmp/board.json", &AppConfig::default())
        .await
        .err()
        .expect("stub factory always fails");

    assert!(err.to_string().contains("/tmp/board.json"));
}
