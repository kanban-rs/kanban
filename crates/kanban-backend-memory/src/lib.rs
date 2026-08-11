mod in_memory_store;

pub use in_memory_store::InMemoryStore;

use kanban_backend::KanbanBackend;
use kanban_domain::data_store::DataStore;
use kanban_domain::{KanbanError, KanbanResult};

impl KanbanBackend for InMemoryStore {
    fn as_data_store(&self) -> &dyn DataStore {
        self
    }
    // All lifecycle defaults are correct for in-memory: flush=noop, reload=noop,
    // needs_flush=false, needs_save_worker=false.

    fn with_transaction(&self, f: &mut dyn FnMut() -> KanbanResult<()>) -> KanbanResult<()> {
        let before = self.snapshot_impl()?;
        match f() {
            Ok(()) => Ok(()),
            Err(e) => {
                if let Err(rollback_err) = self.apply_snapshot_impl(before) {
                    return Err(KanbanError::Internal(format!(
                        "Batch failed ({e}) and rollback also failed ({rollback_err}). State may be inconsistent."
                    )));
                }
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kanban_backend_is_object_safe() {
        let store = InMemoryStore::new();
        let _: &dyn KanbanBackend = &store;
    }

    #[test]
    fn test_as_data_store_returns_data_store_ref() {
        let store = InMemoryStore::new();
        let backend: &dyn KanbanBackend = &store;
        let _: &dyn DataStore = backend.as_data_store();
    }

    #[test]
    fn test_in_memory_store_needs_flush_returns_false() {
        let store = InMemoryStore::new();
        assert!(!store.needs_flush());
    }

    #[test]
    fn test_in_memory_store_needs_save_worker_returns_false() {
        let store = InMemoryStore::new();
        assert!(!store.needs_save_worker());
    }

    #[tokio::test]
    async fn test_in_memory_store_flush_is_noop() {
        let store = InMemoryStore::new();
        store.flush().await.expect("flush should be a no-op");
    }

    #[tokio::test]
    async fn test_in_memory_store_reload_is_noop() {
        let store = InMemoryStore::new();
        store.reload().await.expect("reload should be a no-op");
    }

    #[test]
    fn test_in_memory_backend_returns_none_local_persistence() {
        let store = InMemoryStore::new();
        let backend: &dyn KanbanBackend = &store;
        assert!(backend.local_persistence().is_none());
    }

    #[test]
    fn test_in_memory_backend_health_checker_returns_none() {
        let store = InMemoryStore::new();
        let backend: &dyn KanbanBackend = &store;
        assert!(backend.health_checker().is_none());
    }

    #[test]
    fn test_remote_writes_defaults_to_none_for_local_backends() {
        let store = InMemoryStore::new();
        let backend: &dyn KanbanBackend = &store;
        assert!(backend.remote_writes().is_none());
    }
}
