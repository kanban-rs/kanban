use std::sync::Arc;

use crate::{KanbanBackend, RemoteWrites};

/// Build an in-memory-backed [`KanbanBackend`] whose writes divert to `rw`.
///
/// Lets dependent crates' tests (e.g. `kanban-tui`) drive the service and
/// handler code paths against a *remote* backend without a real HTTP server:
/// reads and the local store still resolve against an in-memory store, while
/// every `KanbanContext` mutation diverts to `rw` — exactly the shape of a
/// production HTTP backend (where `KanbanBackend::remote_writes()` is `Some`).
///
/// Returns `Arc<dyn KanbanBackend>` so it can be passed directly to
/// `KanbanContext::open`/`open_deferred`, which both take `Arc<dyn KanbanBackend>`.
pub fn remote_backend(rw: Arc<dyn RemoteWrites>) -> Arc<dyn KanbanBackend> {
    Arc::new(crate::backend_test_support::MockBackend::with_remote_writes(rw))
}
