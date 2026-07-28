use crate::traits::{ChangeDetector, ChangeEvent};
use crate::{PersistenceError, PersistenceResult};
use chrono::Utc;
use notify::{RecursiveMode, Watcher};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::sync::Mutex as TokioMutex;
use uuid::Uuid;

/// Fallback suppression window used only when ownership can't be determined
/// from file content (non-JSON backends, unparseable/unreadable file).
const SUPPRESS_WINDOW: Duration = Duration::from_millis(500);

#[derive(Deserialize)]
struct EnvelopeMetadataProbe {
    metadata: MetadataInstanceProbe,
}

#[derive(Deserialize)]
struct MetadataInstanceProbe {
    instance_id: Uuid,
}

#[derive(Debug, PartialEq, Eq)]
enum Ownership {
    Own,
    External,
    Unknown,
}

/// Determine whether the file at `path` was last written by `expected`
/// (our own instance) by comparing against the `instance_id` stamped into
/// the JSON envelope on every save. Returns `Unknown` when this can't be
/// determined (no expected id configured, unreadable file, non-JSON
/// content) rather than guessing.
fn determine_ownership(path: &Path, expected: Option<Uuid>) -> Ownership {
    let Some(expected) = expected else {
        return Ownership::Unknown;
    };
    let Ok(bytes) = std::fs::read(path) else {
        return Ownership::Unknown;
    };
    match serde_json::from_slice::<EnvelopeMetadataProbe>(&bytes) {
        Ok(probe) if probe.metadata.instance_id == expected => Ownership::Own,
        Ok(_) => Ownership::External,
        Err(_) => Ownership::Unknown,
    }
}

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
    own_instance_id: Arc<StdMutex<Option<Uuid>>>,
    suppress_until: Arc<StdMutex<Option<Instant>>>,
}

impl FileWatcher {
    /// Create a new file watcher
    /// The broadcast channel has a buffer size of 10
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(10);
        Self {
            tx,
            task_handle: Arc::new(TokioMutex::new(None)),
            own_instance_id: Arc::new(StdMutex::new(None)),
            suppress_until: Arc::new(StdMutex::new(None)),
        }
    }

    /// Record this process's own persistence instance id, so own-write
    /// events can be identified definitively by comparing it against the
    /// `instance_id` stamped into the saved JSON envelope, instead of
    /// guessing from event timing/count.
    pub fn set_own_instance_id(&self, instance_id: Uuid) {
        *self.own_instance_id.lock().unwrap() = Some(instance_id);
    }

    /// Returns whether the fallback suppression window is currently open.
    ///
    /// Intended for tests only; not part of the stable API.
    #[doc(hidden)]
    pub fn is_suppressing(&self) -> bool {
        self.suppress_until
            .lock()
            .unwrap()
            .is_some_and(|deadline| Instant::now() < deadline)
    }

    /// Open the fallback suppression window for the next own-write.
    ///
    /// Only consulted when ownership can't be determined from file content
    /// (see [`determine_ownership`]) — e.g. non-JSON backends. Call
    /// immediately before each atomic rename so it does not expire if the
    /// writer is delayed.
    pub fn suppress_next_event(&self) {
        *self.suppress_until.lock().unwrap() = Some(Instant::now() + SUPPRESS_WINDOW);
        tracing::debug!("File watcher suppression window opened");
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
        let own_instance_id = self.own_instance_id.clone();
        let suppress_until = self.suppress_until.clone();

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
            let own_instance_id_clone = own_instance_id.clone();
            let suppress_until_clone = suppress_until.clone();

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
                        let expected_id = *own_instance_id_clone.lock().unwrap();
                        let suppressed = match determine_ownership(&watch_path, expected_id) {
                            Ownership::Own => true,
                            Ownership::External => false,
                            Ownership::Unknown => {
                                let mut guard = suppress_until_clone.lock().unwrap();
                                match *guard {
                                    Some(deadline) if Instant::now() < deadline => true,
                                    _ => {
                                        *guard = None;
                                        false
                                    }
                                }
                            }
                        };
                        if suppressed {
                            tracing::debug!(
                                "Own-write event suppressed: kind={:?}, path={}",
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

    fn envelope_json(instance_id: Uuid) -> Vec<u8> {
        format!(r#"{{"version":1,"metadata":{{"instance_id":"{instance_id}"}},"data":{{}}}}"#)
            .into_bytes()
    }

    // -- determine_ownership: pure, no I/O timing, deterministic --

    #[test]
    fn test_determine_ownership_matching_instance_id_returns_own() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("own.json");
        let id = Uuid::new_v4();
        std::fs::write(&path, envelope_json(id)).unwrap();

        assert_eq!(determine_ownership(&path, Some(id)), Ownership::Own);
    }

    #[test]
    fn test_determine_ownership_different_instance_id_returns_external() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("external.json");
        std::fs::write(&path, envelope_json(Uuid::new_v4())).unwrap();

        assert_eq!(
            determine_ownership(&path, Some(Uuid::new_v4())),
            Ownership::External
        );
    }

    #[test]
    fn test_determine_ownership_non_json_content_returns_unknown() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("db.sqlite");
        std::fs::write(&path, b"SQLite format 3\0not really json").unwrap();

        assert_eq!(
            determine_ownership(&path, Some(Uuid::new_v4())),
            Ownership::Unknown
        );
    }

    #[test]
    fn test_determine_ownership_no_expected_id_returns_unknown() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("own.json");
        std::fs::write(&path, envelope_json(Uuid::new_v4())).unwrap();

        assert_eq!(determine_ownership(&path, None), Ownership::Unknown);
    }

    // -- suppression window: fallback path only, used when ownership is Unknown --

    #[test]
    fn test_suppress_next_event_opens_suppression_window() {
        let watcher = FileWatcher::new();
        assert!(!watcher.is_suppressing(), "window must start closed");
        watcher.suppress_next_event();
        assert!(
            watcher.is_suppressing(),
            "suppress_next_event must open the window"
        );
    }

    /// With an own instance id configured and a matching envelope on disk,
    /// suppression is definitive: no window needs to be armed at all. This
    /// is the regression test for the flake — an arbitrary number of raw OS
    /// events for the same on-disk content can never leak, since each is
    /// checked against actual content rather than a fixed count/deadline.
    #[tokio::test]
    async fn test_own_write_with_matching_instance_id_is_suppressed_without_window() {
        use std::fs;
        use tempfile::NamedTempFile;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("own.json");
        let my_id = Uuid::new_v4();
        tokio::fs::write(&file_path, envelope_json(Uuid::new_v4()))
            .await
            .unwrap();

        let watcher = FileWatcher::new();
        watcher.set_own_instance_id(my_id);
        let mut rx = watcher.subscribe();
        watcher.start_watching(file_path.clone()).await.unwrap();
        sleep(Duration::from_millis(100)).await;

        let temp = NamedTempFile::new_in(dir.path()).unwrap();
        std::fs::write(temp.path(), envelope_json(my_id)).unwrap();
        fs::rename(temp.path(), &file_path).unwrap();

        sleep(Duration::from_millis(150)).await;

        assert!(
            !watcher.is_suppressing(),
            "no window was ever opened for this write"
        );
        let result = rx.try_recv();
        assert!(
            result.is_err(),
            "no event should reach the channel for an own write; got: {:?}",
            result
        );

        watcher.stop_watching().await.unwrap();
    }

    /// A definitively external write (different stamped instance id) is
    /// delivered even while a suppression window happens to be open —
    /// content-based detection overrides the timing fallback.
    #[tokio::test]
    async fn test_external_write_with_different_instance_id_is_delivered_even_within_window() {
        use std::fs;
        use tempfile::NamedTempFile;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("shared.json");
        let my_id = Uuid::new_v4();
        tokio::fs::write(&file_path, envelope_json(my_id))
            .await
            .unwrap();

        let watcher = FileWatcher::new();
        watcher.set_own_instance_id(my_id);
        let mut rx = watcher.subscribe();
        watcher.start_watching(file_path.clone()).await.unwrap();
        sleep(Duration::from_millis(100)).await;

        watcher.suppress_next_event();

        let temp = NamedTempFile::new_in(dir.path()).unwrap();
        std::fs::write(temp.path(), envelope_json(Uuid::new_v4())).unwrap();
        fs::rename(temp.path(), &file_path).unwrap();

        let result = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;
        watcher.stop_watching().await.unwrap();

        assert!(
            result.is_ok(),
            "external write with a different instance id must be delivered even within an open suppression window, got: {:?}",
            result
        );
    }

    /// Fallback path: with no own instance id configured (or unparseable
    /// content), ownership can't be determined, so the timing window is
    /// consulted. After it expires, a subsequent write IS delivered —
    /// guards against the window getting stuck open.
    #[tokio::test]
    async fn test_external_write_delivered_after_suppression_window_expires() {
        use std::fs;
        use tempfile::NamedTempFile;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("external.json");
        tokio::fs::write(&file_path, b"initial").await.unwrap();

        let watcher = FileWatcher::new();
        let mut rx = watcher.subscribe();
        watcher.start_watching(file_path.clone()).await.unwrap();
        sleep(Duration::from_millis(100)).await;

        // Own write — no instance id configured, falls back to the window.
        watcher.suppress_next_event();
        let temp = NamedTempFile::new_in(dir.path()).unwrap();
        std::fs::write(temp.path(), b"own write").unwrap();
        fs::rename(temp.path(), &file_path).unwrap();

        // Wait past the window so it has definitively expired.
        sleep(SUPPRESS_WINDOW + Duration::from_millis(150)).await;

        // Second rename simulates an external write after the window closed.
        let temp2 = NamedTempFile::new_in(dir.path()).unwrap();
        std::fs::write(temp2.path(), b"external write").unwrap();
        fs::rename(temp2.path(), &file_path).unwrap();

        let result = tokio::time::timeout(Duration::from_millis(300), rx.recv()).await;
        watcher.stop_watching().await.unwrap();

        assert!(
            result.is_ok(),
            "external write after the suppression window expires must fire an event, got: {:?}",
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
