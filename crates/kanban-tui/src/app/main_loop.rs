use super::{App, AppMode, DialogMode, MigrationState};
use crate::{
    events::{Event, EventHandler},
    ui,
};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use kanban_domain::KanbanResult;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

impl App {
    #[doc(hidden)]
    pub async fn load_initial_state(&mut self) {
        // Trigger the lazy data load eagerly so file errors are caught here.
        // On error, clear save_file to avoid writing back to a broken file.
        if let Err(e) = self.ctx.snapshot() {
            tracing::warn!("Failed to load initial state from file: {e}");
            self.persistence.save_file = None;
            self.set_error(format!("Failed to read data file: {e}"));
            return;
        }
        self.migrate_sprint_logs();
        // Migration is a transparent startup operation, not a user change.
        // mark_clean so the startup flush doesn't trigger the conflict popup.
        self.ctx.mark_clean();
        self.prepare_frame();
        self.check_ended_sprints();
        if self.selection.board.get().is_none() && self.model.live_boards().next().is_some() {
            self.selection.board.set(Some(0));
        }
    }

    pub async fn run(
        &mut self,
        save_rx: Option<tokio::sync::mpsc::Receiver<()>>,
    ) -> KanbanResult<()> {
        self.load_initial_state().await;

        let mut terminal = setup_terminal()?;

        // Initialize file watching if a save file is configured
        if let Some(ref save_file) = self.persistence.save_file {
            use kanban_persistence::ChangeDetector;
            tracing::info!("Initializing file watcher for: {}", save_file);
            let watcher = kanban_persistence::FileWatcher::new();
            let rx = watcher.subscribe();
            self.persistence.file_change_rx = Some(rx);
            tracing::debug!("File change broadcast receiver subscribed");

            let path = std::path::PathBuf::from(save_file);
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
                Some(path)
            };

            // Store the watcher to keep the background task alive
            self.persistence.file_watcher = Some(watcher.clone());
            let watcher_arc = std::sync::Arc::new(watcher);
            self.ctx.save_coordinator.set_file_watcher(watcher_arc);

            // Spawn async save worker if save channel is configured
            if let Some(rx) = save_rx {
                self.spawn_save_worker(rx, deferred_watch_path);
            } else {
                tracing::debug!("No save channel receiver - no saves will be processed");
            }
        } else if let Some(rx) = save_rx {
            self.spawn_save_worker(rx, None);
        } else {
            tracing::debug!("No save channel receiver - no saves will be processed");
        }

        self.maybe_push_startup_file_dialog();

        while !self.should_quit {
            let mut events = EventHandler::new();

            loop {
                if self.needs_redraw {
                    self.prepare_frame();
                    terminal.draw(|frame| ui::render(self, frame))?;
                    self.needs_redraw = false;
                }

                tokio::select! {
                    Some(event) = events.next() => {
                        match event {
                            Event::Key(key) => {
                                self.needs_redraw = true;
                                let should_restart = self.handle_key_event(key, &mut terminal, &events);
                                if should_restart {
                                    break;
                                }
                                // Drain buffered events before next draw to
                                // prevent input lag when rendering is slow.
                                let mut saw_tick = false;
                                while let Some(queued) = events.try_next() {
                                    match queued {
                                        Event::Key(k) => {
                                            let should_restart = self.handle_key_event(k, &mut terminal, &events);
                                            if should_restart {
                                                break;
                                            }
                                        }
                                        Event::Tick => {
                                            saw_tick = true;
                                        }
                                    }
                                }
                                if saw_tick {
                                    self.handle_animation_tick();
                                    if let Some(ref banner) = self.ui_state.banner {
                                        if banner.is_expired(std::time::Duration::from_secs(3)) {
                                            self.clear_banner();
                                            self.needs_redraw = true;
                                        }
                                    }
                                }
                            }
                            Event::Tick => {
                                if !self.animation.animating.is_empty() {
                                    self.needs_redraw = true;
                                }
                                self.handle_animation_tick();

                                // Auto-open error log only on new ERROR entries (not WARN)
                                let error_count =
                                    self.with_error_log(|log| log.error_count);
                                if error_count > self.auto_open_seen_count
                                    && !matches!(self.mode, AppMode::ErrorLog)
                                {
                                    self.auto_open_seen_count = error_count;
                                    self.open_error_log();
                                    self.needs_redraw = true;
                                }

                                // Auto-clear banner after 3 seconds
                                if let Some(ref banner) = self.ui_state.banner {
                                    if banner.is_expired(std::time::Duration::from_secs(3)) {
                                        self.clear_banner();
                                        self.needs_redraw = true;
                                    }
                                }

                                // Handle pending conflict resolution actions
                                // Only consume pending_key if it matches expected conflict actions
                                // to avoid breaking multi-key sequences like 'gg'
                                match self.pending_key {
                                    Some('o') => {
                                        self.pending_key = None;
                                        self.needs_redraw = true;
                                        if let Some(ref watcher) = self.persistence.file_watcher {
                                            watcher.suppress_next_event();
                                        }
                                        self.ctx.clear_conflict();
                                        if let Err(e) = self.ctx.save().await {
                                            tracing::error!("Failed to force overwrite: {}", e);
                                        }
                                    }
                                    Some('t') => {
                                        self.pending_key = None;
                                        self.needs_redraw = true;
                                        // Reload from disk via backend
                                        match self.ctx.reload().await {
                                            Ok(()) => {
                                                self.ctx.clear_conflict();
                                                self.prepare_frame();
                                                self.needs_redraw = true;
                                                tracing::info!("Reloaded state from disk");
                                            }
                                            Err(e) => {
                                                tracing::error!("Failed to reload from disk: {}", e);
                                            }
                                        }
                                    }
                                    Some('r') => {
                                        self.pending_key = None;
                                        self.needs_redraw = true;
                                        self.auto_reload_from_external_change().await;
                                    }
                                    // Don't consume pending_key for other values (e.g., 'g' for gg sequence)
                                    _ => {}
                                }

                                // Check if help menu pending action should execute
                                if let Some((start_time, action)) = &self.ui_state.help_pending_action {
                                    if start_time.elapsed().as_millis() >= 100 {
                                        self.needs_redraw = true;
                                        if let AppMode::Help(previous_mode) = &self.mode {
                                            self.mode = (**previous_mode).clone();
                                        } else {
                                            self.mode = AppMode::Normal;
                                        }
                                        self.ui_state.help_list.reset();

                                        let action = *action;
                                        self.ui_state.help_pending_action = None;
                                        let should_restart =
                                            self.dispatch_help_action(action, &mut terminal, &events);
                                        if should_restart {
                                            break;
                                        }
                                    }
                                }

                            }
                        }
                    }
                    result = async {
                        if let MigrationState::Migrating { ref mut result_rx, .. } = self.migration_state {
                            result_rx.await.ok()
                        } else {
                            std::future::pending().await
                        }
                    } => {
                        self.needs_redraw = true;
                        let old_config = match std::mem::replace(&mut self.migration_state, MigrationState::Idle) {
                            MigrationState::Migrating { old_config, .. } => old_config,
                            MigrationState::Idle => unreachable!(),
                        };
                        if let Some(result) = result {
                            self.handle_migration_complete(*old_config, result).await;
                        }
                    }
                    export_result = async {
                        if let Some(ref mut rx) = &mut self.export_result_rx {
                            rx.await.ok()
                        } else {
                            std::future::pending().await
                        }
                    } => {
                        self.needs_redraw = true;
                        self.export_result_rx = None;
                        if let Some(result) = export_result {
                            match result {
                                Ok(filename) => self.set_success(format!("Exported to {}", filename)),
                                Err(e) => self.set_error(e),
                            }
                        }
                    }
                    _ = async {
                        if let Some(ref mut rx) = &mut self.persistence.save_completion_rx {
                            rx.recv().await
                        } else {
                            std::future::pending().await
                        }
                    } => {
                        // Save operation completed - update dirty flag
                        tracing::debug!("Save completion signal received");
                        self.ctx.save_coordinator.save_completed();
                        self.clear_save_error();
                        // Reset force quit flag and dirty flag if all saves are now complete
                        if !self.ctx.save_coordinator.has_pending_saves() {
                            self.ctx.mark_clean();
                            self.quit_with_pending = false;
                        }
                    }
                    Some(error_msg) = async {
                        if let Some(ref mut rx) = &mut self.persistence.save_error_rx {
                            rx.recv().await
                        } else {
                            std::future::pending().await
                        }
                    } => {
                        tracing::warn!("Save error received from worker: {}", error_msg);
                        self.set_save_error(error_msg);
                        self.needs_redraw = true;
                    }
                    Some(_change_event) = async {
                        if let Some(ref mut rx) = &mut self.persistence.file_change_rx {
                            match rx.recv().await {
                                Ok(event) => {
                                    tracing::debug!(
                                        "File change event received at {}",
                                        event.detected_at
                                    );
                                    Some(event)
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                                    tracing::warn!(
                                        "File watcher events lagged: {} events dropped",
                                        count
                                    );
                                    None
                                }
                                Err(e) => {
                                    tracing::error!("File change receiver error: {}", e);
                                    None
                                }
                            }
                        } else {
                            std::future::pending().await
                        }
                    } => {
                        self.needs_redraw = true;
                        if self.ctx.save_coordinator.has_pending_saves() {
                            tracing::debug!("File change event ignored: own save in flight");
                        } else if !self.ctx.is_dirty() {
                            tracing::info!("External change detected, auto-reloading");
                            self.auto_reload_from_external_change().await;
                            tracing::info!("Auto-reloaded due to external file change");
                        } else if self.mode != AppMode::Dialog(DialogMode::ConflictResolution)
                            && self.mode != AppMode::Dialog(DialogMode::ExternalChangeDetected)
                        {
                            tracing::warn!("External file change detected with local changes");
                            self.open_dialog(DialogMode::ExternalChangeDetected);
                        }
                    }
                }

                if self.should_quit {
                    break;
                }
            }

            if self.should_quit {
                break;
            }
        }

        // If a migration completed at the same instant as quit, apply it before tearing down.
        self.await_migration().await;

        // Graceful shutdown: ensure all queued saves complete before exit
        self.ctx.save_coordinator.close_save_channel(); // Close save_tx channel to signal worker to finish

        // Wait for save worker to finish processing all queued saves
        if let Some(handle) = self.persistence.save_worker_handle.take() {
            handle.await.ok();
            tracing::info!("Save worker finished, all saves complete");
        }

        restore_terminal(&mut terminal)?;
        Ok(())
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<(), io::Error> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
