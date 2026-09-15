/// The source the app intended to wire for freshness, not proof a watch is
/// armed. `File` is set even when the watched path did not exist yet (the
/// save worker arms the watch after the first flush) and when
/// `start_watching` failed and was only logged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FreshnessSource {
    #[default]
    Off,
    File(std::path::PathBuf),
    Remote,
}

#[derive(Default)]
pub struct PersistenceState {
    pub save_file: Option<String>,
    pub file_change_rx: Option<tokio::sync::broadcast::Receiver<kanban_persistence::ChangeEvent>>,
    pub file_watcher: Option<kanban_persistence::FileWatcher>,
    pub save_worker_handle: Option<tokio::task::JoinHandle<()>>,
    pub save_completion_rx: Option<tokio::sync::mpsc::UnboundedReceiver<()>>,
    pub save_error_rx: Option<tokio::sync::mpsc::UnboundedReceiver<String>>,
    pub remote_change_rx:
        Option<tokio::sync::mpsc::Receiver<kanban_service::api::ChangeEventFrame>>,
    /// Written only by `App::rewire_freshness`; mirrors which of
    /// `file_change_rx`/`file_watcher`/`remote_change_rx` is live.
    pub freshness: FreshnessSource,
}

impl PersistenceState {
    pub fn new(
        save_file: Option<String>,
        save_completion_rx: Option<tokio::sync::mpsc::UnboundedReceiver<()>>,
    ) -> Self {
        Self {
            save_file,
            file_change_rx: None,
            file_watcher: None,
            save_worker_handle: None,
            save_completion_rx,
            save_error_rx: None,
            remote_change_rx: None,
            freshness: FreshnessSource::default(),
        }
    }
}
