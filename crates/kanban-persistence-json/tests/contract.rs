use kanban_persistence_json::JsonDataStore;
use kanban_persistence_json::JsonFileStore;
use std::sync::Arc;

fn json_store_factory() -> kanban_persistence::test_helpers::StoreFactory {
    Box::new(|path| Arc::new(JsonFileStore::new(path)))
}

fn json_backend_factory() -> kanban_service::test_helpers::BackendFactory {
    Box::new(|path| Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path)))))
}

mod tier1 {
    kanban_persistence::store_contract_tests!(super::json_store_factory);
}

mod tier2 {
    kanban_service::context_contract_tests!(super::json_backend_factory);
}
