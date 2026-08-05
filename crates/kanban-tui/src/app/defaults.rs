use super::types::default_store_manager;
use super::{
    AnimationState, App, AppMode, DialogInputState, FilterState, FocusState, MigrationState,
    MultiSelectState, PersistenceState, RelationshipState, SelectionHub, SprintViewState,
    StorageBackendChoice, UiState, ViewState,
};
use kanban_core::InputState;
use std::sync::{Arc, Mutex};

impl Default for App {
    fn default() -> Self {
        Self::test_default()
    }
}

impl App {
    #[doc(hidden)]
    pub fn test_default() -> Self {
        // Without an explicit configuration_location, effective_configuration_location
        // falls back to the real $HOME/.config/kanban/config.toml (or KANBAN_CONFIG).
        // A test that then edits config and specifies its own location sees this as
        // the OLD location relative to its new one and deletes it — pointing this at
        // a path nothing ever writes to means "old" is never a real file on disk.
        let app_config = kanban_core::AppConfig {
            configuration_location: Some(
                std::env::temp_dir()
                    .join("kanban-test-default-no-real-config.toml")
                    .display()
                    .to_string(),
            ),
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
            animation: AnimationState::default(),
            filter: FilterState::default(),
            dialog_input: DialogInputState::default(),
            focus: FocusState::default(),
            persistence: PersistenceState::new(None, save_completion_rx),
            multi_select: MultiSelectState::default(),
            ui_state: UiState::default(),
            sprint_view: SprintViewState::default(),
            view: ViewState::default(),
            model: super::model::Model::default(),
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
