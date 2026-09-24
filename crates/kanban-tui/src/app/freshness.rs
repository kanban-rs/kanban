use super::App;
use crate::app::FreshnessSource;
use kanban_persistence::ChangeDetector;
use std::path::PathBuf;

impl App {
    #[doc(hidden)]
    pub async fn rewire_freshness(&mut self) -> Option<PathBuf> {
        self.clear_freshness().await;
        match self
            .persistence
            .save_file
            .clone()
            .as_deref()
            .and_then(super::types::watcher_target)
        {
            Some(local) => self.wire_file_watcher(local).await,
            None => {
                self.wire_remote_subscription();
                None
            }
        }
    }

    async fn clear_freshness(&mut self) {
        if let Some(watcher) = self.persistence.file_watcher.take() {
            let _ = watcher.stop_watching().await;
        }
        self.persistence.file_change_rx = None;
        self.persistence.remote_change_rx = None;
        self.persistence.freshness = FreshnessSource::Off;
    }

    async fn wire_file_watcher(&mut self, local: &str) -> Option<PathBuf> {
        tracing::info!("Initializing file watcher for: {}", local);
        let watcher = kanban_persistence::FileWatcher::new();
        watcher.set_own_instance_id(self.ctx.backend().instance_id());
        let rx = watcher.subscribe();
        self.persistence.file_change_rx = Some(rx);
        tracing::debug!("File change broadcast receiver subscribed");

        let path = PathBuf::from(local);
        let deferred_watch_path = if path.exists() {
            if let Err(e) = watcher.start_watching(path.clone()).await {
                tracing::warn!(
                    "Failed to start file watching for {}: {}",
                    path.display(),
                    e
                );
            } else {
                tracing::info!("File watcher started for: {}", path.display());
            }
            None
        } else {
            tracing::debug!(
                "File does not exist yet, deferring file watching until first save: {}",
                path.display()
            );
            Some(path.clone())
        };

        self.persistence.file_watcher = Some(watcher);
        self.persistence.freshness = FreshnessSource::File(path);

        deferred_watch_path
    }

    fn wire_remote_subscription(&mut self) {
        if let Some(ref f) = self.persistence.save_file {
            tracing::info!("Remote storage location; skipping file watcher: {f}");
        }
        let backend = self.ctx.backend();
        if let Some(http) = backend
            .as_any()
            .and_then(|a| a.downcast_ref::<kanban_backend_http::HttpBackend>())
        {
            tracing::info!("Remote storage location; subscribing to SSE change events");
            self.persistence.remote_change_rx = Some(http.subscribe());
            self.persistence.freshness = FreshnessSource::Remote;
        }
    }
}
