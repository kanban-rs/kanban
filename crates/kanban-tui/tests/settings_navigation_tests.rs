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

// --- cli_file_override navigation tests (descriptor-based, no skipping) ---
//
// Under the descriptor model the Configuration panel's navigation is a plain
// sequential walk over its *selectable* rows, in every mode. The two `Storage`
// rows the override + storage-configured sub-mode renders are non-selectable,
// so they simply do not appear in the walk -- there is no "skip" to assert
// because the walk only ever visits selectable rows. The new Search Jupto row
// is the last selectable row in every mode, so it is the one that wraps to the
// `ConfigFile` panel. The old special-case `set(8)` hack (a cell-index jump over
// the two non-selectable rows) is gone: `next(count)` already lands on the
// right selectable row by index.

#[test]
fn test_settings_j_traverses_the_selectable_rows_sequentially_in_override_mode() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    app.cli_file_override = true;
    app.has_data_file = true;
    app.original_storage_backend = Some("json".into());
    app.original_storage_location = Some("./b".into());
    app.focus.settings_focus = SettingsFocus::Configuration;

    // Selectable set in override mode: base 6 (0-5) + Active Storage pair (6-7)
    // + Search Jupto (8). The two non-selectable styled `Storage` rows are absent
    // from this set, so the walk is a plain sequence: 5 -> 6 -> 7 -> 8 -> ConfigFile.

    app.selection.settings_config.set(Some(5)); // Background Highlight
    app.handle_settings_key_nav(KeyCode::Char('j'));
    assert_eq!(
        app.selection.settings_config.get(),
        Some(6),
        "j from Background Highlight (selectable 5) lands on Active Storage Backend (selectable 6)"
    );

    app.handle_settings_key_nav(KeyCode::Char('j'));
    assert_eq!(
        app.selection.settings_config.get(),
        Some(7),
        "j from Active Storage Backend lands on Active Storage Location"
    );

    app.handle_settings_key_nav(KeyCode::Char('j'));
    assert_eq!(
        app.selection.settings_config.get(),
        Some(8),
        "j from Active Storage Location lands on Search Jupto (the new last row)"
    );

    app.handle_settings_key_nav(KeyCode::Char('j'));
    assert_eq!(
        app.focus.settings_focus,
        SettingsFocus::ConfigFile,
        "j from Search Jupto (the last selectable row) wraps to the ConfigFile panel"
    );
    assert_eq!(
        app.selection.settings_config_file.get(),
        Some(0),
        "entering ConfigFile lands on its first row"
    );
}

#[test]
fn test_settings_k_traverses_the_selectable_rows_sequentially_in_reverse() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    app.cli_file_override = true;
    app.has_data_file = true;
    app.original_storage_backend = Some("json".into());
    app.original_storage_location = Some("./b".into());
    app.focus.settings_focus = SettingsFocus::Configuration;

    app.selection.settings_config.set(Some(8)); // Search Jupto
    app.handle_settings_key_nav(KeyCode::Char('k'));
    assert_eq!(
        app.selection.settings_config.get(),
        Some(7),
        "k from Search Jupto lands on Active Storage Location"
    );

    app.handle_settings_key_nav(KeyCode::Char('k'));
    assert_eq!(
        app.selection.settings_config.get(),
        Some(6),
        "k from Active Storage Location lands on Active Storage Backend"
    );

    app.handle_settings_key_nav(KeyCode::Char('k'));
    assert_eq!(
        app.selection.settings_config.get(),
        Some(5),
        "k from Active Storage Backend lands on Background Highlight"
    );
}

#[test]
fn test_settings_config_file_lands_on_last_selectable_row() {
    use crossterm::event::KeyCode;

    // Coming from the ConfigFile panel into Configuration must land on the last
    // selectable row (Search Jupto), regardless of mode. We exercise the three
    // distinct sub-modes by setting the flags directly and re-using the same
    // three assertions each time.
    let run = |app: &mut kanban_tui::App| {
        app.focus.settings_focus = SettingsFocus::ConfigFile;
        app.selection.settings_config_file.set(Some(0));
        app.handle_settings_key_nav(KeyCode::Char('k'));
        let last = app.config_panel_selectable_count(SettingsFocus::Configuration) - 1;
        assert_eq!(
            app.focus.settings_focus,
            SettingsFocus::Configuration,
            "k from ConfigFile must enter the Configuration panel"
        );
        assert_eq!(
            app.selection.settings_config.get(),
            Some(last),
            "entering Configuration must land on the last selectable row (Search Jumpto)"
        );
    };

    // no_data
    let mut app = helpers::setup_settings_app();
    app.has_data_file = false;
    run(&mut app);
    // data_no_override
    let mut app = helpers::setup_settings_app();
    app.has_data_file = true;
    app.cli_file_override = false;
    run(&mut app);
    // data_override + storage configured
    let mut app = helpers::setup_settings_app();
    app.has_data_file = true;
    app.cli_file_override = true;
    app.original_storage_backend = Some("json".into());
    app.original_storage_location = Some("./b".into());
    run(&mut app);
}
