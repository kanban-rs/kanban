/// Builds the `StoreManager` with every store and backend factory the
/// server supports registered, in the order that decides which factory
/// claims an ambiguous locator. This is the single place kanban-server
/// derives a backend-kind decision from; every caller that needs to know
/// whether a locator is SQLite or JSON goes through the manager this
/// returns.
pub fn registered_store_manager() -> kanban_service::StoreManager {
    let mut stores = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    stores.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    kanban_service::StoreManager::new(stores, backends)
}
