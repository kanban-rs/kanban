use crate::app::{App, AppMode, DialogMode, ExportDialogState, Focus, SettingsFocus};
use crate::edit_format::EditFormat;
use crate::editor::edit_in_external_editor;
use crate::events::EventHandler;
use crossterm::event::KeyCode;
use kanban_domain::export::BoardExporter;
use kanban_service::AppConfigDto;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;

const EXPORT_BUTTON_STORAGE_INDEX: usize = 3;

impl App {
    pub fn settings_item_count(&self, panel: SettingsFocus) -> usize {
        match panel {
            SettingsFocus::Configuration => {
                if self.has_data_file {
                    if self.cli_file_override {
                        9
                    } else {
                        7
                    }
                } else {
                    5
                }
            }
            SettingsFocus::ConfigFile => 3,
            SettingsFocus::Storage => 4,
        }
    }

    pub fn handle_open_settings(&mut self) {
        if self.focus.active == Focus::Boards {
            self.focus.settings_focus = SettingsFocus::Configuration;
            self.selection.settings_config.set(Some(0));
            self.push_mode(AppMode::Settings);
        }
    }

    pub fn apply_config_edit(
        &mut self,
        new_content: &str,
        format: &EditFormat,
    ) -> Result<bool, String> {
        if !matches!(self.migration_state, crate::app::MigrationState::Idle) {
            return Err("Migration in progress, please wait".to_string());
        }
        let old_location =
            kanban_service::config::effective_configuration_location(&self.app_config);
        let old_storage_location =
            kanban_service::config::resolve_storage_location(&self.app_config);
        let old_config = self.app_config.clone();
        let mut config = self.app_config.clone();
        if self.cli_file_override {
            config.storage_backend = self.original_storage_backend.clone();
            config.storage_location = self.original_storage_location.clone();
        }

        let mut updated_dto: AppConfigDto = format
            .deserialize(new_content)
            .map_err(|e| format!("Failed to parse config: {}", e))?;

        // When cli_file_override is active the storage lines are commented out in
        // the editor. If the user deliberately uncomments them, the DTO will carry
        // storage fields — treat that as an explicit request to persist those
        // settings and drop the CLI override.
        let user_unlocked_storage = self.cli_file_override
            && (updated_dto.storage_backend.is_some() || updated_dto.storage_location.is_some());

        // Strip storage fields from the DTO unless the user explicitly changed the
        // storage path to a different file:
        // - CLI-supplied storage is always session-only (unless the user unlocks it).
        // - In the normal case, from_config puts the resolved absolute path into the
        //   DTO; if the user only changed non-storage fields, that same path comes
        //   back and must not be written to config (strip_defaults compares against
        //   the relative default, not the absolute, so it would survive stripping).
        let strip_storage = !user_unlocked_storage
            && (self.cli_file_override || {
                match updated_dto.storage_location.as_deref() {
                    None => false,
                    Some(loc) => {
                        let p = std::path::Path::new(loc);
                        let dto_resolved = if p.is_absolute() {
                            loc.to_string()
                        } else {
                            std::env::current_dir()
                                .map(|cwd| cwd.join(p).display().to_string())
                                .unwrap_or_else(|_| loc.to_string())
                        };
                        dto_resolved == old_storage_location
                    }
                }
            });
        if strip_storage {
            updated_dto.storage_backend = None;
            updated_dto.storage_location = None;
            // Also reset the working config to the original config-file values so
            // that any absolute path injected by App::new during CLI arg processing
            // does not survive into the saved file (strip_defaults can't recognise
            // an absolute path as the default relative "boards.json").
            config.storage_backend = self.original_storage_backend.clone();
            config.storage_location = self.original_storage_location.clone();
        }

        updated_dto
            .validate_and_apply(&mut config)
            .map_err(|e| format!("Invalid config: {}", e))?;

        if kanban_service::config::has_non_default_values(&config) {
            kanban_service::config::save(&config)
                .map_err(|e| format!("Failed to save config: {}", e))?;
            let new_location = kanban_service::config::effective_configuration_location(&config);
            if new_location != old_location {
                let old_path = std::path::Path::new(&old_location);
                if old_path.exists() {
                    if let Err(e) = std::fs::remove_file(old_path) {
                        tracing::warn!(
                            "Failed to remove old config file {}: {}",
                            old_path.display(),
                            e
                        );
                    }
                }
            }
        } else {
            let location = kanban_service::config::effective_configuration_location(&config);
            let path = std::path::Path::new(&location);
            if path.exists() {
                if let Err(e) = std::fs::remove_file(path) {
                    tracing::warn!("Failed to remove config file {}: {}", path.display(), e);
                }
            }
        }

        self.set_app_config(config);

        if self.cli_file_override {
            if user_unlocked_storage {
                // User explicitly uncommitted the storage fields → drop the override
                // so the new storage settings take effect permanently.
                self.cli_file_override = false;
                self.cli_file_provided = false;
            } else {
                // Storage lines were still commented out → keep the CLI-supplied
                // storage active for this session and skip migration.
                let mut reverted = self.app_config.clone();
                reverted.storage_backend = old_config.storage_backend.clone();
                reverted.storage_location = old_config.storage_location.clone();
                self.set_app_config(reverted);
                return Ok(true);
            }
        }

        self.apply_storage_location_change(old_config, &old_storage_location);
        Ok(true)
    }

    pub fn apply_storage_location_change(
        &mut self,
        old_config: kanban_core::AppConfig,
        old_storage_location: &str,
    ) {
        use crate::app::MigrationState;

        let new_storage_location =
            kanban_service::config::resolve_storage_location(&self.app_config);

        if self
            .store_manager
            .sync_backend_with_file(&new_storage_location, &mut self.app_config)
        {
            self.set_success(format!(
                "storage_backend changed to '{}' to match file at '{}'",
                self.app_config.effective_storage_backend(),
                new_storage_location
            ));
        }

        if !self.has_data_file || new_storage_location == old_storage_location {
            return;
        }

        let new_backend = self.app_config.effective_storage_backend().to_string();
        let old_backend = old_config.effective_storage_backend().to_string();
        let old_storage_location_owned = old_storage_location.to_string();
        let old_path_exists = std::path::Path::new(old_storage_location).exists();

        let (tx, rx) = tokio::sync::oneshot::channel();

        let new_storage_clone = new_storage_location.clone();
        let new_backend_clone = new_backend.clone();
        let store_manager = self.store_manager.clone();
        tokio::spawn(async move {
            let file_existed = std::path::Path::new(&new_storage_clone).exists();
            let result: Result<bool, String> = async {
                if !file_existed && old_path_exists {
                    store_manager
                        .migrate_store(
                            &old_backend,
                            &old_storage_location_owned,
                            &new_backend_clone,
                            &new_storage_clone,
                        )
                        .await
                        .map_err(|e| format!("Migration failed: {}", e))?;
                }

                store_manager
                    .validate_store_readable(&new_backend_clone, &new_storage_clone)
                    .await
                    .map_err(|e| format!("Invalid storage file: {}", e))?;
                Ok(file_existed)
            }
            .await;

            let _ = tx.send(result);
        });

        self.migration_state = MigrationState::Migrating {
            old_config: Box::new(old_config),
            old_storage_location: old_storage_location.to_string(),
            result_rx: rx,
        };
        self.set_success("Migrating storage...".to_string());
    }

    pub async fn handle_migration_complete(
        &mut self,
        old_config: kanban_core::AppConfig,
        result: Result<bool, String>,
    ) {
        self.migration_state = crate::app::MigrationState::Idle;
        self.quit_with_migration = false;

        let file_existed = match result {
            Ok(existed) => existed,
            Err(e) => {
                self.set_app_config(old_config);
                self.set_error(e);
                return;
            }
        };

        let new_storage_location =
            kanban_service::config::resolve_storage_location(&self.app_config);

        let new_backend = match self
            .store_manager
            .make_backend(&new_storage_location, &self.app_config)
            .await
        {
            Ok(b) => b,
            Err(e) => {
                self.set_app_config(old_config);
                self.set_error(format!("Store swap failed: {}", e));
                return;
            }
        };

        self.ctx.replace_backend(new_backend);
        if let Some(watcher) = &self.persistence.file_watcher {
            watcher.set_own_instance_id(self.ctx.backend().instance_id());
        }
        let (save_rx, completion_rx) = self.ctx.save_coordinator.reset_save_channels();
        use crate::state::snapshot::TuiSnapshot;
        let snapshot = kanban_domain::Snapshot::from_app(self);
        if let Err(e) = snapshot.apply_to_app(self) {
            tracing::error!("Failed to apply snapshot: {}", e);
        }
        self.ctx.mark_clean();
        if let Err(e) = self.ctx.clear_history() {
            tracing::error!("Failed to clear history: {}", e);
        }

        // The backend now serves a different file, so the selection below must be
        // computed from that file's boards rather than the outgoing one's.
        self.reload_model();

        self.selection.active_board_id = self.model.live_boards().next().map(|b| b.id);
        let live_ids: Vec<uuid::Uuid> = self.model.live_boards().map(|b| b.id).collect();
        self.board_list.update_boards(live_ids);
        self.selection.active_card_id = None;
        self.selection.card_navigation_history.clear();

        self.persistence.save_file = Some(new_storage_location.clone());
        self.persistence.save_completion_rx = Some(completion_rx);
        self.spawn_save_worker(save_rx, None);
        self.cli_file_override = false;
        self.cli_file_provided = false;
        let msg = if file_existed {
            format!("Loaded from {}", new_storage_location)
        } else {
            format!("Migrated to {}", new_storage_location)
        };
        self.set_success(msg);
    }

    pub async fn await_migration(&mut self) {
        use crate::app::MigrationState;
        let (old_config, rx) =
            match std::mem::replace(&mut self.migration_state, MigrationState::Idle) {
                MigrationState::Migrating {
                    old_config,
                    result_rx,
                    ..
                } => (old_config, result_rx),
                MigrationState::Idle => return,
            };
        if let Ok(result) = rx.await {
            self.handle_migration_complete(*old_config, result).await;
        }
    }

    pub(crate) fn open_config_editor(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
    ) -> bool {
        let format = EditFormat::parse(self.app_config.effective_editing_format());
        let ext = format.file_extension();
        let mut dto = AppConfigDto::from_config(&self.app_config, self.has_data_file);
        if self.cli_file_override {
            dto.storage_backend = if self.config_storage_backend.is_empty() {
                None
            } else {
                Some(self.config_storage_backend.clone())
            };
            dto.storage_location = if self.config_storage_location.is_empty() {
                None
            } else {
                Some(self.config_storage_location.clone())
            };
        }
        let serialized = format.serialize(&dto).unwrap_or_else(|_| "{}".to_string());
        let current_content = if self.cli_file_override {
            format.comment_storage_fields(&serialized)
        } else {
            serialized
        };
        let temp_file = std::env::temp_dir().join(format!("kanban_config_edit.{}", ext));
        match edit_in_external_editor(terminal, event_handler, temp_file, &current_content) {
            Ok(Some(new_content)) => {
                if new_content.trim() != current_content.trim() {
                    if let Err(e) = self.apply_config_edit(&new_content, &format) {
                        self.set_error(e);
                    }
                }
            }
            Ok(None) => {}
            Err(e) => {
                self.set_error(format!("Failed to edit config: {}", e));
            }
        }
        true
    }

    pub fn handle_settings_key(
        &mut self,
        key: KeyCode,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        event_handler: &EventHandler,
    ) -> bool {
        match key {
            KeyCode::Enter if self.focus.settings_focus == SettingsFocus::Configuration => {
                self.open_config_editor(terminal, event_handler)
            }
            KeyCode::Char('1')
            | KeyCode::Char('2')
            | KeyCode::Char('3')
            | KeyCode::Char('j')
            | KeyCode::Down
            | KeyCode::Char('k')
            | KeyCode::Up
            | KeyCode::Char('h')
            | KeyCode::Left
            | KeyCode::Char('l')
            | KeyCode::Right
            | KeyCode::Enter => self.handle_settings_key_nav(key),
            KeyCode::Char('e') => self.open_config_editor(terminal, event_handler),
            KeyCode::Char('x') => {
                self.open_export_boards_dialog();
                false
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.pop_mode();
                false
            }
            _ => false,
        }
    }

    pub fn handle_settings_key_nav(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Char('1') => {
                self.focus.settings_focus = SettingsFocus::Configuration;
                self.selection
                    .settings_config
                    .auto_select_first_if_empty(true);
            }
            KeyCode::Char('2') => {
                self.focus.settings_focus = SettingsFocus::ConfigFile;
                self.selection
                    .settings_config_file
                    .auto_select_first_if_empty(true);
            }
            KeyCode::Char('3') => {
                self.focus.settings_focus = SettingsFocus::Storage;
                self.selection
                    .settings_storage
                    .auto_select_first_if_empty(true);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.handle_settings_nav_down();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.handle_settings_nav_up();
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if self.focus.settings_focus == SettingsFocus::Storage {
                    self.focus.settings_focus = SettingsFocus::Configuration;
                    self.selection
                        .settings_config
                        .auto_select_first_if_empty(true);
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if self.focus.settings_focus != SettingsFocus::Storage {
                    self.focus.settings_focus = SettingsFocus::Storage;
                    self.selection
                        .settings_storage
                        .auto_select_first_if_empty(true);
                }
            }
            KeyCode::Enter
                if self.focus.settings_focus == SettingsFocus::Storage
                    && self.selection.settings_storage.get()
                        == Some(EXPORT_BUTTON_STORAGE_INDEX) =>
            {
                return self.trigger_export();
            }
            _ => {}
        }
        false
    }

    pub fn handle_settings_nav_down(&mut self) {
        match self.focus.settings_focus {
            SettingsFocus::Configuration => {
                let count = self.settings_item_count(SettingsFocus::Configuration);
                let current = self.selection.settings_config.get().unwrap_or(0);
                if self.cli_file_override && self.has_data_file {
                    match current {
                        c if c >= count - 1 => {
                            self.focus.settings_focus = SettingsFocus::ConfigFile;
                            self.selection.settings_config_file.set(Some(0));
                        }
                        4 => self.selection.settings_config.set(Some(7)),
                        _ => self.selection.settings_config.next(count),
                    }
                } else if current >= count - 1 {
                    self.focus.settings_focus = SettingsFocus::ConfigFile;
                    self.selection.settings_config_file.set(Some(0));
                } else {
                    self.selection.settings_config.next(count);
                }
            }
            SettingsFocus::ConfigFile => {
                let count = self.settings_item_count(SettingsFocus::ConfigFile);
                let current = self.selection.settings_config_file.get().unwrap_or(0);
                if current >= count - 1 {
                    self.focus.settings_focus = SettingsFocus::Configuration;
                    self.selection.settings_config.set(Some(0));
                } else {
                    self.selection.settings_config_file.next(count);
                }
            }
            SettingsFocus::Storage => {
                let count = self.settings_item_count(SettingsFocus::Storage);
                let current = self.selection.settings_storage.get().unwrap_or(0);
                if current >= count - 1 {
                    self.selection.settings_storage.set(Some(0));
                } else {
                    self.selection.settings_storage.next(count);
                }
            }
        }
    }

    fn handle_settings_nav_up(&mut self) {
        match self.focus.settings_focus {
            SettingsFocus::Configuration => {
                let current = self.selection.settings_config.get().unwrap_or(0);
                if current == 0 {
                    let count = self.settings_item_count(SettingsFocus::ConfigFile);
                    self.focus.settings_focus = SettingsFocus::ConfigFile;
                    self.selection.settings_config_file.set(Some(count - 1));
                } else if self.cli_file_override && self.has_data_file && current == 7 {
                    self.selection.settings_config.set(Some(4));
                } else {
                    self.selection.settings_config.prev();
                }
            }
            SettingsFocus::ConfigFile => {
                let current = self.selection.settings_config_file.get().unwrap_or(0);
                if current == 0 {
                    let count = self.settings_item_count(SettingsFocus::Configuration);
                    self.focus.settings_focus = SettingsFocus::Configuration;
                    self.selection.settings_config.set(Some(count - 1));
                } else {
                    self.selection.settings_config_file.prev();
                }
            }
            SettingsFocus::Storage => {
                let current = self.selection.settings_storage.get().unwrap_or(0);
                if current == 0 {
                    let count = self.settings_item_count(SettingsFocus::Storage);
                    self.selection.settings_storage.set(Some(count - 1));
                } else {
                    self.selection.settings_storage.prev();
                }
            }
        }
    }

    fn trigger_export(&mut self) -> bool {
        let live_ids: Vec<uuid::Uuid> = self.model.live_boards().map(|b| b.id).collect();
        if live_ids.is_empty() {
            self.set_error("No boards to export".to_string());
            return false;
        }
        self.export_dialog = Some(ExportDialogState::from_selection(
            &live_ids,
            &self.multi_select.selected_boards,
        ));
        self.push_mode(AppMode::Dialog(DialogMode::ExportBoards));
        false
    }

    pub fn handle_export_boards_dialog(&mut self, key_code: KeyCode) {
        let Some(ref mut dialog) = self.export_dialog else {
            return;
        };

        match dialog.step {
            crate::app::ExportStep::SelectBoards => match key_code {
                KeyCode::Char(' ') => {
                    dialog.toggle(dialog.cursor);
                }
                KeyCode::Char('a') => {
                    dialog.select_all();
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    let len = dialog.board_selections.len();
                    if len > 0 {
                        dialog.cursor = (dialog.cursor + 1) % len;
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    let len = dialog.board_selections.len();
                    if len > 0 {
                        dialog.cursor = (dialog.cursor + len - 1) % len;
                    }
                }
                KeyCode::Enter => {
                    if dialog.any_selected() {
                        dialog.step = crate::app::ExportStep::ExportOptions;
                    }
                }
                KeyCode::Esc => {
                    self.export_dialog = None;
                    self.pop_mode();
                }
                _ => {}
            },
            crate::app::ExportStep::ExportOptions => match key_code {
                KeyCode::Tab | KeyCode::BackTab => {
                    dialog.format = match dialog.format {
                        crate::app::ExportFormat::Json => crate::app::ExportFormat::Sqlite,
                        crate::app::ExportFormat::Sqlite => crate::app::ExportFormat::Json,
                    };
                    let stem = dialog
                        .filename
                        .rsplit_once('.')
                        .map(|(s, _)| s)
                        .unwrap_or(&dialog.filename)
                        .to_string();
                    dialog.filename = match dialog.format {
                        crate::app::ExportFormat::Json => format!("{}.json", stem),
                        crate::app::ExportFormat::Sqlite => format!("{}.sqlite", stem),
                    };
                }
                KeyCode::Backspace => {
                    dialog.filename.pop();
                }
                KeyCode::Char(c)
                    if !matches!(
                        c,
                        '/' | '\\' | '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                    ) =>
                {
                    dialog.filename.push(c);
                }
                KeyCode::Enter => {
                    self.execute_export();
                }
                KeyCode::Esc => {
                    dialog.step = crate::app::ExportStep::SelectBoards;
                }
                _ => {}
            },
        }
    }

    fn execute_export(&mut self) {
        let Some(ref dialog) = self.export_dialog else {
            return;
        };

        let filename = dialog.filename.clone();
        let selected_board_ids: Vec<uuid::Uuid> = dialog
            .board_ids
            .iter()
            .zip(dialog.board_selections.iter())
            .filter(|(_, &selected)| selected)
            .map(|(&id, _)| id)
            .collect();

        if selected_board_ids.is_empty() || filename.is_empty() {
            self.set_error("No boards selected or filename empty".to_string());
            return;
        }

        // Route through the snapshot so each selected board's archived-card live
        // rows and markers round-trip.
        let export = match self.build_boards_export(&selected_board_ids) {
            Ok(export) => export,
            Err(e) => {
                self.set_error(format!("Export failed: {}", e));
                return;
            }
        };

        match dialog.format {
            crate::app::ExportFormat::Json => {
                match BoardExporter::export_to_file(&export, &filename) {
                    Ok(()) => self.set_success(format!("Exported to {}", filename)),
                    Err(e) => self.set_error(format!("Export failed: {}", e)),
                }
            }
            crate::app::ExportFormat::Sqlite => {
                let filename_clone = filename.clone();
                let export_clone = export.clone();
                let store_manager = self.store_manager.clone();
                let config_clone = self.app_config.clone();
                let (tx, rx) = tokio::sync::oneshot::channel();
                tokio::spawn(async move {
                    let result = store_manager
                        .export_to_sqlite(export_clone, &filename_clone, &config_clone)
                        .await
                        .map(|_| filename_clone)
                        .map_err(|e| format!("Export failed: {}", e));
                    let _ = tx.send(result);
                });
                self.export_result_rx = Some(rx);
                self.set_success("Exporting...".to_string());
            }
        }

        self.export_dialog = None;
        self.pop_mode();
    }

    /// Open the export-all-boards dialog, or set an error if there are no
    /// live boards to export. Matches the direct `x` keypress's guard exactly.
    pub(crate) fn open_export_boards_dialog(&mut self) {
        let live_ids: Vec<uuid::Uuid> = self.model.live_boards().map(|b| b.id).collect();
        if live_ids.is_empty() {
            self.set_error("No boards to export".to_string());
            return;
        }
        self.export_dialog = Some(ExportDialogState::from_selection(
            &live_ids,
            &self.multi_select.selected_boards,
        ));
        self.push_mode(AppMode::Dialog(DialogMode::ExportBoards));
    }
}
