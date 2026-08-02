//! Contract tests for `StoreManager`'s dependency-inversion surface.
//!
//! These tests drive the public API that third-party backend crates will
//! consume: build an explicit `StoreRegistry`, wrap it in a `StoreManager`,
//! and confirm that registration order and locator dispatch behave as
//! advertised.

use kanban_backend::KanbanBackendRegistry;
use kanban_persistence::StoreRegistry;
use kanban_persistence_json::{JsonBackendFactory, JsonStoreFactory};
use kanban_service::StoreManager;
use std::sync::Arc;

#[test]
fn test_store_manager_make_store_returns_expected_path() {
    let mut registry = StoreRegistry::new();
    registry.register(Box::new(JsonStoreFactory));
    let manager = StoreManager::new(registry, KanbanBackendRegistry::new());

    let store = manager
        .make_store("json", "/tmp/kan260_injection.json")
        .unwrap();
    assert_eq!(store.path().to_str().unwrap(), "/tmp/kan260_injection.json");
}

#[test]
fn test_store_manager_unknown_backend_is_rejected() {
    let mut registry = StoreRegistry::new();
    registry.register(Box::new(JsonStoreFactory));
    let manager = StoreManager::new(registry, KanbanBackendRegistry::new());

    match manager.make_store("postgres", "/tmp/whatever.sql") {
        Ok(_) => panic!("expected missing-backend error"),
        Err(err) => assert!(
            err.to_string().contains("No backend registered for"),
            "expected missing-backend error, got: {err}"
        ),
    }
}

#[test]
fn test_store_manager_has_backends_returns_false_when_empty() {
    let manager = StoreManager::new(StoreRegistry::new(), KanbanBackendRegistry::new());
    assert!(
        !manager.has_backends(),
        "empty registry must report no backends"
    );
}

#[test]
fn test_store_manager_has_backends_returns_true_after_registration() {
    let mut registry = StoreRegistry::new();
    registry.register(Box::new(JsonStoreFactory));
    let mut backends = KanbanBackendRegistry::new();
    backends.register(Box::new(JsonBackendFactory));
    let manager = StoreManager::new(registry, backends);
    assert!(
        manager.has_backends(),
        "registry with one factory must report has_backends"
    );
}

#[test]
fn test_store_manager_has_backends_reflects_registration_count() {
    let empty_manager = StoreManager::new(StoreRegistry::new(), KanbanBackendRegistry::new());
    assert!(!empty_manager.has_backends());

    let mut registry = StoreRegistry::new();
    registry.register(Box::new(JsonStoreFactory));
    let mut backends = KanbanBackendRegistry::new();
    backends.register(Box::new(JsonBackendFactory));
    let full_manager = StoreManager::new(registry, backends);
    assert!(full_manager.has_backends());
}

/// `has_backends()` must reflect the `KanbanBackendRegistry` (what
/// `make_backend` actually dispatches through), not the `StoreRegistry` —
/// they are two independent registries `StoreManager::new` accepts as two
/// separate parameters, and nothing enforces they stay in sync. This is the
/// divergent case `register_backend()`'s paired-factory contract prevents at
/// its own call sites, but `StoreManager::new` itself does not.
#[test]
fn test_store_manager_has_backends_true_when_only_backend_registry_populated() {
    let mut backends = KanbanBackendRegistry::new();
    backends.register(Box::new(JsonBackendFactory));
    let manager = StoreManager::new(StoreRegistry::new(), backends);
    assert!(
        manager.has_backends(),
        "a populated KanbanBackendRegistry alone must report has_backends, \
         even with an empty StoreRegistry"
    );
}

#[test]
fn test_store_manager_clone_shares_registry() {
    // Cloning a StoreManager must produce a handle that can build the same
    // stores — the underlying Arc<StoreRegistry> is shared, not copied.
    let mut registry = StoreRegistry::new();
    registry.register(Box::new(JsonStoreFactory));
    let mut backends = KanbanBackendRegistry::new();
    backends.register(Box::new(JsonBackendFactory));
    let original = StoreManager::new(registry, backends);
    let cloned = original.clone();

    // Both handles must see the same backends.
    assert!(original.has_backends());
    assert!(cloned.has_backends());

    // Both must produce a store for the same locator.
    let store_a = original.make_store("json", "/tmp/clone_a.json").unwrap();
    let store_b = cloned.make_store("json", "/tmp/clone_b.json").unwrap();

    // instance_ids are per-handle, but both paths must be set correctly.
    assert!(store_a.path().to_str().unwrap().ends_with(".json"));
    assert!(store_b.path().to_str().unwrap().ends_with(".json"));
}

#[test]
fn test_store_manager_registry_exposes_arc_sharing_semantics() {
    // StoreManager owns Arc<StoreRegistry>; callers should be able to peek
    // at the registry without consuming the manager.
    let mut registry = StoreRegistry::new();
    registry.register(Box::new(JsonStoreFactory));
    let manager = StoreManager::new(registry, KanbanBackendRegistry::new());

    let _: &StoreRegistry = manager.registry();

    // Build a store and hold two Arc references to verify typical usage.
    let store: Arc<dyn kanban_persistence::PersistenceStore + Send + Sync> = manager
        .make_store("json", "/tmp/kan260_arc_check.json")
        .unwrap();
    let cloned = Arc::clone(&store);
    assert_eq!(store.instance_id(), cloned.instance_id());
}
