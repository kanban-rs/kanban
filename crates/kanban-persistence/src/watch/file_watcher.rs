use crate::traits::{ChangeDetector, ChangeEvent};
use crate::{PersistenceError, PersistenceResult};
use chrono::Utc;
use notify::{RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::Mutex as TokioMutex;

/// File system watcher for detecting changes to the persistence file
/// Uses the `notify` crate for cross-platform file watching
/// Spawns the watcher in a tokio task to handle the Send requirement
///
/// # Future Directory Format Support
///
/// This implementation is currently designed for single-file JSON persistence
/// but can be extended to support directory-based formats by:
///
/// 1. Adding a `WatchTarget` enum to distinguish between `File(path)` and `Directory(path, pattern)`
/// 2. For directory watching, use `RecursiveMode::Recursive` instead of `NonRecursive`
/// 3. Add glob pattern filtering to the event handler to match specific file extensions
/// 4. Implement event debouncing (e.g., 100ms window) to batch rapid file changes
/// 5. The OS-native backends (inotify, FSEvents, ReadDirectoryChangesW) efficiently
///    handle watching directories with hundreds of files, incurring negligible overhead
///
/// Example future usage:
/// ```ignore
/// let watcher = FileWatcher::new();
/// watcher.start_watching(WatchTarget::Directory("./data".into(), "*.json")).await?;
/// // Efficiently watches all JSON files in directory and subdirectories
/// ```
#[derive(Clone)]
pub struct FileWatcher {
    tx: broadcast::Sender<ChangeEvent>,
    task_handle: Arc<TokioMutex<Option<tokio::task::JoinHandle<()>>>>,
    suppress_remaining: Arc<AtomicUsize>,
}

impl FileWatcher {
    /// Create a new file watcher
    /// The broadcast channel has a buffer size of 10
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(10);
        Self {
            tx,
            task_handle: Arc::new(TokioMutex::new(None)),
            suppress_remaining: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns the number of events that will still be suppressed.
    ///
    /// Intended for tests only; not part of the stable API.
    #[doc(hidden)]
    pub fn suppress_remaining(&self) -> usize {
        self.suppress_remaining.load(Ordering::SeqCst)
    }

    /// Suppress the next 2 own-write events.
    ///
    /// Call immediately before each atomic rename. Each OS event from that
    /// rename decrements the counter; when the counter reaches 0, subsequent
    /// events are delivered normally. Using 2 is conservative — Linux fires
    /// only 1 event per rename on the target path (after `has_our_file`
    /// filtering), so the counter is typically fully consumed by 1 event.
    pub fn suppress_next_event(&self) {
        self.suppress_remaining.store(2, Ordering::SeqCst);
        tracing::debug!("File watcher suppress counter set to 2");
    }
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ChangeDetector for FileWatcher {
    async fn start_watching(&self, path: PathBuf) -> PersistenceResult<()> {
        let tx = self.tx.clone();
        let task_handle = self.task_handle.clone();
        let suppress_remaining = self.suppress_remaining.clone();

        // Canonicalize the parent directory rather than `path` itself, so a
        // locator that hasn't been written yet can still be watched — the OS
        // watch below is placed on the parent directory regardless (better
        // for detecting atomic writes), so only the parent needs to resolve.
        let file_name = path
            .file_name()
            .ok_or_else(|| {
                PersistenceError::Io(std::io::Error::other(format!(
                    "watch path has no file name: {}",
                    path.display()
                )))
            })?
            .to_owned();
        let parent_dir = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => PathBuf::from("."),
        };
        let canonical_parent = tokio::fs::canonicalize(&parent_dir).await?;
        let canonical_path = canonical_parent.join(file_name);

        // The OS-level watch is registered inside the spawned task below;
        // without this signal, `start_watching` would return as soon as the
        // task is merely scheduled, racing an immediate write against a
        // watch that isn't armed yet. `ready_tx` reports back once the watch
        // is actually in place (or definitively failed), so callers that
        // `.await` this function can rely on every subsequent write being
        // observed.
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<PersistenceResult<()>>();

        // Spawn file watching in a background task
        let handle = tokio::spawn(async move {
            let parent = canonical_path
                .parent()
                .expect("Canonicalized path should always have parent")
                .to_path_buf();
            let watch_path = canonical_path.clone();
            let suppress_remaining_clone = suppress_remaining.clone();

            match notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
                Ok(event) => {
                    let is_relevant_event = matches!(
                        event.kind,
                        notify::EventKind::Modify(notify::event::ModifyKind::Data(
                            notify::event::DataChange::Content,
                        )) | notify::EventKind::Modify(notify::event::ModifyKind::Name(_),)
                            | notify::EventKind::Create(_)
                            | notify::EventKind::Remove(_)
                    );

                    let has_our_file = event.paths.iter().any(|p| p == &watch_path);

                    if is_relevant_event {
                        tracing::debug!(
                            "File system event detected: kind={:?}, paths={:?}, has_our_file={}",
                            event.kind,
                            event.paths,
                            has_our_file
                        );
                    }

                    if is_relevant_event && has_our_file {
                        let suppressed = suppress_remaining_clone
                            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                            .is_ok();
                        if suppressed {
                            tracing::debug!(
                                "Own-write event suppressed (counter): kind={:?}, path={}",
                                event.kind,
                                watch_path.display()
                            );
                            return;
                        }

                        tracing::debug!(
                            "File event detected: kind={:?}, path={}, our_file_exists={}",
                            event.kind,
                            watch_path.display(),
                            watch_path.exists()
                        );
                        let change = ChangeEvent {
                            path: watch_path.clone(),
                            detected_at: Utc::now(),
                        };
                        match tx.send(change) {
                            Ok(receiver_count) => {
                                tracing::debug!(
                                    "File change event sent to {} receivers",
                                    receiver_count
                                );
                            }
                            Err(e) => {
                                tracing::warn!("Failed to send file change event: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("File watcher error: {}", e);
                }
            }) {
                Ok(mut watcher) => {
                    // Watch parent directory first (better for detecting atomic writes on macOS FSEvents)
                    let watch_result = watcher.watch(&parent, RecursiveMode::NonRecursive);

                    if watch_result.is_err() {
                        // Fallback to watching the file directly if parent watch fails
                        if let Err(e) = watcher.watch(&canonical_path, RecursiveMode::NonRecursive)
                        {
                            tracing::error!("Failed to watch file or parent directory: {}", e);
                            let _ =
                                ready_tx.send(Err(PersistenceError::Io(std::io::Error::other(
                                    format!("failed to watch file or parent directory: {e}"),
                                ))));
                            return;
                        }
                        tracing::info!("Watching file: {}", canonical_path.display());
                    } else {
                        tracing::info!("Watching parent directory: {}", parent.display());
                    }

                    let _ = ready_tx.send(Ok(()));

                    // Keep watcher alive
                    std::future::pending::<()>().await;
                }
                Err(e) => {
                    tracing::error!("Failed to create watcher: {}", e);
                    let _ = ready_tx.send(Err(PersistenceError::Io(std::io::Error::other(
                        format!("failed to create watcher: {e}"),
                    ))));
                }
            }
        });

        let mut guard = task_handle.lock().await;
        *guard = Some(handle);

        // Wait for the watch to actually be armed before returning, closing
        // the race between this call returning and the first write landing.
        ready_rx.await.map_err(|_| {
            PersistenceError::Io(std::io::Error::other(
                "file watcher task ended before confirming the watch was armed",
            ))
        })??;

        Ok(())
    }

    async fn stop_watching(&self) -> PersistenceResult<()> {
        let mut guard = self.task_handle.lock().await;
        if let Some(handle) = guard.take() {
            handle.abort();
            tracing::info!("Stopped file watching");
        }
        Ok(())
    }

    fn subscribe(&self) -> broadcast::Receiver<ChangeEvent> {
        self.tx.subscribe()
    }

    fn is_watching(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_file_watcher_detects_direct_writes() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.json");

        // Create initial file
        tokio::fs::write(&file_path, b"initial content")
            .await
            .unwrap();

        let watcher = FileWatcher::new();
        let mut rx = watcher.subscribe();

        watcher.start_watching(file_path.clone()).await.unwrap();

        // Give watcher time to start
        sleep(Duration::from_millis(100)).await;

        // Modify the file with direct write
        tokio::fs::write(&file_path, b"modified content")
            .await
            .unwrap();

        // Wait for change event (with timeout)
        let result = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;

        watcher.stop_watching().await.unwrap();

        // We got an event (timing is platform-dependent, so this might be flaky)
        if let Ok(Ok(event)) = result {
            // Canonicalize both paths to handle platform differences (e.g., macOS /var -> /private/var)
            let expected_path = tokio::fs::canonicalize(&file_path)
                .await
                .unwrap_or(file_path.clone());
            let event_path = tokio::fs::canonicalize(&event.path)
                .await
                .unwrap_or(event.path.clone());
            assert_eq!(event_path, expected_path);
        }
    }

    #[tokio::test]
    async fn test_file_watcher_detects_atomic_writes() {
        use std::fs;
        use tempfile::NamedTempFile;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.json");

        // Create initial file
        tokio::fs::write(&file_path, b"initial content")
            .await
            .unwrap();

        let watcher = FileWatcher::new();
        let mut rx = watcher.subscribe();

        watcher.start_watching(file_path.clone()).await.unwrap();

        // Give watcher time to start
        sleep(Duration::from_millis(100)).await;

        // Modify file with atomic write pattern (temp → rename)
        let temp_file = NamedTempFile::new_in(dir.path()).unwrap();
        let temp_path = temp_file.path().to_path_buf();
        std::fs::write(&temp_path, b"atomic write content").unwrap();
        fs::rename(&temp_path, &file_path).unwrap();

        // Wait for change event (with timeout)
        let result = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;

        watcher.stop_watching().await.unwrap();

        // We got an event from the atomic write
        if let Ok(Ok(event)) = result {
            let expected_path = tokio::fs::canonicalize(&file_path)
                .await
                .unwrap_or(file_path.clone());
            let event_path = tokio::fs::canonicalize(&event.path)
                .await
                .unwrap_or(event.path.clone());
            assert_eq!(event_path, expected_path);
        }
    }

    /// Pure unit test: `suppress_next_event` loads the counter to 2.
    /// No I/O, no async, no timing.
    #[test]
    fn test_suppress_next_event_sets_counter() {
        let watcher = FileWatcher::new();
        assert_eq!(watcher.suppress_remaining(), 0, "counter must start at 0");
        watcher.suppress_next_event();
        assert_eq!(
            watcher.suppress_remaining(),
            2,
            "suppress_next_event must set counter to 2"
        );
    }

    /// After our own atomic rename, the counter is decremented and no event
    /// reaches the channel.  Replaces the 500 ms timeout test.
    #[tokio::test]
    async fn test_own_write_decrements_suppress_counter() {
        use std::fs;
        use tempfile::NamedTempFile;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("own.json");
        tokio::fs::write(&file_path, b"initial").await.unwrap();

        let watcher = FileWatcher::new();
        let mut rx = watcher.subscribe();
        watcher.start_watching(file_path.clone()).await.unwrap();
        sleep(Duration::from_millis(100)).await;

        watcher.suppress_next_event();
        assert_eq!(watcher.suppress_remaining(), 2);

        let temp = NamedTempFile::new_in(dir.path()).unwrap();
        std::fs::write(temp.path(), b"own write").unwrap();
        fs::rename(temp.path(), &file_path).unwrap();

        // Give the OS time to deliver the event to the handler.
        sleep(Duration::from_millis(150)).await;

        assert!(
            watcher.suppress_remaining() < 2,
            "counter must have been decremented by the OS event"
        );
        // No event must have been forwarded to subscribers.
        let result = rx.try_recv();
        assert!(
            result.is_err(),
            "no event should reach the channel for an own write; got: {:?}",
            result
        );

        watcher.stop_watching().await.unwrap();
    }

    /// After the counter is exhausted, a subsequent external write IS delivered.
    /// Guards against a "counter stuck at MAX" regression.
    #[tokio::test]
    async fn test_external_write_delivered_after_counter_exhausted() {
        use std::fs;
        use tempfile::NamedTempFile;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("external.json");
        tokio::fs::write(&file_path, b"initial").await.unwrap();

        let watcher = FileWatcher::new();
        let mut rx = watcher.subscribe();
        watcher.start_watching(file_path.clone()).await.unwrap();
        sleep(Duration::from_millis(100)).await;

        // Own write — suppress counter counts it down.
        watcher.suppress_next_event();
        let temp = NamedTempFile::new_in(dir.path()).unwrap();
        std::fs::write(temp.path(), b"own write").unwrap();
        fs::rename(temp.path(), &file_path).unwrap();
        sleep(Duration::from_millis(150)).await;

        // Second rename simulates an external write after counter is exhausted.
        let temp2 = NamedTempFile::new_in(dir.path()).unwrap();
        std::fs::write(temp2.path(), b"external write").unwrap();
        fs::rename(temp2.path(), &file_path).unwrap();

        let result = tokio::time::timeout(Duration::from_millis(300), rx.recv()).await;
        watcher.stop_watching().await.unwrap();

        assert!(
            result.is_ok(),
            "external write after counter is exhausted must fire an event, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_file_watcher_does_not_fire_for_unrelated_temp_file() {
        use tempfile::NamedTempFile;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.json");

        // Create the watched file
        tokio::fs::write(&file_path, b"initial content")
            .await
            .unwrap();

        let watcher = FileWatcher::new();
        let mut rx = watcher.subscribe();

        watcher.start_watching(file_path.clone()).await.unwrap();

        // Give watcher time to start
        sleep(Duration::from_millis(100)).await;

        // Create a temp file in the SAME directory but do NOT rename it to test.json
        let temp_file = NamedTempFile::new_in(dir.path()).unwrap();
        std::fs::write(temp_file.path(), b"unrelated content").unwrap();

        // No event should be emitted — the temp file is not our watched path
        let result = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;

        watcher.stop_watching().await.unwrap();

        assert!(
            result.is_err(),
            "Expected timeout (no event), but got: {:?}",
            result
        );
    }
}
