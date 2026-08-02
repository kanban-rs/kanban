use super::{
    AnimationState, DialogInputState, FilterState, FocusState, MultiSelectState, PersistenceState,
    RelationshipState, SelectionHub, SprintViewState, UiState, ViewState,
};
use crate::app::AppMode;
use crate::tui_context::TuiContext;
use kanban_core::{AppConfig, InputState};
use kanban_service::StoreManager;
use std::sync::{Arc, Mutex};

/// Builds a `StoreManager` that mirrors the default CLI registry: SQLite
/// first (so content-sniffing prefers it) and JSON second as a catch-all
/// fallback. Used by [`App::new`] as the default backend configuration.
pub(in crate::app) fn default_store_manager() -> StoreManager {
    let mut registry = kanban_persistence::StoreRegistry::new();
    let mut backends = kanban_backend::KanbanBackendRegistry::new();
    registry.register(Box::new(kanban_persistence_sqlite::SqliteStoreFactory));
    backends.register(Box::new(kanban_persistence_sqlite::SqliteBackendFactory));
    registry.register(Box::new(kanban_persistence_json::JsonStoreFactory));
    backends.register(Box::new(kanban_persistence_json::JsonBackendFactory));
    StoreManager::new(registry, backends)
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
    pub model: super::model::Model,
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
        // Boxed to keep the enum small: `AppConfig` is by far the largest field
        // and the `Idle` variant carries nothing (clippy::large_enum_variant).
        old_config: Box<AppConfig>,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
