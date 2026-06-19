pub mod mode;
pub use mode::{AppMode, DialogMode};

pub mod focus;
pub use focus::{BoardFocus, CardFocus, Focus, FocusState, SettingsFocus};

pub mod sprint_view;
pub use sprint_view::{SprintTaskPanel, SprintViewState};

pub mod animation;
pub use animation::{AnimationState, CardAnimation};

pub mod selection;
pub use selection::SelectionHub;

pub mod filter;
pub use filter::FilterState;

pub mod multi_select;
pub use multi_select::MultiSelectState;

pub mod dialog_input;
pub use dialog_input::DialogInputState;

pub mod relationship;
pub use relationship::RelationshipState;

pub mod model;

pub mod view;
pub use view::ViewState;

pub mod persistence;
pub use persistence::PersistenceState;

pub mod ui_state;
pub use ui_state::UiState;

use crate::{
    clipboard,
    components::Banner,
    editor::edit_in_external_editor,
    events::{Event, EventHandler},
    tui_context::TuiContext,
    ui,
    view_strategy::{UnifiedViewStrategy, ViewRefreshContext, ViewStrategy},
};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use kanban_core::{AppConfig, Editable, InputState};
use kanban_domain::AnimationType;
use kanban_domain::KanbanResult;
use kanban_domain::{
    export::{AllBoardsExport, BoardExporter, BoardImporter},
    partition_sprint_cards, sort_card_ids, Board, Card, SortField, SortOrder, Sprint,
};
use kanban_service::StoreManager;

use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Builds a `StoreManager` that mirrors the default CLI registry: SQLite
/// first (so content-sniffing prefers it) and JSON second as a catch-all
/// fallback. Used by [`App::new`] as the default backend configuration.
fn default_store_manager() -> StoreManager {
    StoreManager::new(kanban_service::default_registry())
}

pub struct App {
    pub store_manager: Arc<StoreManager>,
    pub should_quit: bool,
    pub quit_with_pending: bool, // Force quit even if saves are pending (second 'q' press)
    pub quit_with_migration: bool, // Force quit even if migration is in progress (second 'q' press)
    pub mode: AppMode,
    pub mode_stack: Vec<AppMode>,
    pub input: InputState,
    pub ctx: TuiContext,
    pub app_config: AppConfig,
    pub selection: SelectionHub,
    pub animation: AnimationState,
    pub filter: FilterState,
    pub dialog_input: DialogInputState,
    pub focus: FocusState,
    pub persistence: PersistenceState,
    pub multi_select: MultiSelectState,
    pub ui_state: UiState,
    pub sprint_view: SprintViewState,
    pub view: ViewState,
    pub model: model::Model,
    pub relationship: RelationshipState,
    pub save_error: Option<String>,
    pub pending_key: Option<char>,
    pub has_data_file: bool,
    pub cli_file_provided: bool,
    pub cli_file_override: bool,
    pub config_storage_backend: String,
    pub config_storage_location: String,
    pub original_storage_backend: Option<String>,
    pub original_storage_location: Option<String>,
    pub export_dialog: Option<ExportDialogState>,
    pub migration_state: MigrationState,
    pub export_result_rx: Option<tokio::sync::oneshot::Receiver<Result<String, String>>>,
    pub needs_redraw: bool,
    pub error_log: Arc<Mutex<crate::error_log::ErrorLogState>>,
    pub auto_open_seen_count: usize,
    pub(crate) choose_storage_backend: StorageBackendChoice,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum StorageBackendChoice {
    #[default]
    Json,
    Sqlite,
}

impl StorageBackendChoice {
    pub(crate) fn extension(self) -> &'static str {
        match self {
            Self::Json => ".json",
            Self::Sqlite => ".sqlite",
        }
    }

    pub(crate) fn toggle(self) -> Self {
        match self {
            Self::Json => Self::Sqlite,
            Self::Sqlite => Self::Json,
        }
    }
}

/// Replaces a known data-file extension on `filename` with `new_ext`, or
/// appends `new_ext` if no known extension is present. Used by the
/// choose-storage dialog when the user toggles between JSON and SQLite.
pub(crate) fn swap_known_extension(filename: &str, new_ext: &str) -> String {
    const KNOWN: &[&str] = &[".json", ".sqlite", ".sqlite3", ".db"];
    for ext in KNOWN {
        if let Some(stem) = filename.strip_suffix(ext) {
            return format!("{}{}", stem, new_ext);
        }
    }
    format!("{}{}", filename, new_ext)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportStep {
    SelectBoards,
    ExportOptions,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExportFormat {
    #[default]
    Json,
    Sqlite,
}

#[derive(Debug, Clone)]
pub struct ExportDialogState {
    pub board_selections: Vec<bool>,
    pub cursor: usize,
    pub step: ExportStep,
    pub format: ExportFormat,
    pub filename: String,
}

impl ExportDialogState {
    pub fn new(board_count: usize) -> Self {
        Self {
            board_selections: vec![false; board_count],
            cursor: 0,
            step: ExportStep::SelectBoards,
            format: ExportFormat::default(),
            filename: "export.json".to_string(),
        }
    }

    pub fn toggle(&mut self, index: usize) {
        if let Some(selected) = self.board_selections.get_mut(index) {
            *selected = !*selected;
        }
    }

    pub fn select_all(&mut self) {
        let all_selected = self.board_selections.iter().all(|&s| s);
        for s in &mut self.board_selections {
            *s = !all_selected;
        }
    }

    pub fn any_selected(&self) -> bool {
        self.board_selections.iter().any(|&s| s)
    }
}

pub enum MigrationState {
    Idle,
    Migrating {
        old_config: AppConfig,
        old_storage_location: String,
        result_rx: tokio::sync::oneshot::Receiver<Result<(kanban_domain::Snapshot, bool), String>>,
    },
}

pub enum CardField {
    Title,
    Description,
}

pub enum BoardField {
    Name,
    Description,
}

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
        let mut app_config = kanban_service::config::load();
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
                std::sync::Arc::new(kanban_domain::InMemoryStore::new()),
                None,
                false,
            )
        };
        let inner_ctx = kanban_service::KanbanContext::open(kanban_backend, app_config.clone())
            .await?
            .with_app_type(kanban_service::AppType::Tui);
        let (ctx, save_rx, save_completion_rx) = TuiContext::new(inner_ctx)?;
        let store_manager = Arc::new(store_manager);
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
            animation: AnimationState::default(),
            filter: FilterState::default(),
            dialog_input: DialogInputState::default(),
            focus: FocusState::default(),
            persistence: PersistenceState::new(persistence_file, save_completion_rx),
            multi_select: MultiSelectState::default(),
            ui_state: UiState::default(),
            sprint_view: SprintViewState::default(),
            view: ViewState::default(),
            model: model::Model::default(),
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

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn handle_quit_key(&mut self) {
        let needs_pending_confirm =
            self.ctx.save_coordinator.has_pending_saves() && !self.quit_with_pending;
        let needs_migration_confirm =
            matches!(self.migration_state, MigrationState::Migrating { .. })
                && !self.quit_with_migration;

        if needs_pending_confirm || needs_migration_confirm {
            if needs_pending_confirm && needs_migration_confirm {
                self.set_error(
                    "⏳ Saves pending and migration in progress... press 'q' again to force quit"
                        .to_string(),
                );
            } else if needs_pending_confirm {
                self.set_error(
                    "⏳ Saves pending... press 'q' again to force quit, or wait for completion"
                        .to_string(),
                );
                tracing::warn!("Quit attempted with pending saves, requiring confirmation");
            } else {
                self.set_error(
                    "Migration in progress... press 'q' again to abort and quit".to_string(),
                );
            }
            self.quit_with_pending = true;
            self.quit_with_migration = true;
            return;
        }

        self.quit();
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

    pub fn push_mode(&mut self, new_mode: AppMode) {
        self.mode_stack.push(self.mode.clone());
        self.mode = new_mode;
    }

    pub fn pop_mode(&mut self) {
        self.mode = self.mode_stack.pop().unwrap_or(AppMode::Normal);
    }

    pub fn is_dialog_mode(&self) -> bool {
        matches!(self.mode, AppMode::Dialog(_))
    }

    pub fn get_base_mode(&self) -> &AppMode {
        if self.is_dialog_mode() {
            self.mode_stack.last().unwrap_or(&AppMode::Normal)
        } else {
            &self.mode
        }
    }

    pub fn with_error_log<R>(&self, f: impl FnOnce(&crate::error_log::ErrorLogState) -> R) -> R {
        let log = self.error_log.lock().unwrap_or_else(|e| e.into_inner());
        f(&log)
    }

    pub fn with_error_log_mut<R>(
        &mut self,
        f: impl FnOnce(&mut crate::error_log::ErrorLogState) -> R,
    ) -> R {
        let mut log = self.error_log.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut log)
    }

    pub fn open_error_log(&mut self) {
        let entry_count = self.with_error_log_mut(|log| {
            log.has_unread_errors = false;
            log.unread_count = 0;
            log.entries.len()
        });
        self.ui_state.error_log_list.update_item_count(entry_count);
        self.ui_state.error_log_list.set_scroll_offset(0);
        self.push_mode(AppMode::ErrorLog);
    }

    pub fn set_error_log(&mut self, error_log: Arc<Mutex<crate::error_log::ErrorLogState>>) {
        self.error_log = error_log;
    }

    pub fn error_log_arc(&self) -> Arc<Mutex<crate::error_log::ErrorLogState>> {
        Arc::clone(&self.error_log)
    }

    fn handle_error_log_mode(&mut self, key_code: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        match key_code {
            KeyCode::Esc | KeyCode::Char('q') => self.pop_mode(),
            KeyCode::Char('j') | KeyCode::Down => {
                self.ui_state.error_log_list.navigate_down();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.ui_state.error_log_list.navigate_up();
            }
            _ => {}
        }
    }

    pub fn open_dialog(&mut self, dialog: DialogMode) {
        self.push_mode(AppMode::Dialog(dialog));
    }

    pub fn maybe_push_startup_file_dialog(&mut self) {
        if !self.has_data_file {
            self.choose_storage_backend = StorageBackendChoice::Json;
            self.input
                .set(format!("boards{}", StorageBackendChoice::Json.extension()));
            self.open_dialog(DialogMode::ChooseStorageFile);
        }
    }

    pub fn handle_choose_storage_file_dialog(&mut self, key_code: crossterm::event::KeyCode) {
        use crate::dialog::{handle_dialog_input, DialogAction};
        if matches!(key_code, crossterm::event::KeyCode::Tab) {
            self.choose_storage_backend = self.choose_storage_backend.toggle();
            let new_filename =
                swap_known_extension(self.input.as_str(), self.choose_storage_backend.extension());
            self.input.set(new_filename);
            return;
        }
        match handle_dialog_input(&mut self.input, key_code, false) {
            DialogAction::Confirm => {
                let filename = self.input.as_str().to_string();
                if self.adopt_storage_file(filename) {
                    self.input.clear();
                    self.pop_mode();
                }
                // On failure, leave the dialog open so the user can correct
                // the path and retry; the error banner explains what went wrong.
            }
            DialogAction::Cancel => {
                self.input.clear();
                self.pop_mode();
            }
            DialogAction::None => {}
        }
    }

    fn adopt_storage_file(&mut self, filename: String) -> bool {
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

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.ui_state.banner = Some(Banner::error(message));
    }

    pub fn set_success(&mut self, message: impl Into<String>) {
        self.ui_state.banner = Some(Banner::success(message));
    }

    pub fn clear_banner(&mut self) {
        self.ui_state.banner = None;
    }

    pub fn set_save_error(&mut self, message: String) {
        self.save_error = Some(message);
    }

    pub fn clear_save_error(&mut self) {
        self.save_error = None;
    }

    fn keycode_matches_binding_key(
        key_code: &crossterm::event::KeyCode,
        binding_key: &str,
    ) -> bool {
        use crossterm::event::KeyCode;

        match key_code {
            KeyCode::Char(c) => {
                // Check if the entire binding_key is a single char match (handles "/" correctly)
                if binding_key.len() == 1 && binding_key.starts_with(*c) {
                    return true;
                }
                // Check if any part after splitting on '/' matches
                binding_key
                    .split('/')
                    .any(|k| k.trim().len() == 1 && k.trim().starts_with(*c))
            }
            KeyCode::Enter => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "Enter" || trimmed == "ENTER"
            }),
            KeyCode::Esc => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "Esc" || trimmed == "ESC"
            }),
            KeyCode::Backspace => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "Backspace" || trimmed == "BACKSPACE"
            }),
            KeyCode::Home => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "Home" || trimmed == "HOME"
            }),
            KeyCode::End => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "End" || trimmed == "END"
            }),
            KeyCode::Down => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "↓" || trimmed == "Down" || trimmed == "DOWN"
            }),
            KeyCode::Up => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "↑" || trimmed == "Up" || trimmed == "UP"
            }),
            KeyCode::Left => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "←" || trimmed == "Left" || trimmed == "LEFT"
            }),
            KeyCode::Right => binding_key.split('/').any(|k| {
                let trimmed = k.trim();
                trimmed == "→" || trimmed == "Right" || trimmed == "RIGHT"
            }),
            _ => false,
        }
    }

    fn execute_action(&mut self, action: &crate::keybindings::KeybindingAction) {
        use crate::keybindings::KeybindingAction;

        match action {
            KeybindingAction::NavigateDown => self.handle_navigation_down(),
            KeybindingAction::NavigateUp => self.handle_navigation_up(),
            KeybindingAction::NavigateLeft => self.handle_kanban_column_left(),
            KeybindingAction::NavigateRight => self.handle_kanban_column_right(),
            KeybindingAction::SelectItem => self.handle_selection_activate(),
            KeybindingAction::CreateCard => self.handle_create_card_key(),
            KeybindingAction::CreateBoard => self.handle_create_board_key(),
            KeybindingAction::CreateSprint => self.handle_create_sprint_key(),
            KeybindingAction::CreateColumn => self.handle_create_column_key(),
            KeybindingAction::RenameBoard => self.handle_rename_board_key(),
            KeybindingAction::RenameColumn => self.handle_rename_column_key(),
            KeybindingAction::EditCard => {}
            KeybindingAction::EditBoard => self.handle_edit_board_key(),
            KeybindingAction::ToggleCompletion => self.handle_toggle_card_completion(),
            KeybindingAction::AssignToSprint => self.handle_assign_to_sprint_key(),
            KeybindingAction::ArchiveCard => self.handle_archive_card(),
            KeybindingAction::RestoreCard => self.handle_restore_card(),
            KeybindingAction::DeleteCard => self.handle_delete_card_permanent(),
            KeybindingAction::MoveCardLeft => self.handle_move_card_left(),
            KeybindingAction::MoveCardRight => self.handle_move_card_right(),
            KeybindingAction::MoveColumnUp => self.handle_move_column_up(),
            KeybindingAction::MoveColumnDown => self.handle_move_column_down(),
            KeybindingAction::DeleteColumn => self.handle_delete_column_key(),
            KeybindingAction::ExportBoard => self.handle_export_board_key(),
            KeybindingAction::ExportAll => self.handle_export_all_key(),
            KeybindingAction::ImportBoard => self.handle_import_board_key(),
            KeybindingAction::OrderCards => self.handle_order_cards_key(),
            KeybindingAction::ToggleSortOrder => self.handle_toggle_sort_order_key(),
            KeybindingAction::ToggleFilter => self.handle_toggle_sprint_filter(),
            KeybindingAction::ToggleHideAssigned => self.handle_open_filter_dialog(),
            KeybindingAction::ToggleArchivedView => self.handle_toggle_archived_cards_view(),
            KeybindingAction::ToggleTaskListView => self.handle_toggle_task_list_view(),
            KeybindingAction::ToggleCardSelection => self.handle_card_selection_toggle(),
            KeybindingAction::ClearCardSelection => self.handle_clear_card_selection(),
            KeybindingAction::SelectAllCards => self.handle_select_all_cards_in_view(),
            KeybindingAction::SetSelectedCardsPriority => self.handle_set_selected_cards_priority(),
            KeybindingAction::Search => {
                if self.focus.active == Focus::Cards {
                    self.filter.search.activate();
                    self.mode = AppMode::Search;
                }
            }
            KeybindingAction::ShowHelp => {}
            KeybindingAction::Escape => self.handle_escape_key(),
            KeybindingAction::FocusPanel(panel) => self.handle_column_or_focus_switch(*panel),
            KeybindingAction::JumpToTop => self.handle_jump_to_top(),
            KeybindingAction::JumpToBottom => self.handle_jump_to_bottom(),
            KeybindingAction::JumpHalfViewportUp => self.handle_jump_half_viewport_up(),
            KeybindingAction::JumpHalfViewportDown => self.handle_jump_half_viewport_down(),
            KeybindingAction::ManageParents => self.handle_manage_parents(),
            KeybindingAction::ManageChildren => self.handle_manage_children(),
            KeybindingAction::CarryOver => {}
            KeybindingAction::Undo => {
                if let Err(e) = self.undo() {
                    self.set_error(format!("Undo failed: {}", e));
                }
            }
            KeybindingAction::Redo => {
                if let Err(e) = self.redo() {
                    self.set_error(format!("Redo failed: {}", e));
                }
            }
            KeybindingAction::OpenSettings => self.handle_open_settings(),
            KeybindingAction::ExportBoards => {}
        }
    }

    fn handle_key_event(
        &mut self,
        key: crossterm::event::KeyEvent,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
    ) -> bool {
        use crossterm::event::KeyCode;
        let mut should_restart_events = false;

        // Clear banner on any key press
        if self.ui_state.banner.is_some() {
            self.clear_banner();
            return false;
        }

        let is_input_mode = matches!(
            self.mode,
            AppMode::Search
                | AppMode::Dialog(DialogMode::CreateBoard)
                | AppMode::Dialog(DialogMode::CreateCard)
                | AppMode::Dialog(DialogMode::CreateSprint)
                | AppMode::Dialog(DialogMode::RenameBoard)
                | AppMode::Dialog(DialogMode::ExportBoard)
                | AppMode::Dialog(DialogMode::ExportAll)
                | AppMode::Dialog(DialogMode::SetCardPoints)
                | AppMode::Dialog(DialogMode::SetBranchPrefix)
                | AppMode::Dialog(DialogMode::CreateColumn)
                | AppMode::Dialog(DialogMode::RenameColumn)
                | AppMode::Dialog(DialogMode::SetSprintPrefix)
                | AppMode::Dialog(DialogMode::SetSprintCardPrefix)
                | AppMode::Dialog(DialogMode::ChooseStorageFile)
        );

        if matches!(key.code, KeyCode::Char('q') | KeyCode::Char('Q'))
            && !is_input_mode
            && !matches!(self.mode, AppMode::ArchivedCardsView)
        {
            self.handle_quit_key();
            return false;
        }

        if matches!(key.code, KeyCode::F(12)) && !matches!(self.mode, AppMode::ErrorLog) {
            self.open_error_log();
            return false;
        }

        if matches!(key.code, KeyCode::Char('?'))
            && !is_input_mode
            && !matches!(self.mode, AppMode::Help(_))
        {
            let previous_mode = self.mode.clone();
            let provider = crate::keybindings::KeybindingRegistry::get_provider(self);
            let context = provider.get_context();
            self.ui_state
                .help_list
                .update_item_count(context.bindings.len());
            self.ui_state.help_list.set_scroll_offset(0);
            self.mode = AppMode::Help(Box::new(previous_mode));
            return false;
        }

        // Handle Ctrl+a for select all cards
        if matches!(self.mode, AppMode::Normal)
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('a'))
        {
            self.pending_key = None;
            self.handle_select_all_cards_in_view();
            return false;
        }

        match self.mode {
            AppMode::Normal => match key.code {
                KeyCode::Char('/') => {
                    self.pending_key = None;
                    if self.focus.active == Focus::Cards {
                        self.filter.search.activate();
                        self.mode = AppMode::Search;
                    }
                }
                KeyCode::Char('g') => {
                    if self.pending_key == Some('g') {
                        self.pending_key = None;
                        self.handle_jump_to_top();
                    } else {
                        self.pending_key = Some('g');
                    }
                }
                KeyCode::Char('G') => {
                    self.pending_key = None;
                    self.handle_jump_to_bottom();
                }
                KeyCode::Char('{') => {
                    self.pending_key = None;
                    self.handle_jump_half_viewport_up();
                }
                KeyCode::Char('}') => {
                    self.pending_key = None;
                    self.handle_jump_half_viewport_down();
                }
                KeyCode::Char('n') => {
                    self.pending_key = None;
                    match self.focus.active {
                        Focus::Boards => self.handle_create_board_key(),
                        Focus::Cards => self.handle_create_card_key(),
                    }
                }
                KeyCode::Char('r') => {
                    self.pending_key = None;
                    self.handle_rename_board_key();
                }
                KeyCode::Char('e') => {
                    self.pending_key = None;
                    match self.focus.active {
                        Focus::Boards => self.handle_edit_board_key(),
                        Focus::Cards => {
                            should_restart_events =
                                self.handle_edit_card_key(terminal, event_handler);
                        }
                    }
                }
                KeyCode::Char('x') => {
                    self.pending_key = None;
                    self.handle_export_board_key();
                }
                KeyCode::Char('X') => {
                    self.pending_key = None;
                    self.handle_export_all_key();
                }
                KeyCode::Char('d') => {
                    self.pending_key = None;
                    self.handle_archive_card();
                }
                KeyCode::Char('D') => {
                    self.pending_key = None;
                    self.handle_toggle_archived_cards_view();
                }
                KeyCode::Char('i') => {
                    self.pending_key = None;
                    self.handle_import_board_key();
                }
                KeyCode::Char('a') => {
                    self.pending_key = None;
                    self.handle_assign_to_sprint_key();
                }
                KeyCode::Char('c') => {
                    self.pending_key = None;
                    self.handle_toggle_card_completion();
                }
                KeyCode::Char('s') => {
                    self.pending_key = None;
                    if self.focus.active == Focus::Cards {
                        self.handle_manage_children_from_list();
                    }
                }
                KeyCode::Char('o') => {
                    self.pending_key = None;
                    self.handle_order_cards_key();
                }
                KeyCode::Char('O') => {
                    self.pending_key = None;
                    self.handle_toggle_sort_order_key();
                }
                KeyCode::Char('T') => {
                    self.pending_key = None;
                    self.handle_open_filter_dialog();
                }
                KeyCode::Char('t') => {
                    self.pending_key = None;
                    self.handle_toggle_sprint_filter();
                }
                KeyCode::Char('v') => {
                    self.pending_key = None;
                    self.handle_card_selection_toggle();
                }
                KeyCode::Char('V') => {
                    self.pending_key = None;
                    self.handle_toggle_task_list_view();
                }
                KeyCode::Char('P') => {
                    self.pending_key = None;
                    self.handle_set_selected_cards_priority();
                }
                KeyCode::Char('H') => {
                    self.pending_key = None;
                    self.handle_move_card_left();
                }
                KeyCode::Char('L') => {
                    self.pending_key = None;
                    self.handle_move_card_right();
                }
                KeyCode::Char('h') => {
                    self.pending_key = None;
                    self.handle_kanban_column_left();
                }
                KeyCode::Char('l') => {
                    self.pending_key = None;
                    self.handle_kanban_column_right();
                }
                KeyCode::Char('1') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(0);
                }
                KeyCode::Char('2') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(1);
                }
                KeyCode::Char('3') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(2);
                }
                KeyCode::Char('4') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(3);
                }
                KeyCode::Char('5') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(4);
                }
                KeyCode::Char('6') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(5);
                }
                KeyCode::Char('7') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(6);
                }
                KeyCode::Char('8') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(7);
                }
                KeyCode::Char('9') => {
                    self.pending_key = None;
                    self.handle_column_or_focus_switch(8);
                }
                KeyCode::Esc => {
                    self.pending_key = None;
                    self.handle_escape_key();
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.pending_key = None;
                    self.handle_navigation_down();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.pending_key = None;
                    self.handle_navigation_up();
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.pending_key = None;
                    self.handle_selection_activate();
                }
                KeyCode::Char('u') => {
                    self.pending_key = None;
                    if let Err(e) = self.undo() {
                        self.set_error(format!("Undo failed: {}", e));
                    }
                }
                KeyCode::Char('U') => {
                    self.pending_key = None;
                    if let Err(e) = self.redo() {
                        self.set_error(format!("Redo failed: {}", e));
                    }
                }
                KeyCode::Char('S') => {
                    self.pending_key = None;
                    self.handle_open_settings();
                }
                _ => {
                    self.pending_key = None;
                }
            },
            AppMode::CardDetail => {
                should_restart_events =
                    self.handle_card_detail_key(key.code, terminal, event_handler);
            }
            AppMode::BoardDetail => {
                should_restart_events =
                    self.handle_board_detail_key(key.code, terminal, event_handler);
            }
            AppMode::SprintDetail => self.handle_sprint_detail_key(key.code),
            AppMode::Search => self.handle_search_mode(key.code),
            AppMode::ArchivedCardsView => self.handle_archived_cards_view_mode(key.code),
            AppMode::Settings => {
                should_restart_events = self.handle_settings_key(key.code, terminal, event_handler);
            }
            AppMode::Help(_) => self.handle_help_mode(key.code),
            AppMode::ErrorLog => self.handle_error_log_mode(key.code),
            AppMode::Dialog(ref dialog) => match dialog {
                DialogMode::CreateBoard => self.handle_create_board_dialog(key.code),
                DialogMode::CreateCard => self.handle_create_card_dialog(key.code),
                DialogMode::CreateSprint => self.handle_create_sprint_dialog(key.code),
                DialogMode::RenameBoard => self.handle_rename_board_dialog(key.code),
                DialogMode::ExportBoard => self.handle_export_board_dialog(key.code),
                DialogMode::ExportAll => self.handle_export_all_dialog(key.code),
                DialogMode::ImportBoard => self.handle_import_board_popup(key.code),
                DialogMode::SetCardPoints => {
                    should_restart_events = self.handle_set_card_points_dialog(key.code);
                }
                DialogMode::SetCardPriority => self.handle_set_card_priority_popup(key.code),
                DialogMode::SetMultipleCardsPriority => {
                    self.handle_set_multiple_cards_priority_popup(key.code)
                }
                DialogMode::SetBranchPrefix => self.handle_set_branch_prefix_dialog(key.code),
                DialogMode::SetSprintPrefix => self.handle_set_sprint_prefix_dialog(key.code),
                DialogMode::SetSprintCardPrefix => {
                    self.handle_set_sprint_card_prefix_dialog(key.code)
                }
                DialogMode::OrderCards => {
                    should_restart_events = self.handle_order_cards_popup(key.code);
                }
                DialogMode::AssignCardToSprint => self.handle_assign_card_to_sprint_popup(key.code),
                DialogMode::AssignMultipleCardsToSprint => {
                    self.handle_assign_multiple_cards_to_sprint_popup(key.code)
                }
                DialogMode::CreateColumn => self.handle_create_column_dialog(key.code),
                DialogMode::RenameColumn => self.handle_rename_column_dialog(key.code),
                DialogMode::DeleteColumnConfirm => {
                    self.handle_delete_column_confirm_popup(key.code)
                }
                DialogMode::SelectTaskListView => self.handle_select_task_list_view_popup(key.code),
                DialogMode::ConfirmSprintPrefixCollision => {
                    self.handle_confirm_sprint_prefix_collision_popup(key.code)
                }
                DialogMode::FilterOptions => self.handle_filter_options_popup(key.code),
                DialogMode::ConflictResolution => self.handle_conflict_resolution_popup(key.code),
                DialogMode::ExternalChangeDetected => {
                    self.handle_external_change_detected_popup(key.code)
                }
                DialogMode::ManageParents => self.handle_manage_parents_popup(key.code),
                DialogMode::ManageChildren => self.handle_manage_children_popup(key.code),
                DialogMode::CarryOverSprint => self.handle_carry_over_sprint_popup(key.code),
                DialogMode::ExportBoards => self.handle_export_boards_dialog(key.code),
                DialogMode::ChooseStorageFile => self.handle_choose_storage_file_dialog(key.code),
            },
        }
        should_restart_events
    }

    fn handle_search_mode(&mut self, key_code: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        match key_code {
            KeyCode::Char(c) => {
                self.filter.search.input.insert_char(c);
            }
            KeyCode::Backspace => {
                self.filter.search.input.backspace();
            }
            KeyCode::Enter => {
                self.mode = AppMode::Normal;
            }
            KeyCode::Esc => {
                self.filter.search.deactivate();
                self.mode = AppMode::Normal;
            }
            _ => {}
        }
    }

    pub fn handle_archived_cards_view_mode(&mut self, key_code: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        if self.focus.active != Focus::Cards {
            self.focus.active = Focus::Cards;
        }

        match key_code {
            KeyCode::Char('r') => self.handle_restore_card(),
            KeyCode::Char('x') => self.handle_delete_card_permanent(),
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.handle_toggle_archived_cards_view();
            }
            KeyCode::Char('v') => self.handle_card_selection_toggle(),
            KeyCode::Char('V') => self.handle_toggle_task_list_view(),
            KeyCode::Char('h') => self.handle_kanban_column_left(),
            KeyCode::Char('l') => self.handle_kanban_column_right(),
            KeyCode::Char('j') | KeyCode::Down => self.handle_navigation_down(),
            KeyCode::Char('k') | KeyCode::Up => self.handle_navigation_up(),
            _ => {}
        }
    }

    /// Scrolls the help list so the selected item is visible.
    ///
    /// Two passes are needed because `get_adjusted_viewport_height` reserves rows
    /// for scroll indicators, and an indicator can appear or disappear after the
    /// first `ensure_selected_visible` call — changing the available height. A
    /// second pass with the updated height corrects any residual mis-alignment.
    fn scroll_help_into_view(&mut self) {
        let raw = crate::ui::help_popup_viewport_height(self.view.last_frame_area);
        if raw == 0 {
            return;
        }
        let h0 = self.ui_state.help_list.get_adjusted_viewport_height(raw);
        self.ui_state.help_list.ensure_selected_visible(h0);
        let h1 = self.ui_state.help_list.get_adjusted_viewport_height(raw);
        if h1 != h0 {
            self.ui_state.help_list.ensure_selected_visible(h1);
        }
    }

    fn handle_help_mode(&mut self, key_code: crossterm::event::KeyCode) {
        use crate::keybindings::KeybindingRegistry;
        use crossterm::event::KeyCode;

        match key_code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.ui_state.help_pending_action = None;
                self.ui_state.help_list.navigate_down();
                self.scroll_help_into_view();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.ui_state.help_pending_action = None;
                self.ui_state.help_list.navigate_up();
                self.scroll_help_into_view();
            }
            KeyCode::Char('h') | KeyCode::Char('l') => {
                self.ui_state.help_pending_action = None;
            }
            KeyCode::Enter => {
                self.ui_state.help_pending_action = None;
                if let Some(index) = self.ui_state.help_list.get_selected_index() {
                    let provider = KeybindingRegistry::get_provider(self);
                    let context = provider.get_context();

                    if let Some(binding) = context.bindings.get(index) {
                        if let AppMode::Help(previous_mode) = &self.mode {
                            self.mode = (**previous_mode).clone();
                        } else {
                            self.mode = AppMode::Normal;
                        }
                        self.ui_state.help_list.reset();

                        self.execute_action(&binding.action);
                    }
                }
            }
            KeyCode::Esc | KeyCode::Char('?') => {
                self.ui_state.help_pending_action = None;
                if let AppMode::Help(previous_mode) = &self.mode {
                    self.mode = (**previous_mode).clone();
                } else {
                    self.mode = AppMode::Normal;
                }
                self.ui_state.help_list.reset();
            }
            _ => {
                let provider = KeybindingRegistry::get_provider(self);
                let context = provider.get_context();

                if let Some((index, binding)) = context
                    .bindings
                    .iter()
                    .enumerate()
                    .find(|(_, b)| Self::keycode_matches_binding_key(&key_code, &b.key))
                {
                    self.ui_state.help_list.jump_to(index);
                    self.scroll_help_into_view();
                    self.ui_state.help_pending_action = Some((Instant::now(), binding.action));
                }
            }
        }
    }

    pub fn handle_animation_tick(&mut self) {
        let now = Instant::now();
        let mut completed_animations = Vec::new();

        for (&card_id, animation) in &self.animation.animating {
            let elapsed = now.duration_since(animation.start_time).as_millis();
            if elapsed >= animation::ANIMATION_DURATION_MS {
                completed_animations.push((card_id, animation.animation_type));
            }
        }

        // Group animations by type for batch processing
        let mut archive_cards = Vec::new();
        let mut affected_columns: Vec<uuid::Uuid> = Vec::new();
        let mut restore_cards = Vec::new();
        let mut delete_cards = Vec::new();

        for (card_id, animation_type) in completed_animations {
            self.animation.animating.remove(&card_id);
            match animation_type {
                AnimationType::Archiving => {
                    let cards = self.model.cards();
                    if let Some(card_pos) = cards.iter().position(|c| c.id == card_id) {
                        let card = &cards[card_pos];
                        if !affected_columns.contains(&card.column_id) {
                            affected_columns.push(card.column_id);
                        }
                        archive_cards.push(card_id);
                    }
                }
                AnimationType::Restoring => {
                    restore_cards.push(card_id);
                }
                AnimationType::Deleting => {
                    delete_cards.push(card_id);
                }
            }
        }

        let had_archives = !archive_cards.is_empty();
        let had_deletes = !delete_cards.is_empty();

        // Execute archive + per-column compact as a single undo batch so that
        // one user-perceived "delete" maps to one `u` press to undo.
        if had_archives {
            let mut commands = vec![kanban_domain::commands::Command::Card(
                kanban_domain::commands::CardCommand::Archive(
                    kanban_domain::commands::ArchiveCards { ids: archive_cards },
                ),
            )];
            for column_id in &affected_columns {
                commands.push(kanban_domain::commands::Command::Card(
                    kanban_domain::commands::CardCommand::CompactPositions(
                        kanban_domain::commands::CompactColumnPositions {
                            column_id: *column_id,
                        },
                    ),
                ));
            }

            if let Err(e) = self.execute_commands_batch(commands) {
                tracing::error!("Failed to archive cards: {}", e);
            } else if let Some((column_id, position)) = self.animation.archive_anchor.take() {
                self.select_card_after_deletion(column_id, position);
            }
        }

        // Execute batch delete commands
        if had_deletes {
            let mut delete_commands: Vec<kanban_domain::commands::Command> = Vec::new();
            for card_id in delete_cards {
                let cmd = kanban_domain::commands::Command::Card(
                    kanban_domain::commands::CardCommand::Delete(
                        kanban_domain::commands::DeleteCard { card_id },
                    ),
                );
                delete_commands.push(cmd);
            }
            if let Err(e) = self.execute_commands_batch(delete_commands) {
                tracing::error!("Failed to delete cards: {}", e);
            }
        }

        // Handle restore animations individually (less common)
        for card_id in restore_cards {
            self.complete_restore_animation(card_id);
        }
    }

    fn complete_restore_animation(&mut self, card_id: uuid::Uuid) {
        if let Some(archived_card) = self
            .model
            .archived_cards()
            .iter()
            .find(|dc| dc.card.id == card_id)
            .cloned()
        {
            self.restore_card(archived_card);
        }
    }

    pub fn get_board_card_count(&self, board_id: uuid::Uuid) -> usize {
        let filter = self.board_card_filter(board_id);
        let board = self.model.boards().iter().find(|b| b.id == board_id);
        kanban_domain::count_filtered_cards(
            self.model.cards(),
            self.model.columns(),
            self.model.sprints(),
            board,
            &filter,
        )
    }

    pub fn get_sorted_board_cards(&self, board_id: uuid::Uuid) -> Vec<Card> {
        let filter = self.board_card_filter(board_id);
        let board = self.model.boards().iter().find(|b| b.id == board_id);
        kanban_domain::filter_and_sort_cards(
            self.model.cards(),
            self.model.columns(),
            self.model.sprints(),
            board,
            &filter,
        )
    }

    fn board_card_filter(&self, board_id: uuid::Uuid) -> kanban_domain::CardListFilter {
        let sprint_ids: std::collections::HashSet<uuid::Uuid> =
            self.filter.active_sprint_filters.iter().copied().collect();
        kanban_domain::CardListFilter {
            board_id: Some(board_id),
            sprint_ids: (!sprint_ids.is_empty()).then_some(sprint_ids),
            hide_assigned: self.filter.hide_assigned_cards,
            ..Default::default()
        }
    }

    pub fn get_selected_card_in_context(&self) -> Option<Card> {
        if let Some(task_list) = self.view.strategy.get_active_task_list() {
            if let Some(card_id) = task_list.get_selected_card_id() {
                return self.get_card_by_id(card_id);
            }
        }
        None
    }

    pub fn get_selected_card_id(&self) -> Option<uuid::Uuid> {
        self.view
            .strategy
            .get_active_task_list()
            .and_then(|list| list.get_selected_card_id())
    }

    pub fn select_card_by_id(&mut self, card_id: uuid::Uuid) {
        // Try the active task list first (covers flat and grouped views, and
        // kanban view when the card stays in the same column).
        if let Some(task_list) = self.view.strategy.get_active_task_list_mut() {
            if task_list.select_card(card_id) {
                return;
            }
        }
        // Kanban (column) view: if the card moved to a different column the
        // active list no longer contains it.  Find the column that now holds
        // the card, switch the active column to it, then select.
        let col_index = self
            .view
            .strategy
            .get_all_task_lists()
            .iter()
            .enumerate()
            .find_map(|(i, list)| list.cards.iter().position(|&id| id == card_id).map(|_| i));
        if let Some(idx) = col_index {
            self.view.strategy.try_navigate_to_column(idx);
            if let Some(task_list) = self.view.strategy.get_active_task_list_mut() {
                task_list.select_card(card_id);
            }
        }
    }

    pub fn get_card_by_id(&self, card_id: uuid::Uuid) -> Option<Card> {
        self.model
            .card(card_id)
            .or_else(|| self.model.archived_card(card_id))
            .cloned()
    }

    pub fn get_card_for_detail_view(&self) -> Option<Card> {
        self.selection
            .active_card_id
            .and_then(|id| self.model.card(id).cloned())
    }

    /// Sets `active_card_id` to `id` if a card with that id exists in the
    /// model. Returns whether the activation took effect, so callers that
    /// gate downstream work on the card existing can chain off the boolean.
    /// On miss the previously-active card is left untouched; sites that
    /// require clear-on-miss semantics must use [`Self::set_active_card_or_clear`].
    pub(crate) fn activate_card(&mut self, id: uuid::Uuid) -> bool {
        if self.model.card(id).is_some() {
            self.selection.active_card_id = Some(id);
            true
        } else {
            false
        }
    }

    /// Sets `active_card_id` to `id` if the card resolves in the model,
    /// otherwise clears it. Use at sites where `id` was obtained from a
    /// surface that may still reference an archived card (the file-watcher
    /// reload race), so downstream code that gates on
    /// `active_card_id.is_some()` does not act on a stale previous card.
    pub(crate) fn set_active_card_or_clear(&mut self, id: uuid::Uuid) {
        self.selection.active_card_id = self.model.card(id).map(|c| c.id);
    }

    pub fn populate_sprint_task_lists(&mut self, sprint_id: uuid::Uuid) {
        let cards = self.model.cards();
        let board_opt = self
            .selection
            .active_board_index
            .and_then(|i| self.model.boards().get(i));

        let (uncompleted_ids, completed_ids) = if let Some(board) = board_opt {
            let columns = self.model.columns();
            let sprints = self.model.sprints();
            let sorted_sprint_ids =
                kanban_domain::CardQueryBuilder::new(cards, columns, sprints, board)
                    .in_sprints(std::iter::once(sprint_id))
                    .execute();
            let mut unc = Vec::new();
            let mut comp = Vec::new();
            for id in sorted_sprint_ids {
                if let Some(card) = cards.iter().find(|c| c.id == id) {
                    if card.is_completed() {
                        comp.push(id);
                    } else {
                        unc.push(id);
                    }
                }
            }
            (unc, comp)
        } else {
            partition_sprint_cards(sprint_id, cards)
        };

        self.sprint_view
            .uncompleted_cards
            .update_cards(uncompleted_ids.clone());
        self.sprint_view
            .completed_cards
            .update_cards(completed_ids.clone());

        self.sprint_view
            .uncompleted_component
            .update_cards(uncompleted_ids);
        self.sprint_view
            .completed_component
            .update_cards(completed_ids);

        // Default to uncompleted panel
        self.sprint_view.panel = SprintTaskPanel::Uncompleted;
    }

    pub fn apply_sort_to_sprint_lists(&mut self, sort_field: SortField, sort_order: SortOrder) {
        let cards = self.model.cards();
        let sorted_uncompleted_ids = sort_card_ids(
            &self.sprint_view.uncompleted_cards.cards,
            cards,
            sort_field,
            sort_order,
        );
        let sorted_completed_ids = sort_card_ids(
            &self.sprint_view.completed_cards.cards,
            cards,
            sort_field,
            sort_order,
        );

        self.sprint_view
            .uncompleted_cards
            .update_cards(sorted_uncompleted_ids);
        self.sprint_view
            .completed_cards
            .update_cards(sorted_completed_ids);

        self.sprint_view
            .uncompleted_component
            .update_cards(self.sprint_view.uncompleted_cards.cards.clone());
        self.sprint_view
            .completed_component
            .update_cards(self.sprint_view.completed_cards.cards.clone());
    }

    /// Execute a single command and queue a flush.
    /// For multiple commands, prefer `execute_commands_batch` to produce only one flush signal.
    pub fn execute_command(
        &mut self,
        command: kanban_domain::commands::Command,
    ) -> KanbanResult<()> {
        self.execute_commands_batch(vec![command])
    }

    /// Execute multiple commands as a batch, producing a single flush signal.
    pub fn execute_commands_batch(
        &mut self,
        commands: Vec<kanban_domain::commands::Command>,
    ) -> KanbanResult<()> {
        self.ctx.execute_commands_batch(commands)?;
        Ok(())
    }

    pub fn prepare_frame(&mut self) {
        match self.ctx.snapshot() {
            Ok(snapshot) => self.model.load_from_snapshot(snapshot),
            Err(e) => tracing::warn!("Failed to load snapshot for frame: {e}"),
        }

        let cards_for_display: &[Card] = if self.mode == AppMode::ArchivedCardsView {
            self.model.archived_cards_flat()
        } else {
            self.model.cards()
        };

        let board_idx = self
            .selection
            .active_board_index
            .or(self.selection.board.get());
        if let Some(idx) = board_idx {
            if let Some(board) = self.model.boards().get(idx) {
                let search_query = if self.filter.search.is_active {
                    Some(self.filter.search.query())
                } else {
                    None
                };
                let ctx = ViewRefreshContext {
                    board,
                    all_cards: cards_for_display,
                    all_columns: self.model.columns(),
                    all_sprints: self.model.sprints(),
                    active_sprint_filters: self.filter.active_sprint_filters.clone(),
                    hide_assigned_cards: self.filter.hide_assigned_cards,
                    search_query,
                };
                self.view.strategy.refresh_task_lists(&ctx);
            }
        }
        self.sync_card_list_component();
    }

    /// Undo the last action
    pub fn undo(&mut self) -> KanbanResult<()> {
        if self.ctx.undo()? {
            self.needs_redraw = true;
        } else {
            self.set_error("Nothing to undo".to_string());
        }
        Ok(())
    }

    /// Redo the last undone action
    pub fn redo(&mut self) -> KanbanResult<()> {
        if self.ctx.redo()? {
            self.needs_redraw = true;
        } else {
            self.set_error("Nothing to redo".to_string());
        }
        Ok(())
    }

    pub fn sync_card_list_component(&mut self) {
        if let Some(active_list) = self.view.strategy.get_active_task_list() {
            self.view
                .card_list_component
                .update_cards(active_list.cards.clone());
        }
    }

    pub fn switch_view_strategy(&mut self, task_list_view: kanban_domain::TaskListView) {
        let new_strategy: Box<dyn ViewStrategy> = match task_list_view {
            kanban_domain::TaskListView::Flat => Box::new(UnifiedViewStrategy::flat()),
            kanban_domain::TaskListView::GroupedByColumn => {
                Box::new(UnifiedViewStrategy::grouped())
            }
            kanban_domain::TaskListView::ColumnView => Box::new(UnifiedViewStrategy::kanban()),
        };

        self.view.strategy = new_strategy;
    }

    pub fn export_board_with_filename(&self) -> io::Result<()> {
        if let Some(board_idx) = self.selection.board.get() {
            let boards = self.model.boards();
            if let Some(board) = boards.get(board_idx) {
                let columns = self.model.columns();
                let cards = self.model.cards();
                let archived_cards = self.model.archived_cards();
                let sprints = self.model.sprints();
                let board_export =
                    BoardExporter::export_board(board, columns, cards, archived_cards, sprints);

                let export = AllBoardsExport {
                    boards: vec![board_export],
                };

                BoardExporter::export_to_file(&export, self.input.as_str())?;
            }
        }
        Ok(())
    }

    pub fn export_all_boards_with_filename(&self) -> io::Result<()> {
        let boards = self.model.boards();
        let columns = self.model.columns();
        let cards = self.model.cards();
        let archived_cards = self.model.archived_cards();
        let sprints = self.model.sprints();
        let export =
            BoardExporter::export_all_boards(boards, columns, cards, archived_cards, sprints);
        BoardExporter::export_to_file(&export, self.input.as_str())?;
        Ok(())
    }

    pub fn auto_save(&self) -> io::Result<()> {
        if let Some(ref filename) = self.persistence.save_file {
            let boards = self.model.boards();
            let columns = self.model.columns();
            let cards = self.model.cards();
            let archived_cards = self.model.archived_cards();
            let sprints = self.model.sprints();
            let export =
                BoardExporter::export_all_boards(boards, columns, cards, archived_cards, sprints);
            BoardExporter::export_to_file(&export, filename)?;
        }
        Ok(())
    }

    fn check_ended_sprints(&self) {
        let ended_sprints: Vec<_> = self
            .model
            .sprints()
            .iter()
            .filter(|s| s.is_ended(chrono::Utc::now()))
            .collect();

        if !ended_sprints.is_empty() {
            tracing::warn!(
                "Found {} ended sprint(s) that need attention:",
                ended_sprints.len()
            );
            for sprint in &ended_sprints {
                if let Some(board) = self.model.boards().iter().find(|b| b.id == sprint.board_id) {
                    tracing::warn!(
                        "  - {} (ended: {})",
                        sprint.formatted_name(board, "sprint"),
                        sprint
                            .end_date
                            .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    );
                }
            }
        }
    }

    pub fn edit_board_field(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
        field: BoardField,
    ) -> io::Result<()> {
        if let Some(board_idx) = self.selection.board.get() {
            let boards = self.model.boards();
            if let Some(board) = boards.get(board_idx) {
                let temp_dir = std::env::temp_dir();
                let (temp_file, current_content) = match field {
                    BoardField::Name => {
                        let temp_file = temp_dir.join(format!("kanban-board-{}-name.md", board.id));
                        (temp_file, board.name.clone())
                    }
                    BoardField::Description => {
                        let temp_file =
                            temp_dir.join(format!("kanban-board-{}-description.md", board.id));
                        let content = board.description.as_deref().unwrap_or("").to_string();
                        (temp_file, content)
                    }
                };

                let board_id = board.id;
                if let Some(new_content) =
                    edit_in_external_editor(terminal, event_handler, temp_file, &current_content)?
                {
                    let updates = match field {
                        BoardField::Name => {
                            if new_content.trim().is_empty() {
                                None
                            } else {
                                Some(kanban_domain::BoardUpdate {
                                    name: Some(new_content.trim().to_string()),
                                    ..Default::default()
                                })
                            }
                        }
                        BoardField::Description => {
                            let desc = if new_content.trim().is_empty() {
                                kanban_domain::FieldUpdate::Clear
                            } else {
                                kanban_domain::FieldUpdate::Set(new_content)
                            };
                            Some(kanban_domain::BoardUpdate {
                                description: desc,
                                ..Default::default()
                            })
                        }
                    };
                    if let Some(updates) = updates {
                        let cmd = kanban_domain::commands::Command::Board(
                            kanban_domain::commands::BoardCommand::Update(
                                kanban_domain::commands::UpdateBoard { board_id, updates },
                            ),
                        );
                        if let Err(e) = self.execute_command(cmd) {
                            tracing::error!("Failed to update board: {}", e);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn edit_card_field(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
        field: CardField,
    ) -> io::Result<()> {
        if let Some(active_id) = self.selection.active_card_id {
            if let Some(card) = self.model.card(active_id) {
                let temp_dir = std::env::temp_dir();
                let (temp_file, current_content) = match field {
                    CardField::Title => {
                        let temp_file = temp_dir.join(format!("kanban-card-{}-title.md", card.id));
                        (temp_file, card.title.clone())
                    }
                    CardField::Description => {
                        let temp_file =
                            temp_dir.join(format!("kanban-card-{}-description.md", card.id));
                        let content = card.description.as_deref().unwrap_or("").to_string();
                        (temp_file, content)
                    }
                };

                let card_id = card.id;
                if let Some(new_content) =
                    edit_in_external_editor(terminal, event_handler, temp_file, &current_content)?
                {
                    let updates = match field {
                        CardField::Title => {
                            if new_content.trim().is_empty() {
                                None
                            } else {
                                Some(kanban_domain::CardUpdate {
                                    title: Some(new_content.trim().to_string()),
                                    ..Default::default()
                                })
                            }
                        }
                        CardField::Description => {
                            let desc = if new_content.trim().is_empty() {
                                kanban_domain::FieldUpdate::Clear
                            } else {
                                kanban_domain::FieldUpdate::Set(new_content)
                            };
                            Some(kanban_domain::CardUpdate {
                                description: desc,
                                ..Default::default()
                            })
                        }
                    };
                    if let Some(updates) = updates {
                        let cmd = kanban_domain::commands::Command::Card(
                            kanban_domain::commands::CardCommand::Update(
                                kanban_domain::commands::UpdateCard { card_id, updates },
                            ),
                        );
                        if let Err(e) = self.execute_command(cmd) {
                            tracing::error!("Failed to update card: {}", e);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn edit_entity_json_impl<T: Editable<E>, E>(
        entity: &mut E,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
        temp_file: std::path::PathBuf,
    ) -> io::Result<()> {
        Self::edit_entity_impl::<T, E>(
            entity,
            terminal,
            event_handler,
            temp_file,
            crate::edit_format::EditFormat::Json,
        )
    }

    pub fn edit_entity_impl<T: Editable<E>, E>(
        entity: &mut E,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
        temp_file: std::path::PathBuf,
        format: crate::edit_format::EditFormat,
    ) -> io::Result<()> {
        let dto = T::from_entity(entity);
        let current_content = format.serialize(&dto).unwrap_or_else(|_| "{}".to_string());

        if let Some(new_content) =
            edit_in_external_editor(terminal, event_handler, temp_file, &current_content)?
        {
            match format.deserialize::<T>(&new_content) {
                Ok(updated_dto) => {
                    updated_dto.apply_to(entity);
                    tracing::info!("Updated entity via {} editor", format);
                }
                Err(e) => {
                    tracing::error!("Failed to parse {}: {}", format, e);
                }
            }
        }

        Ok(())
    }

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
        if self.selection.board.get().is_none() && !self.model.boards().is_empty() {
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
                                        self.execute_action(&action);
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
                            self.handle_migration_complete(old_config, result).await;
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

    pub fn import_board_from_file(&mut self, filename: &str) -> io::Result<()> {
        let content = std::fs::read_to_string(filename)?;

        let first_new_index = self.model.boards().len();

        // Try V2 format first (preserves graph)
        if let Some(snapshot) = BoardImporter::try_load_snapshot(&content) {
            let cmd = kanban_domain::commands::Command::Board(
                kanban_domain::commands::BoardCommand::Import(
                    kanban_domain::commands::ImportEntities {
                        boards: snapshot.boards,
                        columns: snapshot.columns,
                        cards: snapshot.cards,
                        archived_cards: snapshot.archived_cards,
                        sprints: snapshot.sprints,
                        graph: Some(snapshot.graph),
                    },
                ),
            );
            if let Err(e) = self.ctx.execute_command(cmd) {
                self.set_error(e.to_string());
                tracing::error!("Failed to import V2 board: {}", e);
                return Ok(());
            }

            self.selection.board.set(Some(first_new_index));
            self.switch_view_strategy(kanban_domain::TaskListView::GroupedByColumn);
            return Ok(());
        }

        // Fall back to V1 format (no graph)
        let import = BoardImporter::import_from_json(&content)?;
        let entities = BoardImporter::extract_entities(import);

        let cmd =
            kanban_domain::commands::Command::Board(kanban_domain::commands::BoardCommand::Import(
                kanban_domain::commands::ImportEntities {
                    boards: entities.boards,
                    columns: entities.columns,
                    cards: entities.cards,
                    archived_cards: entities.archived_cards,
                    sprints: entities.sprints,
                    graph: None,
                },
            ));
        if let Err(e) = self.ctx.execute_command(cmd) {
            self.set_error(e.to_string());
            tracing::error!("Failed to import V1 board: {}", e);
            return Ok(());
        }

        self.selection.board.set(Some(first_new_index));
        self.switch_view_strategy(kanban_domain::TaskListView::GroupedByColumn);

        Ok(())
    }

    async fn auto_reload_from_external_change(&mut self) {
        match self.ctx.reload().await {
            Ok(()) => {
                self.ctx.mark_clean();
                self.ctx.clear_conflict();
                self.prepare_frame();
                self.needs_redraw = true;
                tracing::info!("Auto-reloaded state from external file change");
            }
            Err(e) => {
                tracing::error!("Failed to reload from disk: {}", e);
            }
        }
    }

    fn migrate_sprint_logs(&mut self) {
        if let Err(e) = self.ctx.migrate_sprint_logs() {
            tracing::error!("Failed to migrate sprint logs: {}", e);
        }
    }

    /// Generic handler for copying card outputs to clipboard
    fn copy_card_output<F>(&mut self, output_type: &str, get_output: F)
    where
        F: Fn(&Card, &Board, &[Sprint], &str) -> String,
    {
        if let Some(active_id) = self.selection.active_card_id {
            if let Some(board_idx) = self.selection.active_board_index {
                let boards = self.model.boards();
                if let Some(board) = boards.get(board_idx) {
                    if let Some(card) = self.model.card(active_id) {
                        let sprints = self.model.sprints();
                        let output = get_output(
                            card,
                            board,
                            sprints,
                            self.app_config.effective_default_card_prefix(),
                        );
                        if let Err(e) = clipboard::copy_to_clipboard(&output) {
                            self.set_error(format!("Failed to copy: {}", e));
                        } else {
                            self.set_success(format!("Copied {}", output_type));
                        }
                    }
                }
            }
        }
    }

    pub fn copy_branch_name(&mut self) {
        self.copy_card_output("branch name", |card, board, sprints, prefix| {
            card.branch_name(board, sprints, prefix)
        });
    }

    pub fn copy_git_checkout_command(&mut self) {
        self.copy_card_output("command", |card, board, sprints, prefix| {
            card.git_checkout_command(board, sprints, prefix)
        });
    }

    pub fn get_current_priority_selection_index(&self) -> usize {
        if let Some(active_id) = self.selection.active_card_id {
            if let Some(card) = self.model.card(active_id) {
                use kanban_domain::CardPriority;
                return match card.priority {
                    CardPriority::Low => 0,
                    CardPriority::Medium => 1,
                    CardPriority::High => 2,
                    CardPriority::Critical => 3,
                };
            }
        }
        0
    }

    pub fn get_current_sprint_selection_index(&self) -> usize {
        use crate::components::sprint_assign_list::{build_entries, sprint_id_of};

        if let Some(active_id) = self.selection.active_card_id {
            if let Some(card) = self.model.card(active_id) {
                if let Some(card_sprint_id) = card.sprint_id {
                    if let Some(board_idx) = self.selection.active_board_index {
                        let boards = self.model.boards();
                        if let Some(board) = boards.get(board_idx) {
                            let sprints = self.model.sprints();
                            let entries = build_entries(sprints, board.id, chrono::Utc::now());
                            for (idx, entry) in entries.iter().enumerate() {
                                if sprint_id_of(entry) == Some(card_sprint_id) {
                                    return idx;
                                }
                            }
                        }
                    }
                }
            }
        }
        0
    }

    pub fn get_current_sort_field_selection_index(&self) -> usize {
        self.filter
            .current_sort_field
            .map(crate::components::selection_dialog::popup_index_of_sort_field)
            .unwrap_or(0)
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

impl Default for App {
    fn default() -> Self {
        Self::test_default()
    }
}

impl App {
    #[doc(hidden)]
    pub fn test_default() -> Self {
        let backend = std::sync::Arc::new(kanban_domain::InMemoryStore::new());
        let inner = kanban_service::KanbanContext::open_deferred(
            backend,
            kanban_core::AppConfig::default(),
        );
        let (ctx, _save_rx, save_completion_rx) =
            crate::tui_context::TuiContext::new(inner).expect("TuiContext::new failed");
        Self {
            store_manager: Arc::new(default_store_manager()),
            should_quit: false,
            quit_with_pending: false,
            quit_with_migration: false,
            mode: AppMode::Normal,
            mode_stack: Vec::new(),
            input: InputState::new(),
            ctx,
            app_config: kanban_core::AppConfig::default(),
            selection: SelectionHub::default(),
            animation: AnimationState::default(),
            filter: FilterState::default(),
            dialog_input: DialogInputState::default(),
            focus: FocusState::default(),
            persistence: PersistenceState::new(None, save_completion_rx),
            multi_select: MultiSelectState::default(),
            ui_state: UiState::default(),
            sprint_view: SprintViewState::default(),
            view: ViewState::default(),
            model: model::Model::default(),
            relationship: RelationshipState::default(),
            save_error: None,
            pending_key: None,
            has_data_file: true,
            cli_file_provided: false,
            cli_file_override: false,
            config_storage_backend: "json".into(),
            config_storage_location: "kanban.json".into(),
            original_storage_backend: None,
            original_storage_location: None,
            export_dialog: None,
            migration_state: MigrationState::Idle,
            export_result_rx: None,
            needs_redraw: true,
            error_log: Arc::new(Mutex::new(crate::error_log::ErrorLogState::default())),
            auto_open_seen_count: 0,
            choose_storage_backend: StorageBackendChoice::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    /// The save worker must NOT send a completion signal when `backend.flush()`
    /// returns `ConflictDetected`. Sending it on conflict decrements
    /// `pending_saves` to 0, causing the Layer-2 TUI guard to lower its shield
    /// while data is still dirty — leaving the board in an inconsistent state.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_save_worker_does_not_send_completion_on_conflict() {
        use async_trait::async_trait;
        use kanban_domain::DataStore as _;
        use kanban_persistence::{
            PersistenceError, PersistenceMetadata, PersistenceResult, PersistenceStore,
            StoreSnapshot,
        };
        use kanban_service::json_backend::JsonDataStore;
        use std::path::Path;

        struct ConflictingStore;

        #[async_trait]
        impl PersistenceStore for ConflictingStore {
            async fn save(&self, _: StoreSnapshot) -> PersistenceResult<PersistenceMetadata> {
                Err(PersistenceError::ConflictDetected {
                    path: "conflict.json".into(),
                    source: None,
                })
            }
            async fn load(&self) -> PersistenceResult<(StoreSnapshot, PersistenceMetadata)> {
                Err(PersistenceError::Serialization("noop".into()))
            }
            async fn exists(&self) -> bool {
                false
            }
            fn path(&self) -> &Path {
                Path::new("conflict.json")
            }
            fn instance_id(&self) -> uuid::Uuid {
                uuid::Uuid::nil()
            }
            fn load_sync(&self) -> PersistenceResult<Option<(StoreSnapshot, PersistenceMetadata)>> {
                Ok(None)
            }
        }

        let backend = Arc::new(JsonDataStore::new(Arc::new(ConflictingStore)));
        backend
            .upsert_board(kanban_domain::Board::new("B", None::<String>))
            .unwrap();

        let inner = kanban_service::KanbanContext::open_deferred(
            Arc::clone(&backend) as Arc<dyn kanban_service::backend::KanbanBackend>,
            kanban_core::AppConfig::default(),
        );

        let (ctx, save_rx, save_completion_rx) =
            crate::tui_context::TuiContext::new(inner).expect("TuiContext::new failed");
        let save_rx = save_rx.expect("JsonDataStore must need a save worker");

        let mut app = App {
            store_manager: Arc::new(default_store_manager()),
            should_quit: false,
            quit_with_pending: false,
            quit_with_migration: false,
            mode: AppMode::Normal,
            mode_stack: Vec::new(),
            input: InputState::new(),
            ctx,
            app_config: kanban_core::AppConfig::default(),
            selection: SelectionHub::default(),
            animation: AnimationState::default(),
            filter: FilterState::default(),
            dialog_input: DialogInputState::default(),
            focus: FocusState::default(),
            persistence: PersistenceState::new(None, save_completion_rx),
            multi_select: MultiSelectState::default(),
            ui_state: UiState::default(),
            sprint_view: SprintViewState::default(),
            view: ViewState::default(),
            model: model::Model::default(),
            relationship: RelationshipState::default(),
            save_error: None,
            pending_key: None,
            has_data_file: true,
            cli_file_provided: false,
            cli_file_override: false,
            config_storage_backend: "json".into(),
            config_storage_location: "conflict.json".into(),
            original_storage_backend: None,
            original_storage_location: None,
            export_dialog: None,
            migration_state: MigrationState::Idle,
            export_result_rx: None,
            needs_redraw: false,
            error_log: Arc::new(Mutex::new(crate::error_log::ErrorLogState::default())),
            auto_open_seen_count: 0,
            choose_storage_backend: StorageBackendChoice::default(),
        };

        app.spawn_save_worker(save_rx, None);

        // Queue a flush signal (simulate a mutation that needs saving).
        app.ctx.save_coordinator.queue_flush();

        // Allow the save worker time to process the flush signal.
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        // Completion must NOT have been sent — flush returned ConflictDetected.
        let result = app
            .persistence
            .save_completion_rx
            .as_mut()
            .unwrap()
            .try_recv();
        assert!(
            result.is_err(),
            "save worker must not send completion signal when flush returns ConflictDetected"
        );
    }

    #[test]
    fn test_scroll_help_into_view_scrolls_deep_item() {
        let mut app = App::test_default();
        app.view.last_frame_area = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 50,
        };
        app.ui_state.help_list.update_item_count(50);
        app.ui_state.help_list.jump_to(49);
        app.scroll_help_into_view();
        assert!(
            app.ui_state.help_list.get_scroll_offset() > 0,
            "help list should have scrolled to bring item 49 into view"
        );
    }

    #[tokio::test]
    async fn test_no_file_tui_startup_pushes_choose_storage_dialog_prefilled_with_boards_json() {
        let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
        let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();

        app.maybe_push_startup_file_dialog();

        assert_eq!(
            app.mode,
            AppMode::Dialog(DialogMode::ChooseStorageFile),
            "no-file startup must open the ChooseStorageFile dialog"
        );
        assert_eq!(
            app.input.as_str(),
            "boards.json",
            "dialog must be pre-filled with boards.json"
        );
    }

    #[tokio::test]
    async fn test_no_file_tui_startup_dialog_cancel_stays_in_memory() {
        use crossterm::event::KeyCode;

        let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
        let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
        app.maybe_push_startup_file_dialog();

        app.handle_choose_storage_file_dialog(KeyCode::Esc);

        assert_eq!(
            app.mode,
            AppMode::Normal,
            "cancelling the dialog must return to Normal mode"
        );
        assert!(
            !app.has_data_file,
            "cancelling must leave the app in in-memory mode"
        );
        assert!(
            app.persistence.save_file.is_none(),
            "cancelling must not set a save file"
        );
    }

    // multi_thread required: adopt_storage_file uses block_in_place, which
    // panics on the current_thread runtime.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_no_file_tui_startup_dialog_confirm_creates_file_and_adopts_backend() {
        use crossterm::event::KeyCode;

        let dir = tempfile::TempDir::new().unwrap();
        let target = dir.path().join("myboard.json");
        let target_str = target.to_str().unwrap().to_string();

        let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
        let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
        app.maybe_push_startup_file_dialog();

        app.input.clear();
        app.input.set(target_str.clone());

        app.handle_choose_storage_file_dialog(KeyCode::Enter);

        assert!(
            app.has_data_file,
            "confirming must mark the app as having a data file"
        );
        assert_eq!(
            app.persistence.save_file.as_deref(),
            Some(target_str.as_str()),
            "persistence.save_file must point to the chosen path"
        );
        assert_eq!(
            app.mode,
            AppMode::Normal,
            "confirming must dismiss the dialog"
        );
    }

    // multi_thread required: adopt_storage_file uses block_in_place + the save
    // worker is a spawned task that writes to disk asynchronously.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_no_file_tui_startup_dialog_confirm_persists_in_memory_state_to_disk() {
        use crossterm::event::KeyCode;
        use kanban_domain::Board;

        let dir = tempfile::TempDir::new().unwrap();
        let target = dir.path().join("seeded.json");
        let target_str = target.to_str().unwrap().to_string();

        let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
        let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();

        // Seed in-memory state with a board so we can detect whether adopt
        // transferred it to the new on-disk backend.
        let mut snapshot = app.ctx.snapshot().unwrap();
        snapshot
            .boards
            .push(Board::new("BeforeAdopt", None::<String>));
        app.ctx.apply_snapshot(snapshot).unwrap();

        app.maybe_push_startup_file_dialog();
        app.input.clear();
        app.input.set(target_str.clone());
        app.handle_choose_storage_file_dialog(KeyCode::Enter);

        // Wait for the save worker to flush at least once.
        let completion_rx = app
            .persistence
            .save_completion_rx
            .as_mut()
            .expect("save completion channel must exist after adopt");
        tokio::time::timeout(std::time::Duration::from_secs(2), completion_rx.recv())
            .await
            .expect("save worker must signal completion within 2s")
            .expect("completion sender dropped before signal");

        assert!(
            target.exists(),
            "target file must exist on disk after adopt"
        );

        use kanban_persistence::PersistenceStore;
        let on_disk = kanban_persistence_json::JsonFileStore::new(&target_str);
        let (snap, _meta) = on_disk
            .load_sync()
            .unwrap()
            .expect("file must contain a snapshot");
        let parsed = kanban_persistence::snapshot_from_json_bytes(&snap.data).unwrap();
        assert!(
            parsed.boards.iter().any(|b| b.name == "BeforeAdopt"),
            "in-memory state must be transferred to the new on-disk backend"
        );
    }

    // multi_thread required for the same reason as the success test.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_no_file_tui_startup_dialog_confirm_refuses_existing_path() {
        use crossterm::event::KeyCode;

        let dir = tempfile::TempDir::new().unwrap();
        let target = dir.path().join("already-here.json");
        std::fs::write(&target, b"{\"boards\":[]}").unwrap();
        let target_str = target.to_str().unwrap().to_string();

        let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
        let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
        app.maybe_push_startup_file_dialog();
        app.input.clear();
        app.input.set(target_str.clone());

        app.handle_choose_storage_file_dialog(KeyCode::Enter);

        assert_eq!(
            app.mode,
            AppMode::Dialog(DialogMode::ChooseStorageFile),
            "existing-file confirm must leave the dialog open"
        );
        assert_eq!(
            app.input.as_str(),
            target_str.as_str(),
            "input must be preserved so the user can pick a different name"
        );
        assert!(
            !app.has_data_file,
            "existing-file confirm must not flip has_data_file"
        );
        let banner = app
            .ui_state
            .banner
            .as_ref()
            .expect("existing-file confirm must surface a banner");
        assert_eq!(banner.variant, crate::components::BannerVariant::Error);
        assert!(
            banner.message.contains("already exists"),
            "banner must explain that the file already exists, got: {}",
            banner.message
        );
        // The pre-existing file content must not have been touched.
        let on_disk = std::fs::read(&target).unwrap();
        assert_eq!(
            on_disk,
            b"{\"boards\":[]}".to_vec(),
            "the existing file must not have been overwritten"
        );
    }

    // multi_thread required for the same reason as the success test.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_no_file_tui_startup_dialog_confirm_failure_keeps_dialog_open() {
        use crossterm::event::KeyCode;

        // SQLite path inside a non-existent parent directory — SqliteBackend::open
        // cannot create the file because the parent doesn't exist, so make_backend
        // returns Err.
        let dir = tempfile::TempDir::new().unwrap();
        let bad_path = dir
            .path()
            .join("nonexistent_subdir")
            .join("foo.sqlite")
            .to_str()
            .unwrap()
            .to_string();

        let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
        let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
        app.maybe_push_startup_file_dialog();

        app.input.clear();
        app.input.set(bad_path.clone());

        app.handle_choose_storage_file_dialog(KeyCode::Enter);

        assert_eq!(
            app.mode,
            AppMode::Dialog(DialogMode::ChooseStorageFile),
            "confirm failure must leave the dialog open so the user can retry"
        );
        assert_eq!(
            app.input.as_str(),
            bad_path.as_str(),
            "input must be preserved on failure so the user can edit the path"
        );
        assert!(
            !app.has_data_file,
            "confirm failure must not flip has_data_file"
        );
        assert!(
            app.persistence.save_file.is_none(),
            "confirm failure must not set a save file"
        );
        let banner = app
            .ui_state
            .banner
            .as_ref()
            .expect("confirm failure must surface a banner");
        assert_eq!(
            banner.variant,
            crate::components::BannerVariant::Error,
            "banner must be an error variant"
        );
    }

    // multi_thread required: adopt_storage_file uses block_in_place.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_adopt_storage_file_leaves_context_ready_for_mutations() {
        use crossterm::event::KeyCode;
        use kanban_domain::commands::{BoardCommand, Command, CreateBoard};

        let dir = tempfile::TempDir::new().unwrap();
        let target = dir.path().join("after-adopt.json");

        let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
        let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
        app.maybe_push_startup_file_dialog();
        app.input.clear();
        app.input.set(target.to_str().unwrap().to_string());
        app.handle_choose_storage_file_dialog(KeyCode::Enter);

        // Mirrors the user-level "press n to create a board" path: the
        // context must accept a command after the backend has been swapped.
        let cmd = Command::Board(BoardCommand::Create(CreateBoard {
            id: uuid::Uuid::new_v4(),
            name: "AfterAdopt".into(),
            card_prefix: None,
            position: 0,
        }));
        app.ctx
            .execute_command(cmd)
            .expect("execute_command must succeed after adopt_storage_file");
    }

    #[tokio::test]
    async fn test_choose_storage_dialog_default_backend_is_json() {
        let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
        let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
        app.maybe_push_startup_file_dialog();

        assert_eq!(
            app.choose_storage_backend,
            StorageBackendChoice::Json,
            "default storage backend selection must be JSON"
        );
        assert_eq!(
            app.input.as_str(),
            "boards.json",
            "default filename must end in .json to match the default backend"
        );
    }

    #[tokio::test]
    async fn test_choose_storage_dialog_tab_toggles_backend_and_swaps_extension() {
        use crossterm::event::KeyCode;

        let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
        let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
        app.maybe_push_startup_file_dialog();

        app.handle_choose_storage_file_dialog(KeyCode::Tab);
        assert_eq!(
            app.choose_storage_backend,
            StorageBackendChoice::Sqlite,
            "Tab must toggle the backend selection from JSON to SQLite"
        );
        assert_eq!(
            app.input.as_str(),
            "boards.sqlite",
            "Tab must swap the filename extension to match the new backend"
        );

        app.handle_choose_storage_file_dialog(KeyCode::Tab);
        assert_eq!(
            app.choose_storage_backend,
            StorageBackendChoice::Json,
            "Tab must toggle the backend selection back to JSON"
        );
        assert_eq!(
            app.input.as_str(),
            "boards.json",
            "Tab must swap the filename extension back to .json"
        );
    }

    #[tokio::test]
    async fn test_choose_storage_dialog_tab_appends_extension_for_filename_without_one() {
        use crossterm::event::KeyCode;

        let sm = kanban_service::StoreManager::new(kanban_service::default_registry());
        let (mut app, _save_rx) = App::new_with_store(sm, None).await.unwrap();
        app.maybe_push_startup_file_dialog();
        app.input.clear();
        app.input.set("myboard".to_string());

        app.handle_choose_storage_file_dialog(KeyCode::Tab);

        assert_eq!(
            app.input.as_str(),
            "myboard.sqlite",
            "Tab must append the extension when the filename has no known extension"
        );
    }

    #[test]
    fn test_swap_known_extension_table() {
        let cases = [
            // Primary swap
            ("boards.json", ".sqlite", "boards.sqlite"),
            ("boards.sqlite", ".json", "boards.json"),
            // Alternative SQLite extensions are recognised
            ("boards.sqlite3", ".json", "boards.json"),
            ("boards.db", ".json", "boards.json"),
            // No known extension → append
            ("boards", ".sqlite", "boards.sqlite"),
            // Empty input → returns just the new extension. The dialog
            // pre-fills "boards.json" so this is unreachable in practice;
            // documented for the helper's stand-alone behaviour.
            ("", ".json", ".json"),
            // Multi-dot stems are preserved
            ("foo.tar.json", ".sqlite", "foo.tar.sqlite"),
            // Known list is lowercase-only — uppercase extensions are
            // not recognised and the new extension is appended.
            ("FOO.JSON", ".sqlite", "FOO.JSON.sqlite"),
        ];

        for (input, ext, expected) in cases {
            assert_eq!(
                swap_known_extension(input, ext),
                expected,
                "swap_known_extension({:?}, {:?})",
                input,
                ext
            );
        }
    }

    mod active_card_index_regression {
        use crate::test_helpers::{load_with_card_order, setup_reload_resort_fixture};
        use crate::App;
        use kanban_domain::{CardPriority, CardUpdate, KanbanOperations};

        #[test]
        fn test_get_current_priority_selection_index_after_reload_resort_returns_originally_selected_card_priority(
        ) {
            let mut app = App::test_default();
            let fx = setup_reload_resort_fixture(&mut app);

            app.ctx
                .update_card(
                    fx.a_id,
                    CardUpdate {
                        priority: Some(CardPriority::Critical),
                        ..Default::default()
                    },
                )
                .unwrap();
            app.ctx
                .update_card(
                    fx.p_id,
                    CardUpdate {
                        priority: Some(CardPriority::Low),
                        ..Default::default()
                    },
                )
                .unwrap();
            load_with_card_order(&mut app, &[fx.a_id, fx.p_id, fx.b_id, fx.c_id, fx.d_id]);

            let idx = app.get_current_priority_selection_index();

            assert_eq!(
                idx, 3,
                "must return Critical's index (3) — A's priority — not Low's index (0) which is P's priority at A's stale index"
            );
        }

        #[test]
        fn test_get_current_sprint_selection_index_after_reload_resort_returns_originally_selected_card_sprint(
        ) {
            use crate::components::sprint_assign_list::{build_entries, sprint_id_of};

            let mut app = App::test_default();
            let fx = setup_reload_resort_fixture(&mut app);

            let sprint_a = app.ctx.create_sprint(fx.board_id, None, None).unwrap();
            let sprint_p = app.ctx.create_sprint(fx.board_id, None, None).unwrap();
            app.ctx.assign_card_to_sprint(fx.a_id, sprint_a.id).unwrap();
            app.ctx.assign_card_to_sprint(fx.p_id, sprint_p.id).unwrap();
            load_with_card_order(&mut app, &[fx.a_id, fx.p_id, fx.b_id, fx.c_id, fx.d_id]);

            let sprints = app.model.sprints().to_vec();
            let entries = build_entries(&sprints, fx.board_id, chrono::Utc::now());
            let expected_idx = entries
                .iter()
                .position(|e| sprint_id_of(e) == Some(sprint_a.id))
                .expect("sprint_a appears in entries");

            let idx = app.get_current_sprint_selection_index();

            assert_eq!(
                idx, expected_idx,
                "must return A's sprint index, not P's sprint index at A's stale slot"
            );
        }
    }

    mod active_card_helpers {
        use crate::App;
        use kanban_domain::{CreateCardOptions, KanbanOperations, Snapshot};

        fn app_with_card() -> (App, uuid::Uuid) {
            let mut app = App::test_default();
            let board = app.ctx.create_board("B".into(), None).unwrap();
            let column = app
                .ctx
                .create_column(board.id, "Todo".into(), None)
                .unwrap();
            let card = app
                .ctx
                .create_card(
                    board.id,
                    column.id,
                    "C".into(),
                    CreateCardOptions::default(),
                )
                .unwrap();
            let snap = Snapshot {
                boards: app.ctx.data_store().list_boards().unwrap(),
                columns: app.ctx.data_store().list_all_columns().unwrap(),
                cards: app.ctx.data_store().list_all_cards().unwrap(),
                archived_cards: app.ctx.data_store().list_archived_cards().unwrap(),
                sprints: app.ctx.data_store().list_all_sprints().unwrap(),
                graph: app.ctx.data_store().get_graph().unwrap(),
            };
            app.model.load_from_snapshot(snap);
            (app, card.id)
        }

        #[test]
        fn test_activate_card_with_known_id_sets_active_card_id_and_returns_true() {
            let (mut app, card_id) = app_with_card();

            let succeeded = app.activate_card(card_id);

            assert!(succeeded, "must report success when the card exists");
            assert_eq!(app.selection.active_card_id, Some(card_id));
        }

        #[test]
        fn test_activate_card_with_unknown_id_preserves_active_card_id_and_returns_false() {
            let (mut app, card_id) = app_with_card();
            app.selection.active_card_id = Some(card_id);

            let succeeded = app.activate_card(uuid::Uuid::new_v4());

            assert!(!succeeded, "must report failure when the card is absent");
            assert_eq!(
                app.selection.active_card_id,
                Some(card_id),
                "activate_card must not touch active_card_id on miss — sites that need clear-on-miss must use set_active_card_or_clear"
            );
        }

        #[test]
        fn test_set_active_card_or_clear_with_known_id_sets_active_card_id() {
            let (mut app, card_id) = app_with_card();

            app.set_active_card_or_clear(card_id);

            assert_eq!(app.selection.active_card_id, Some(card_id));
        }

        #[test]
        fn test_set_active_card_or_clear_with_unknown_id_clears_active_card_id() {
            let (mut app, card_id) = app_with_card();
            app.selection.active_card_id = Some(card_id);

            app.set_active_card_or_clear(uuid::Uuid::new_v4());

            assert_eq!(
                app.selection.active_card_id, None,
                "set_active_card_or_clear must clear the previous active card when the new id is absent — prevents downstream handlers from acting on a stale active card"
            );
        }
    }
}
