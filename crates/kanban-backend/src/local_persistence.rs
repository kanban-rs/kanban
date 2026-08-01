use kanban_persistence::PersistenceMetadata;

/// Optional backend capability: expose metadata about the underlying
/// persistence store (format version, writer kanban version, writer commit,
/// last save time). JSON and SQLite are the only real implementors; local
/// backends without durable storage (InMemory) and remote backends (Http)
/// never override `KanbanBackend::local_persistence()`, so callers must
/// treat the capability as optional rather than assuming every backend has it.
pub trait LocalPersistence: Send + Sync {
    fn persistence_metadata(&self) -> Option<PersistenceMetadata>;
}
