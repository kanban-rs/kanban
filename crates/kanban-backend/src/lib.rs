pub mod remote_writes;
pub use remote_writes::RemoteWrites;

use async_trait::async_trait;
use kanban_domain::command_store::CommandStore;
use kanban_domain::data_store::DataStore;
use kanban_domain::{KanbanError, KanbanResult};
use kanban_persistence::PersistenceMetadata;
use uuid::Uuid;

/// Combines the entity-level CRUD interface (`DataStore`) with the command
/// log (`CommandStore`) and lifecycle methods needed for pluggable backends.
///
/// # Lifecycle methods
///
/// - `flush()`: persist in-memory state to durable storage. For SQLite this
///   runs an explicit WAL checkpoint (TRUNCATE); for JSON this serialises the
///   cache to disk.
/// - `reload()`: discard cached state so the next read re-fetches from
///   durable storage. For SQLite this is a no-op (reads are always live).
/// - `needs_flush()`: returns `true` when there are uncommitted writes that
///   a subsequent `flush()` would persist.
/// - `needs_save_worker()`: returns `true` for backends (JSON) that require
///   a background worker to call `flush()` asynchronously after mutations.
/// - `instance_id()`: stable ID used to distinguish file writes by this
///   instance from external modifications.
#[async_trait]
pub trait KanbanBackend: DataStore + CommandStore + Send + Sync {
    /// Upcast to `&dyn DataStore`.
    fn as_data_store(&self) -> &dyn DataStore;

    /// Persist any cached writes to durable storage.
    async fn flush(&self) -> KanbanResult<()> {
        Ok(())
    }

    /// Discard cached state so the next read re-fetches from storage.
    async fn reload(&self) -> KanbanResult<()> {
        Ok(())
    }

    /// Returns `true` when there are writes that have not been flushed yet.
    fn needs_flush(&self) -> bool {
        false
    }

    /// Returns `true` for backends (JSON) that require a background flush
    /// worker. Always `false` for write-through backends (SQLite, in-memory).
    fn needs_save_worker(&self) -> bool {
        false
    }

    /// Stable instance UUID used for own-write detection in file watchers.
    fn instance_id(&self) -> Uuid {
        Uuid::nil()
    }

    /// Metadata about the underlying persistence store (file format version,
    /// writer kanban version, writer commit, last save time). Returns `None`
    /// for in-memory backends or when no metadata has been observed yet.
    /// Surfaced by the TUI's F12 diagnostics panel.
    fn persistence_metadata(&self) -> Option<PersistenceMetadata> {
        None
    }

    /// Returns a health checker for this backend, if supported.
    /// The default returns `None`; backends that can self-diagnose
    /// (file readable, connection pool alive) override this.
    fn health_checker(&self) -> Option<Box<dyn kanban_core::HealthChecker>> {
        None
    }

    /// Some(...) when this backend wants create/update/delete for board/column/
    /// card to bypass local command execution entirely (an HTTP backend, where
    /// the remote server is authoritative). `None` (the default) for every local
    /// backend — zero behavior change for JSON/SQLite/InMemory.
    fn remote_writes(&self) -> Option<&dyn crate::RemoteWrites> {
        None
    }

    /// Run `f` as an atomic batch: every mutation commits or rolls
    /// back together. The default impl snapshots state before `f`
    /// runs and restores it on failure — cheap for in-memory backends,
    /// expensive on disk. Disk-backed backends should override with a
    /// native transaction.
    fn with_transaction(&self, f: &mut dyn FnMut() -> KanbanResult<()>) -> KanbanResult<()> {
        let before = self.snapshot()?;
        match f() {
            Ok(()) => Ok(()),
            Err(e) => {
                if let Err(rollback_err) = self.apply_snapshot(before) {
                    return Err(KanbanError::Internal(format!(
                        "Batch failed ({e}) and rollback also failed ({rollback_err}). State may be inconsistent."
                    )));
                }
                Err(e)
            }
        }
    }
}
