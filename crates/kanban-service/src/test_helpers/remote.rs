use std::sync::Arc;

use crate::{KanbanBackend, RemoteBatchWrites, RemoteWrites};

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

/// Like [`remote_backend`], but for operations that route through the
/// dedicated batch-write surface (`KanbanBackend::remote_batch_writes()`, e.g.
/// `archive_cards`/`move_cards`/`update_cards`/`assign_cards_to_sprint`) rather
/// than the single-entity `RemoteWrites` surface: same in-memory-backed shape,
/// but wired to the *batch* dispatch slot `with_remote_writes` alone doesn't
/// reach.
pub fn remote_batch_backend(
    rw: Arc<dyn RemoteWrites>,
    batch: Arc<dyn RemoteBatchWrites>,
) -> Arc<dyn KanbanBackend> {
    Arc::new(crate::backend_test_support::MockBackend::with_remote_batch_writes(rw, batch))
}
