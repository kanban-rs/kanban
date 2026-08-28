mod helpers;

use kanban_tui::app::focus::SettingsFocus;
use kanban_tui::app::mode::{AppMode, DialogMode};
use kanban_tui::keybindings::KeybindingRegistry;

#[test]
fn test_settings_open_initializes_focus_and_cursor() {
    let app = helpers::setup_settings_app();
    assert_eq!(app.focus.settings_focus, SettingsFocus::Configuration);
    assert_eq!(app.selection.settings_config.get(), Some(0));
}

#[test]
fn test_settings_j_moves_cursor_down() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    assert_eq!(app.selection.settings_config.get(), Some(0));

    app.handle_settings_key_nav(KeyCode::Char('j'));
    assert_eq!(app.selection.settings_config.get(), Some(1));

    app.handle_settings_key_nav(KeyCode::Char('j'));
    assert_eq!(app.selection.settings_config.get(), Some(2));
}

#[test]
fn test_settings_k_moves_cursor_up() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    app.selection.settings_config.set(Some(2));

    app.handle_settings_key_nav(KeyCode::Char('k'));
    assert_eq!(app.selection.settings_config.get(), Some(1));

    app.handle_settings_key_nav(KeyCode::Char('k'));
    assert_eq!(app.selection.settings_config.get(), Some(0));
}

#[test]
fn test_settings_j_crosses_config_to_config_file() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    let last_idx = app.settings_item_count(SettingsFocus::Configuration) - 1;
    app.selection.settings_config.set(Some(last_idx));

    app.handle_settings_key_nav(KeyCode::Char('j'));
    assert_eq!(app.focus.settings_focus, SettingsFocus::ConfigFile);
    assert_eq!(app.selection.settings_config_file.get(), Some(0));
}

#[test]
fn test_settings_k_crosses_config_file_to_config() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    app.focus.settings_focus = SettingsFocus::ConfigFile;
    app.selection.settings_config_file.set(Some(0));

    app.handle_settings_key_nav(KeyCode::Char('k'));
    assert_eq!(app.focus.settings_focus, SettingsFocus::Configuration);
    let expected = app.settings_item_count(SettingsFocus::Configuration) - 1;
    assert_eq!(app.selection.settings_config.get(), Some(expected));
}

#[test]
fn test_settings_j_wraps_config_file_to_config() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    app.focus.settings_focus = SettingsFocus::ConfigFile;
    let last_idx = app.settings_item_count(SettingsFocus::ConfigFile) - 1;
    app.selection.settings_config_file.set(Some(last_idx));

    app.handle_settings_key_nav(KeyCode::Char('j'));
    assert_eq!(app.focus.settings_focus, SettingsFocus::Configuration);
    assert_eq!(app.selection.settings_config.get(), Some(0));
}

#[test]
fn test_settings_storage_wraps_down() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    app.focus.settings_focus = SettingsFocus::Storage;
    let last_idx = app.settings_item_count(SettingsFocus::Storage) - 1;
    app.selection.settings_storage.set(Some(last_idx));

    app.handle_settings_key_nav(KeyCode::Char('j'));
    assert_eq!(app.focus.settings_focus, SettingsFocus::Storage);
    assert_eq!(app.selection.settings_storage.get(), Some(0));
}

#[test]
fn test_settings_storage_wraps_up() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    app.focus.settings_focus = SettingsFocus::Storage;
    app.selection.settings_storage.set(Some(0));

    app.handle_settings_key_nav(KeyCode::Char('k'));
    assert_eq!(app.focus.settings_focus, SettingsFocus::Storage);
    let expected = app.settings_item_count(SettingsFocus::Storage) - 1;
    assert_eq!(app.selection.settings_storage.get(), Some(expected));
}

#[test]
fn test_settings_h_l_switches_columns() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    assert_eq!(app.focus.settings_focus, SettingsFocus::Configuration);

    app.handle_settings_key_nav(KeyCode::Char('l'));
    assert_eq!(app.focus.settings_focus, SettingsFocus::Storage);

    app.handle_settings_key_nav(KeyCode::Char('h'));
    assert_eq!(app.focus.settings_focus, SettingsFocus::Configuration);

    app.handle_settings_key_nav(KeyCode::Char('h'));
    assert_eq!(app.focus.settings_focus, SettingsFocus::Configuration);
}

#[test]
fn test_settings_1_2_3_jumps_to_panel() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();

    app.handle_settings_key_nav(KeyCode::Char('3'));
    assert_eq!(app.focus.settings_focus, SettingsFocus::Storage);

    app.handle_settings_key_nav(KeyCode::Char('2'));
    assert_eq!(app.focus.settings_focus, SettingsFocus::ConfigFile);

    app.handle_settings_key_nav(KeyCode::Char('1'));
    assert_eq!(app.focus.settings_focus, SettingsFocus::Configuration);
}

#[test]
fn test_settings_enter_on_export_triggers_dialog() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    use kanban_domain::KanbanOperations;
    app.ctx.create_board("B1".into(), None).unwrap();
    app.reload_model();
    app.prepare_frame();
    app.focus.settings_focus = SettingsFocus::Storage;
    app.selection.settings_storage.set(Some(3));

    app.handle_settings_key_nav(KeyCode::Enter);
    assert!(matches!(
        app.mode,
        AppMode::Dialog(DialogMode::ExportBoards)
    ));
}

#[test]
fn test_render_settings_active_panel_has_focus_indicator() {
    let mut app = helpers::setup_settings_app();
    app.focus.settings_focus = SettingsFocus::Configuration;
    let output = helpers::render_to_string(&app);
    assert!(
        output.contains("Configuration [1]"),
        "Missing focus indicator for Configuration"
    );

    app.focus.settings_focus = SettingsFocus::Storage;
    let output = helpers::render_to_string(&app);
    assert!(
        output.contains("Storage [3]"),
        "Missing focus indicator for Storage"
    );
}

#[test]
fn test_render_settings_inactive_panel_no_indicator() {
    let mut app = helpers::setup_settings_app();
    app.focus.settings_focus = SettingsFocus::Storage;
    let output = helpers::render_to_string(&app);
    assert!(
        !output.contains("Configuration [1]"),
        "Configuration should not show [1] when unfocused"
    );
}

#[test]
fn test_settings_keybinding_provider_includes_nav_bindings() {
    let mut app = helpers::setup_settings_app();
    app.focus.settings_focus = SettingsFocus::Configuration;

    let provider = KeybindingRegistry::get_provider(&app);
    let context = provider.get_context();
    let keys: Vec<&str> = context.bindings.iter().map(|b| b.key.as_str()).collect();
    assert!(keys.contains(&"1"));
    assert!(keys.contains(&"j/k"));
    assert!(keys.contains(&"h/l"));
    assert!(keys.contains(&"q/Esc"));
}

// --- cli_file_override navigation skip tests ---

#[test]
fn test_settings_j_skips_greyed_storage_fields_reaches_active_lines() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    app.cli_file_override = true;
    app.has_data_file = true;
    app.focus.settings_focus = SettingsFocus::Configuration;
    app.selection.settings_config.set(Some(4));

    app.handle_settings_key_nav(KeyCode::Char('j'));

    assert_eq!(
        app.focus.settings_focus,
        SettingsFocus::Configuration,
        "j from index 4 should stay in Configuration (landing on Active Storage Backend)"
    );
    assert_eq!(
        app.selection.settings_config.get(),
        Some(7),
        "should jump to index 7 (Active Storage Backend), skipping greyed indices 5 and 6"
    );
}

#[test]
fn test_settings_j_from_active_lines_goes_to_config_file() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    app.cli_file_override = true;
    app.has_data_file = true;
    app.focus.settings_focus = SettingsFocus::Configuration;
    app.selection.settings_config.set(Some(8));

    app.handle_settings_key_nav(KeyCode::Char('j'));

    assert_eq!(app.focus.settings_focus, SettingsFocus::ConfigFile);
}

#[test]
fn test_settings_k_from_config_file_lands_on_active_storage_location() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    app.cli_file_override = true;
    app.has_data_file = true;
    app.focus.settings_focus = SettingsFocus::ConfigFile;
    app.selection.settings_config_file.set(Some(0));

    app.handle_settings_key_nav(KeyCode::Char('k'));

    assert_eq!(app.focus.settings_focus, SettingsFocus::Configuration);
    assert_eq!(
        app.selection.settings_config.get(),
        Some(8),
        "should land on index 8 (Active Storage Location)"
    );
}

#[test]
fn test_settings_k_from_active_backend_skips_to_editing_format() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    app.cli_file_override = true;
    app.has_data_file = true;
    app.focus.settings_focus = SettingsFocus::Configuration;
    app.selection.settings_config.set(Some(7));

    app.handle_settings_key_nav(KeyCode::Char('k'));

    assert_eq!(
        app.selection.settings_config.get(),
        Some(4),
        "k from Active Storage Backend should skip greyed indices and land on Editing Format (4)"
    );
}
