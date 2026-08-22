use super::KanbanContext;
use crate::cache::EntityCache;
use kanban_domain::{FetchPlan, Invalidation, KanbanResult, Resolved};

impl KanbanContext {
    /// Opt in to per-entity caching. A context built without this call holds
    /// no cache and resolves nothing.
    pub fn with_entity_cache(mut self) -> Self {
        self.cache = Some(EntityCache::new());
        self
    }

    pub fn has_cache(&self) -> bool {
        self.cache.is_some()
    }

    /// Drops the named entities from the cache. A no-op when no cache is
    /// configured.
    pub fn invalidate(&mut self, invalidation: Invalidation) {
        if let Some(cache) = self.cache.as_mut() {
            cache.invalidate(invalidation);
        }
    }

    /// Runs `plan` against the cache, fetching from the backend whatever the
    /// plan still needs. The result names only what this pass fetched, so a
    /// pass that fetched nothing returns `Resolved::default()`.
    ///
    /// Total: a context with no cache configured is a supported
    /// configuration, so it returns `Resolved::default()` rather than an
    /// error. Callers that must branch on it use
    /// [`has_cache`](Self::has_cache).
    pub fn resolve(&mut self, plan: &dyn FetchPlan) -> KanbanResult<Resolved> {
        let backend = std::sync::Arc::clone(&self.backend);
        match self.cache.as_mut() {
            Some(cache) => cache.resolve(plan, backend.as_data_store()),
            None => Ok(Resolved::default()),
        }
    }
}

#[cfg(test)]
mod tests;
