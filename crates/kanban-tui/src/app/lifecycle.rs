use super::types::default_store_manager;
use super::{
    AnimationState, App, AppMode, DialogInputState, FilterState, FocusState, MigrationState,
    MultiSelectState, PersistenceState, RelationshipState, SelectionHub, SprintViewState,
    StorageBackendChoice, UiState, ViewState,
};
use crate::tui_context::TuiContext;
use kanban_core::InputState;
use kanban_service::StoreManager;
use kanban_view::board_list::BoardList;
use kanban_view::model::Model;
use std::sync::{Arc, Mutex};

impl App {
    /// Convenience constructor using the default built-in backends (SQLite +
    /// JSON). Prefer [`App::new_with_store`] when embedding the TUI in a
    /// third-party binary that registers its own [`StoreFactory`].
    pub async fn new(
        save_file: Option<String>,
    ) -> kanban_domain::KanbanResult<(Self, Option<tokio::sync::mpsc::Receiver<()>>)> {
        Self::new_with_store(default_store_manager(), save_file).await
    }

    pub async fn new_with_store(
        store_manager: StoreManager,
        save_file: Option<String>,
    ) -> kanban_domain::KanbanResult<(Self, Option<tokio::sync::mpsc::Receiver<()>>)> {
        Self::new_with_store_and_config(store_manager, save_file, kanban_service::config::load())
            .await
    }

    /// Same as [`App::new_with_store`], but takes the [`AppConfig`] explicitly
    /// instead of reading it from disk — lets tests exercise the "no config
    /// anywhere" startup path without touching the real
    /// `$HOME/.config/kanban/config.toml` or the `KANBAN_CONFIG` override.
    pub async fn new_with_store_and_config(
        store_manager: StoreManager,
        save_file: Option<String>,
        mut app_config: kanban_core::AppConfig,
    ) -> kanban_domain::KanbanResult<(Self, Option<tokio::sync::mpsc::Receiver<()>>)> {
        let config_resolved = kanban_service::config::resolve_storage_location(&app_config);
        let config_storage_backend = app_config.effective_storage_backend().to_string();
        let config_storage_location = config_resolved.clone();
        let original_storage_backend = app_config.storage_backend.clone();
        let original_storage_location = app_config.storage_location.clone();
        // True when the caller or config provides an explicit file path.
        // When false the TUI runs with an in-memory backend and nothing is
        // written to disk.
        let has_explicit_file = save_file.is_some() || original_storage_location.is_some();
        if let Some(ref file) = save_file {
            let path = std::path::Path::new(file);
            let resolved = if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(path))
                    .unwrap_or_else(|_| path.to_path_buf())
            };
            let canonical = dunce::canonicalize(&resolved).unwrap_or(resolved);
            app_config.storage_location = Some(canonical.display().to_string());
            // File arg is the source of truth — ignore config's storage_backend
            app_config.storage_backend = None;
        }
        if has_explicit_file
            && store_manager
                .sync_backend_with_file(&app_config.effective_storage_location(), &mut app_config)
        {
            tracing::warn!(
                "Storage backend auto-corrected to '{}' based on file content",
                app_config.effective_storage_backend()
            );
        }
        let (kanban_backend, persistence_file, cli_file_override): (
            std::sync::Arc<dyn kanban_service::KanbanBackend>,
            Option<String>,
            bool,
        ) = if has_explicit_file {
            let effective_file = kanban_service::config::resolve_storage_location(&app_config);
            let cli_file_override = save_file.is_some() && effective_file != config_resolved;
            // If the CLI-supplied file resolves to the same path as the configured
            // default, this is not a real override. Don't write the canonical
            // absolute path into app_config.storage_location — it is
            // indistinguishable from a user-set value and would be written to the
            // config file whenever any other setting is changed.
            if save_file.is_some() && !cli_file_override && original_storage_location.is_none() {
                app_config.storage_location = None;
            }
            let backend = store_manager
                .make_backend(&effective_file, &app_config)
                .await?;
            (backend, Some(effective_file), cli_file_override)
        } else {
            (
                std::sync::Arc::new(kanban_backend_memory::InMemoryStore::new()),
                None,
                false,
            )
        };
        let inner_ctx = kanban_service::KanbanContext::open(kanban_backend, app_config.clone())
            .await?
            .with_app_type(kanban_service::AppType::Tui);
        let (ctx, save_rx, save_completion_rx) = TuiContext::new(inner_ctx)?;
        let store_manager = Arc::new(store_manager);
        // Seed the projects-panel sort from the persisted AppConfig default so
        // the choice survives a restart (KAN-948). Done before the first
        // `prepare_frame`/`load_from_snapshot`, which re-sorts using this state.
        let mut model = Model::default();
        model.set_board_sort_from_config(&app_config);
        let app = Self {
            store_manager,
            should_quit: false,
            quit_with_pending: false,
            quit_with_migration: false,
            mode: AppMode::Normal,
            mode_stack: Vec::new(),
            input: InputState::new(),
            ctx,
            app_config,
            selection: SelectionHub::default(),
            board_list: BoardList::new(),
            animation: AnimationState::default(),
            filter: FilterState::default(),
            dialog_input: DialogInputState::default(),
            focus: FocusState::default(),
            persistence: PersistenceState::new(persistence_file, save_completion_rx),
            multi_select: MultiSelectState::default(),
            ui_state: UiState::default(),
            sprint_view: SprintViewState::default(),
            view: ViewState::default(),
            model,
            relationship: RelationshipState::default(),
            save_error: None,
            pending_key: None,
            has_data_file: has_explicit_file,
            cli_file_provided: save_file.is_some(),
            cli_file_override,
            config_storage_backend,
            config_storage_location,
            original_storage_backend,
            original_storage_location,
            export_dialog: None,
            migration_state: MigrationState::Idle,
            export_result_rx: None,
            needs_redraw: true,
            error_log: Arc::new(Mutex::new(crate::error_log::ErrorLogState::default())),
            auto_open_seen_count: 0,
            choose_storage_backend: StorageBackendChoice::default(),
        };

        Ok((app, save_rx))
    }

    pub fn spawn_save_worker(
        &mut self,
        mut rx: tokio::sync::mpsc::Receiver<()>,
        deferred_watch_path: Option<std::path::PathBuf>,
    ) {
        if !self.ctx.save_coordinator.has_save_channel() {
            return;
        }

        let backend = self.ctx.backend();
        let file_watcher = self.persistence.file_watcher.clone();
        let save_completion_tx = self.ctx.save_coordinator.save_completion_tx().cloned();
        let (save_error_tx, save_error_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        self.persistence.save_error_rx = Some(save_error_rx);

        tracing::info!("Spawning save worker");
        let handle = tokio::spawn(async move {
            use kanban_persistence::ChangeDetector;
            tracing::info!("Save worker task started");
            let mut watching_started = deferred_watch_path.is_none();
            while rx.recv().await.is_some() {
                tracing::debug!("Save worker received flush signal");

                // Open the suppression window immediately before the atomic
                // rename so it does not expire if the worker was delayed.
                if let Some(ref watcher) = file_watcher {
                    watcher.suppress_next_event();
                }

                let save_succeeded = match backend.flush().await {
                    Ok(()) => {
                        tracing::debug!("Save worker completed flush");
                        if !watching_started {
                            if let (Some(ref watcher), Some(ref p)) =
                                (&file_watcher, &deferred_watch_path)
                            {
                                match watcher.start_watching(p.clone()).await {
                                    Ok(()) => {
                                        tracing::info!(
                                            "Deferred file watching started for: {}",
                                            p.display()
                                        );
                                        watching_started = true;
                                    }
                                    Err(e) => tracing::warn!(
                                        "Failed to start deferred file watching: {}",
                                        e
                                    ),
                                }
                            }
                        }
                        true
                    }
                    Err(kanban_domain::KanbanError::ConflictDetected { path, .. }) => {
                        tracing::warn!(
                            "Save worker detected conflict at {}: external write wins",
                            path
                        );
                        false
                    }
                    Err(e) => {
                        tracing::error!("Save worker flush failed: {}", e);
                        let _ = save_error_tx.send(e.to_string());
                        false
                    }
                };

                // Only signal completion when the flush actually succeeded.
                // On conflict or error the save remains outstanding: pending_saves
                // stays > 0 so the Layer-2 guard keeps protecting the TUI, and
                // the file-watcher event from the external write will trigger the
                // ExternalChangeDetected dialog which queues a fresh flush.
                if save_succeeded {
                    if let Some(ref tx) = save_completion_tx {
                        if let Err(e) = tx.send(()) {
                            tracing::error!("Failed to send save completion signal: {}", e);
                        }
                    }
                }
            }
            tracing::info!("Save worker exited (channel closed)");
        });
        self.persistence.save_worker_handle = Some(handle);
    }

    pub(in crate::app) fn adopt_storage_file(&mut self, filename: String) -> bool {
        let path = if std::path::Path::new(&filename).is_absolute() {
            filename.clone()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(&filename).display().to_string())
                .unwrap_or_else(|_| filename.clone())
        };

        // The dialog creates a *new* board file. Refuse to clobber anything
        // already at the chosen path — if the user wants to open an existing
        // file they should quit and relaunch with `kanban <path>`.
        if std::path::Path::new(&path).exists() {
            self.set_error(format!(
                "\"{}\" already exists. Pick a different name, or quit and run `kanban {}` to open it.",
                filename, filename
            ));
            return false;
        }

        let handle = tokio::runtime::Handle::current();
        debug_assert!(
            handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread,
            "adopt_storage_file requires a multi-threaded Tokio runtime; \
             block_in_place is unavailable on a current_thread runtime."
        );
        // Capture in-memory state before swapping backends; replace_backend
        // discards the old backend and the new one starts empty (or loaded
        // from a non-existent file).
        let snapshot = match self.ctx.snapshot() {
            Ok(s) => s,
            Err(e) => {
                self.set_error(format!("Could not capture in-memory state: {}", e));
                return false;
            }
        };

        let store_manager = self.store_manager.clone();
        let app_config = self.app_config.clone();
        let path_for_closure = path.clone();
        // make_backend creates the backing file as a side effect for SQLite
        // (sqlx opens the DB during construction). A probe failure below
        // therefore can leave an empty SQLite file at `path`; sqlx opens an
        // existing empty DB cleanly so a retry on the same path is safe.
        let backend_result = tokio::task::block_in_place(|| {
            handle.block_on(async move {
                store_manager
                    .make_backend(&path_for_closure, &app_config)
                    .await
            })
        });

        match backend_result {
            Ok(backend) => {
                // Transfer the in-memory state to the new backend; this also
                // marks it dirty so the queued flush actually writes to disk.
                if let Err(e) = backend.as_data_store().apply_snapshot(snapshot) {
                    self.set_error(format!("Could not seed \"{}\": {}", filename, e));
                    return false;
                }
                // Probe the read paths so any backend failure surfaces
                // before we commit by swapping the backend in.
                if let Err(e) = backend.snapshot() {
                    self.set_error(format!(
                        "Could not read seeded snapshot from \"{}\": {}",
                        filename, e
                    ));
                    return false;
                }
                if let Err(e) = backend.batch_count() {
                    self.set_error(format!(
                        "Could not read batch count from \"{}\": {}",
                        filename, e
                    ));
                    return false;
                }
                self.ctx.replace_backend(backend);
                let (save_rx, completion_rx) = self.ctx.save_coordinator.reset_save_channels();
                self.persistence.save_file = Some(path.clone());
                self.persistence.save_completion_rx = Some(completion_rx);
                self.has_data_file = true;
                self.app_config.storage_location = Some(path);
                self.spawn_save_worker(save_rx, None);
                self.ctx.save_coordinator.queue_flush();
                true
            }
            Err(e) => {
                self.set_error(format!("Could not open \"{}\": {}", filename, e));
                false
            }
        }
    }
}
