# kanban-backend

Defines the `KanbanBackend` trait and the registry/factory machinery that lets
the service layer talk to a storage backend without knowing which one is
plugged in. This is the abstraction `kanban-service` depends on instead of
depending on `kanban-persistence-json` / `kanban-persistence-sqlite` /
`kanban-backend-memory` directly (KAN-1027) — adding a new backend means
implementing `KanbanBackend` and `KanbanBackendFactory` in its own crate and
registering it from the application, not editing `kanban-service`.

## Key public exports

From `src/lib.rs`, `src/factory.rs`, `src/remote_writes.rs`, `src/local_persistence.rs`:

```rust
pub use factory::{KanbanBackendFactory, KanbanBackendRegistry};
pub use local_persistence::LocalPersistence;
pub use remote_writes::RemoteWrites;

#[async_trait]
pub trait KanbanBackend: DataStore + CommandStore + Send + Sync {
    fn as_data_store(&self) -> &dyn DataStore;
    async fn flush(&self) -> KanbanResult<()> { Ok(()) }
    async fn reload(&self) -> KanbanResult<()> { Ok(()) }
    fn needs_flush(&self) -> bool { false }
    fn needs_save_worker(&self) -> bool { false }
    fn instance_id(&self) -> Uuid { Uuid::nil() }
    fn local_persistence(&self) -> Option<&dyn LocalPersistence> { None }
    fn health_checker(&self) -> Option<Box<dyn kanban_core::HealthChecker>> { None }
    fn remote_writes(&self) -> Option<&dyn RemoteWrites> { None }
    fn with_transaction(&self, f: TransactionFn<'_>) -> KanbanResult<()>; // required
}

pub type TransactionFn<'f> = Box<dyn FnOnce() -> KanbanResult<()> + 'f>;
```

`KanbanBackend` combines `kanban_domain::DataStore` (entity CRUD) with
`kanban_domain::CommandStore` (the audit/command log) plus the lifecycle hooks
above.

`with_transaction` is the one method with no default. A generic default could
only roll back by snapshotting the whole store and restoring it, which is cheap
in memory and ruinous on disk, so each backend supplies its own mechanism:
SQLite issues a real `BEGIN`/`COMMIT`/`ROLLBACK`, JSON and in-memory restore an
internal snapshot under a single lock, and the HTTP backend declines because the
remote server owns the state. Requiring the method also turns a new backend that
forgets it into a compile error.

The closure is `FnOnce` because the batch runs exactly once, which lets callers
move owned values in; it is boxed so the method stays callable on
`dyn KanbanBackend`.

Snapshot-restoring implementations assume a single writer: they cannot tell the
batch's writes from anyone else's, so a mutation committed by another task
mid-batch would be rolled back with it. Every consumer serialises today
(`kanban-server` and `kanban-mcp` behind `Arc<Mutex<KanbanContext>>`, the TUI on
its main loop).

```rust
#[async_trait::async_trait]
pub trait KanbanBackendFactory: Send + Sync {
    fn name(&self) -> &str;
    fn matches_locator(&self, _locator: &str, _header: &[u8]) -> bool { false }
    async fn create(&self, locator: &str, config: &AppConfig) -> KanbanResult<Arc<dyn KanbanBackend>>;
}

#[derive(Default)]
pub struct KanbanBackendRegistry { /* private */ }

impl KanbanBackendRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, factory: Box<dyn KanbanBackendFactory>);
    pub fn is_empty(&self) -> bool;
    pub fn names(&self) -> Vec<&str>;
    pub fn for_name(&self, name: &str) -> Option<&dyn KanbanBackendFactory>;
    pub fn for_locator(&self, locator: &str) -> Option<&dyn KanbanBackendFactory>;
}
```

`for_name` and `for_locator` are both first-registration-wins. `for_locator`
reads up to the first 32 bytes of the file at `locator` (empty if it doesn't
exist or isn't readable) and asks each registered factory's `matches_locator`
in registration order — the same content-sniffing pattern
`kanban_persistence::StoreRegistry` uses one layer down.

`RemoteWrites` (in `remote_writes.rs`) is the escape hatch for backends where
the remote side is authoritative for create/update/delete (i.e. `kanban-backend-http`):
`KanbanBackend::remote_writes()` returns `Some(&dyn RemoteWrites)` to bypass
local command execution entirely; every local backend (JSON, SQLite,
in-memory) returns `None`, so there's zero behavior change for them.

`LocalPersistence` (in `local_persistence.rs`) is the sibling capability for the
opposite case: backends backed by a durable local file expose format/writer
metadata through it. `KanbanBackend::local_persistence()` returns
`Some(&dyn LocalPersistence)` only for the JSON and SQLite backends;
`kanban-backend-memory` and `kanban-backend-http` inherit the `None` default,
so they are never asked for a `PersistenceMetadata` they cannot produce. This
is an Interface Segregation split (KAN-1029): the trait no longer carries a
blanket `persistence_metadata()` method every backend had to implement. It
mirrors `RemoteWrites`'s optional-capability shape exactly.

## Implementing `KanbanBackendFactory`

A minimal factory just needs a name, a content-sniffing predicate, and a
constructor. `kanban-persistence-json`'s `JsonBackendFactory` (`src/backend_factory.rs`)
is the smallest real example in the workspace:

```rust
pub struct JsonBackendFactory;

#[async_trait::async_trait]
impl KanbanBackendFactory for JsonBackendFactory {
    fn name(&self) -> &str {
        "json"
    }

    fn matches_locator(&self, _locator: &str, header: &[u8]) -> bool {
        let trimmed = header.iter().find(|b| !b.is_ascii_whitespace());
        header.is_empty() || matches!(trimmed, Some(b'{') | Some(b'['))
    }

    async fn create(&self, locator: &str, _config: &AppConfig) -> KanbanResult<Arc<dyn KanbanBackend>> {
        let store: Arc<dyn PersistenceStore + Send + Sync> = Arc::new(JsonFileStore::new(locator));
        Ok(Arc::new(JsonDataStore::new(store)))
    }
}
```

This is the layer above `StoreFactory`/`PersistenceStore` — see
[`kanban-persistence`'s "Writing a Third-Party Backend"](../kanban-persistence/README.md#writing-a-third-party-backend)
for the full walkthrough of the storage-format layer this factory wraps,
including the contract test suite and `register_backend` wiring on
`CliApp`/`McpServer`.

## Position in the workspace

```mermaid
graph TD
    CORE[kanban-core]
    DOM[kanban-domain]
    PER[kanban-persistence]
    BE[kanban-backend]
    BEMEM[kanban-backend-memory]
    BEHTTP[kanban-backend-http]
    JSON[kanban-persistence-json]
    SQL[kanban-persistence-sqlite]
    SVC[kanban-service]
    CLI[kanban-cli]
    MCP[kanban-mcp]
    TUI[kanban-tui]
    SRV[kanban-server]

    BE --> CORE
    BE --> DOM
    BE --> PER

    BEMEM --> BE
    BEHTTP --> BE
    JSON --> BE
    SQL --> BE
    SVC --> BE
    CLI --> BE
    MCP --> BE
    TUI --> BE
    SRV --> BE
```

All edges shown are normal (`[dependencies]`) edges — every crate above
depends on `kanban-backend` unconditionally, none behind a feature flag. See
the [root README](../../README.md) for the full workspace dependency graph
(including the optional/feature edges elsewhere in the graph).

## Dependencies

| Crate | Purpose |
|-------|---------|
| [`kanban-core`](../kanban-core/README.md) | `KanbanResult`, `AppConfig`, `HealthChecker` |
| [`kanban-domain`](../kanban-domain/README.md) | `DataStore`, `CommandStore` traits this crate's trait is built on |
| [`kanban-persistence`](../kanban-persistence/README.md) | `PersistenceMetadata` type surfaced by the `LocalPersistence` capability |
| `async-trait` | Async trait methods |
| `uuid` | Instance IDs |
| `chrono` | Timestamps |

## Related crates

Used by: the concrete backend crates [kanban-backend-memory](../kanban-backend-memory/README.md), [kanban-backend-http](../kanban-backend-http/README.md), [kanban-persistence-json](../kanban-persistence-json/README.md), and [kanban-persistence-sqlite](../kanban-persistence-sqlite/README.md), plus [kanban-service](../kanban-service/README.md).
