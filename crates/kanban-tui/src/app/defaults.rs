use super::types::default_store_manager;
use super::{
    AnimationState, App, AppMode, DialogInputState, FilterState, FocusState, MigrationState,
    MultiSelectState, PersistenceState, RelationshipState, SelectionHub, SprintViewState,
    StorageBackendChoice, UiState, ViewState,
};
use kanban_core::InputState;
use kanban_view::board_list::BoardList;
use kanban_view::model::Model;
use std::sync::{Arc, Mutex};

#[doc(hidden)]
impl Default for App {
    fn default() -> Self {
        Self::test_default()
    }
}

impl App {
    /// A path under the OS temp dir, unique to this test process, that
    /// nothing ever writes to. See [`App::test_default`] for why this must
    /// never resolve to a real file on disk.
    #[doc(hidden)]
    pub fn test_default_configuration_location() -> String {
        std::env::temp_dir()
            .join(format!(
                "kanban-test-default-no-real-config-{}.toml",
                std::process::id()
            ))
            .display()
            .to_string()
    }

    /// The only writer of the app config. The context holds its own copy and
    /// reads prefix defaults from it at allocation, export and import time, so
    /// a write that updates one and not the other splits the session.
    pub(crate) fn set_app_config(&mut self, config: kanban_core::AppConfig) {
        self.ctx.set_app_config(config.clone());
        self.app_config = config;
    }

    #[doc(hidden)]
    pub fn test_default() -> Self {
        // Leaving configuration_location unset here would make
        // effective_configuration_location fall back to the real
        // $HOME/.config/kanban/config.toml (or KANBAN_CONFIG), which lets a
        // test that edits config and moves to its own location delete or
        // overwrite that real file as a side effect.
        let app_config = kanban_core::AppConfig {
            configuration_location: Some(Self::test_default_configuration_location()),
            ..Default::default()
        };
        let backend = std::sync::Arc::new(kanban_backend_memory::InMemoryStore::new());
        let inner = kanban_service::KanbanContext::open_deferred(backend, app_config.clone());
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
            app_config,
            selection: SelectionHub::default(),
            board_list: BoardList::new(),
            animation: AnimationState::default(),
            filter: FilterState::default(),
            dialog_input: DialogInputState::default(),
            focus: FocusState::default(),
            persistence: PersistenceState::new(None, save_completion_rx),
            multi_select: MultiSelectState::default(),
            ui_state: UiState::default(),
            sprint_view: SprintViewState::default(),
            view: ViewState::default(),
            model: Model::default(),
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

    #[test]
    fn test_default_app_config_configuration_location_is_never_the_real_ambient_path() {
        let app = App::test_default();

        assert!(
            app.app_config.configuration_location.is_some(),
            "test_default() must pin configuration_location, or \
             effective_configuration_location falls back to the real \
             $HOME/.config/kanban/config.toml (or KANBAN_CONFIG)"
        );
        assert_ne!(
            app.app_config.configuration_location,
            kanban_service::config::config_path().map(|p| p.display().to_string()),
            "test_default()'s configuration_location must never equal the \
             real ambient config path — a test that edits config and moves \
             to its own location would delete or overwrite that real file"
        );
    }
}
