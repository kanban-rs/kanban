pub mod snapshot;

use std::sync::Arc;
use tokio::sync::mpsc;

pub use snapshot::TuiSnapshot;
/// Capacity of the bounded flush-signal channel between the UI and the save worker.
///
/// A capacity of 1 would cause data loss on slow disks when flush signals arrive
/// faster than the worker drains them. 100 slots allow bursts of rapid mutations
/// (e.g. undo/redo sprees) to queue without blocking the UI thread.
const FLUSH_QUEUE_CAPACITY: usize = 100;

/// Coordinates persistence with immediate auto-saving
///
/// # Save Behavior
///
/// Changes are saved immediately after each command execution:
/// - No debounce delay - every change is persisted instantly
/// - No data loss window - state is always current on disk
/// - Simple and predictable behavior
///
/// The immediate save approach works well for kanban boards because:
/// - User actions are discrete and human-paced (not rapid-fire)
/// - Modern SSDs handle frequent writes efficiently (1-5ms per save)
/// - Conflict detection and file watching remain responsive
///
/// # Example
/// ```ignore
/// // Execute command via TuiContext, then queue snapshot
/// ctx.execute_commands_batch(commands)?;
/// ```
pub struct SaveCoordinator {
    save_tx: Option<mpsc::Sender<()>>,
    save_completion_tx: Option<mpsc::UnboundedSender<()>>,
    pending_saves: usize,
    file_watcher: Option<Arc<kanban_persistence::FileWatcher>>,
}

impl SaveCoordinator {
    /// Create a new save coordinator with optional persistence store
    ///
    /// Returns a tuple of:
    /// - SaveCoordinator instance
    /// - Optional receiver for async save processing (flush signals)
    /// - Optional receiver for save completion notifications
    ///
    #[allow(clippy::type_complexity)]
    pub fn new(
        has_persistence: bool,
    ) -> (
        Self,
        Option<mpsc::Receiver<()>>,
        Option<mpsc::UnboundedReceiver<()>>,
    ) {
        let (save_tx, save_rx) = if has_persistence {
            let (tx, rx) = mpsc::channel(FLUSH_QUEUE_CAPACITY);
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

        let (save_completion_tx, save_completion_rx) = mpsc::unbounded_channel();

        let coordinator = Self {
            save_tx,
            save_completion_tx: Some(save_completion_tx),
            pending_saves: 0,
            file_watcher: None,
        };

        (coordinator, save_rx, Some(save_completion_rx))
    }

    /// Close the save channel to signal the worker to finish processing and exit
    /// Called during graceful shutdown before waiting for the worker to finish
    pub fn close_save_channel(&mut self) {
        self.save_tx = None;
    }

    /// Check if the save channel is available for sending snapshots
    pub fn has_save_channel(&self) -> bool {
        self.save_tx.is_some()
    }

    /// Queue a flush signal for async saving.
    /// Increments pending_saves to track unsaved changes.
    ///
    /// The suppression window on the file watcher is opened by the save worker
    /// immediately before it calls `backend.flush()`, not here. Opening it at
    /// queue-time would allow the 200 ms window to expire before the actual
    /// atomic rename occurs when the worker is delayed.
    ///
    /// Uses try_send to handle bounded channel capacity (100 slots).
    /// If channel is full, logs warning and skips to prevent blocking UI.
    pub fn queue_flush(&mut self) {
        if let Some(ref tx) = self.save_tx {
            tracing::debug!(
                "Queueing flush signal (pending: {} -> {})",
                self.pending_saves,
                self.pending_saves + 1
            );
            match tx.try_send(()) {
                Ok(_) => {
                    self.pending_saves += 1;
                    tracing::debug!("Flush signal queued successfully");
                }
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                    tracing::warn!(
                        "Save queue is full ({} pending), skipping this flush signal. \
                        This may indicate the disk is slow or the save worker is overloaded.",
                        self.pending_saves
                    );
                }
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                    tracing::error!("Failed to queue flush signal: channel closed");
                }
            }
        } else {
            tracing::debug!("No save channel available - skipping save");
        }
    }

    /// Check if there are pending saves waiting to be written
    pub fn has_pending_saves(&self) -> bool {
        self.pending_saves > 0
    }

    /// Signal that a save has been completed (called by save worker)
    pub fn save_completed(&mut self) {
        if self.pending_saves > 0 {
            self.pending_saves -= 1;
            tracing::debug!(
                "Save completed (pending: {} -> {})",
                self.pending_saves + 1,
                self.pending_saves
            );
        }
    }

    /// Get the save completion sender for the worker to use
    pub fn save_completion_tx(&self) -> Option<&mpsc::UnboundedSender<()>> {
        self.save_completion_tx.as_ref()
    }

    #[doc(hidden)]
    pub fn set_pending_for_test(&mut self, n: usize) {
        self.pending_saves = n;
    }

    /// Set the file watcher for coordinating pause/resume with saves
    /// Called after the file watcher is initialized in App::run()
    pub fn set_file_watcher(&mut self, watcher: Arc<kanban_persistence::FileWatcher>) {
        self.file_watcher = Some(watcher);
        tracing::debug!("File watcher set on SaveCoordinator");
    }

    /// Reset save channels after a backend migration.
    ///
    /// Drops the old save channel (causing the old save worker to exit),
    /// creates new channels. Returns receivers so the caller can spawn a new save worker.
    /// The store itself lives on KanbanContext.
    #[allow(clippy::type_complexity)]
    pub fn reset_save_channels(&mut self) -> (mpsc::Receiver<()>, mpsc::UnboundedReceiver<()>) {
        self.file_watcher = None;
        self.pending_saves = 0;

        let (tx, rx) = mpsc::channel(FLUSH_QUEUE_CAPACITY);
        self.save_tx = Some(tx);

        let (completion_tx, completion_rx) = mpsc::unbounded_channel();
        self.save_completion_tx = Some(completion_tx);

        (rx, completion_rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `queue_flush()` must NOT open the file-watcher suppression window.
    /// The window must be opened by the save worker immediately before
    /// `backend.flush()` to avoid expiring before the atomic rename occurs.
    #[test]
    fn test_queue_flush_does_not_open_suppress_window() {
        let (mut coordinator, _rx, _crx) = SaveCoordinator::new(true);
        let watcher = Arc::new(kanban_persistence::FileWatcher::new());
        coordinator.set_file_watcher(Arc::clone(&watcher));

        coordinator.queue_flush();

        assert!(
            !watcher.is_suppressing(),
            "queue_flush must not open the suppress window"
        );
    }

    #[test]
    fn test_save_coordinator_creation_no_persistence() {
        let (coordinator, save_rx, _completion_rx) = SaveCoordinator::new(false);
        assert!(!coordinator.has_pending_saves());
        assert!(save_rx.is_none());
    }

    #[test]
    fn test_save_coordinator_creation_with_persistence() {
        let (coordinator, save_rx, _completion_rx) = SaveCoordinator::new(true);
        assert!(!coordinator.has_pending_saves());
        assert!(save_rx.is_some());
        assert!(coordinator.has_save_channel());
    }

    #[test]
    fn test_reset_save_channels() {
        let (mut coordinator, _rx, _crx) = SaveCoordinator::new(true);
        let (_rx, _crx) = coordinator.reset_save_channels();
        assert!(!coordinator.has_pending_saves());
        assert!(coordinator.has_save_channel());
    }
}
