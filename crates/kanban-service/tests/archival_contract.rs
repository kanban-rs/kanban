//! FOUND-F4 (KAN-873): run the shared `KanbanContext` contract suite on ALL
//! THREE backends — in-memory, JSON, and SQLite — so board/column/card/sprint/
//! archive/edge/lifecycle round-trips (incl. the F3b reference-marker archival
//! model and an edit-while-archived round-trip) are held to ONE spec everywhere.
//!
//! The `context_contract_tests!` macro generates a `#[tokio::test]` per contract
//! function; invoking it inside three modules gives 3× the coverage.
//!
//! Requires the `test-helpers` feature (which exposes `test_helpers` and the
//! contract macro). CI runs `cargo test --all-features`, so this is active there;
//! run locally with `cargo test -p kanban-service --features test-helpers`.
#![cfg(feature = "test-helpers")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use kanban_backend_memory::InMemoryStore;
use kanban_persistence_json::JsonFileStore;
use kanban_persistence_sqlite::SqliteBackend;
use kanban_service::test_helpers::BackendFactory;
use kanban_service::{json_backend::JsonDataStore, KanbanBackend};

/// JSON backend: a `JsonDataStore` over a `JsonFileStore` at the given path.
/// Reopening the same path reads the persisted file, so `factory(&path)` twice
/// returns two stores sharing on-disk state — exactly what the reload
/// assertions need.
fn json_backend_factory() -> BackendFactory {
    Box::new(|path: &Path| {
        Arc::new(JsonDataStore::new(Arc::new(JsonFileStore::new(path)))) as Arc<dyn KanbanBackend>
    })
}

/// SQLite backend: `SqliteBackend::open` is async, but the factory is sync AND
/// is called from within the test's tokio runtime. Blocking on a nested runtime
/// on the current thread panics ("cannot start a runtime from within a
/// runtime"), so open on a fresh OS thread with its own runtime and join it.
/// Reopening the same file path shares the on-disk database.
fn sqlite_backend_factory() -> BackendFactory {
    Box::new(|path: &Path| {
        let path = path.to_path_buf();
        std::thread::spawn(move || {
            // Multi-threaded: `SqliteStore` rejects a current_thread runtime, and
            // opening does async SQLite I/O. The pool it builds is runtime-agnostic;
            // the backend's later sync `DataStore` calls run on the test's own
            // multi-thread runtime via `Handle::current()` + `block_in_place`.
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("multi-thread runtime");
            let backend = rt
                .block_on(SqliteBackend::open(path.to_str().unwrap()))
                .expect("open sqlite backend");
            Arc::new(backend) as Arc<dyn KanbanBackend>
        })
        .join()
        .expect("sqlite open thread")
    })
}

/// In-memory backend: the contract fns reopen by calling `factory(&path)` a
/// SECOND time and expect the SAME persisted state. In-memory has no disk, so a
/// naive fresh `InMemoryStore::new()` per call would drop the data. Key a shared
/// registry on the path so the same path always returns the same store.
fn in_memory_backend_factory() -> BackendFactory {
    let registry: Arc<Mutex<HashMap<PathBuf, Arc<dyn KanbanBackend>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    Box::new(move |path: &Path| {
        let mut map = registry.lock().unwrap();
        map.entry(path.to_path_buf())
            .or_insert_with(|| Arc::new(InMemoryStore::new()) as Arc<dyn KanbanBackend>)
            .clone()
    })
}

mod in_memory {
    kanban_service::context_contract_tests!(super::in_memory_backend_factory);
}

mod json {
    kanban_service::context_contract_tests!(super::json_backend_factory);
}

mod sqlite {
    kanban_service::context_contract_tests!(super::sqlite_backend_factory);
}

/// Optimistic-concurrency conflict detection is a FILE-store feature (it versions
/// on-disk metadata), so it is not part of the shared cross-backend macro. Run it
/// against the JSON backend, which is the one that implements it.
#[tokio::test(flavor = "multi_thread")]
async fn test_json_save_with_stale_metadata_returns_conflict() {
    kanban_service::test_helpers::contract::lifecycle::test_save_with_stale_metadata_returns_conflict(
        &json_backend_factory(),
    )
    .await;
}
