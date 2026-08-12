mod in_memory_store;

pub use in_memory_store::InMemoryStore;

use kanban_backend::{rollback_failed, KanbanBackend, TransactionFn};
use kanban_domain::data_store::DataStore;
use kanban_domain::KanbanResult;

impl KanbanBackend for InMemoryStore {
    fn as_data_store(&self) -> &dyn DataStore {
        self
    }
    // All lifecycle defaults are correct for in-memory: flush=noop, reload=noop,
    // needs_flush=false, needs_save_worker=false.

    fn with_transaction(&self, f: TransactionFn<'_>) -> KanbanResult<()> {
        let before = self.snapshot_impl()?;
        match f() {
            Ok(()) => Ok(()),
            Err(e) => match self.apply_snapshot_impl(before) {
                Err(rollback_err) => Err(rollback_failed(e, rollback_err)),
                Ok(()) => Err(e),
            },
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

    /// Poisons the state lock from inside the batch. `modify_graph_impl` holds
    /// the write guard across its closure, so panicking there leaves the lock
    /// poisoned and the rollback's `apply_snapshot_impl` cannot reacquire it.
    fn poison_state_lock(store: &InMemoryStore) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = store.modify_graph(Box::new(|_| panic!("poison the state lock")));
        }));
    }

    #[test]
    fn test_with_transaction_surfaces_both_errors_when_the_rollback_also_fails() {
        let store = InMemoryStore::new();

        let result = store.with_transaction(Box::new(|| {
            poison_state_lock(&store);
            Err(kanban_domain::KanbanError::Internal("batch boom".into()))
        }));

        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("batch boom"),
            "the batch's own error must survive (got: {msg:?})"
        );
        assert!(
            msg.contains("poisoned"),
            "the rollback failure must be reported too, not swallowed in favour \
             of the batch error (got: {msg:?})"
        );
        assert!(
            msg.contains("State may be inconsistent"),
            "a rollback that failed leaves the store in an unknown state and \
             must say so (got: {msg:?})"
        );
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
