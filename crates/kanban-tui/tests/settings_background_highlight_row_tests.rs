mod helpers;

use kanban_tui::app::focus::SettingsFocus;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use ratatui::Terminal;

fn render_settings(app: &kanban_tui::App) -> Buffer {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render_settings_view(app, frame, frame.area());
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn render_full(app: &mut kanban_tui::App) -> Buffer {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render(app, frame);
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

fn row_text(buffer: &Buffer, y: u16) -> String {
    let mut row = String::new();
    for x in 0..buffer.area.width {
        row.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
    }
    row
}

fn find_row_containing(buffer: &Buffer, needle: &str) -> u16 {
    for y in 0..buffer.area.height {
        if row_text(buffer, y).contains(needle) {
            return y;
        }
    }
    panic!("row containing {needle:?} not found in buffer");
}

fn find_col_on_row(buffer: &Buffer, y: u16, needle: &str) -> u16 {
    let row = row_text(buffer, y);
    let byte_idx = row.find(needle).unwrap_or_else(|| {
        panic!("text {needle:?} not found on row {y} ({row:?})");
    });
    row[..byte_idx].chars().count() as u16
}

/// The contiguous run of cells on `y` whose background equals `bg`, as
/// (symbol, fg) pairs in left-to-right order.
fn bg_run(buffer: &Buffer, y: u16, bg: Color) -> Vec<(String, Color)> {
    let mut run = Vec::new();
    let mut started = false;
    for x in 0..buffer.area.width {
        let cell = buffer.cell((x, y)).unwrap();
        if cell.bg == bg {
            started = true;
            run.push((cell.symbol().to_string(), cell.fg));
        } else if started {
            break;
        }
    }
    run
}

#[test]
fn test_background_highlight_row_shows_default_when_unset() {
    let app = helpers::setup_settings_app();
    let buffer = render_settings(&app);
    let y = find_row_containing(&buffer, "Background Highlight: default");
    assert!(row_text(&buffer, y).contains("Background Highlight: default"));
}

#[test]
fn test_background_highlight_row_shows_the_configured_colour_as_uppercase_hex() {
    let mut app = helpers::setup_settings_app();
    app.app_config.background_highlight = Some("#2e3440".to_string());
    let buffer = render_settings(&app);
    let y = find_row_containing(&buffer, "Background Highlight: #2E3440");
    assert!(row_text(&buffer, y).contains("Background Highlight: #2E3440"));
}

#[test]
fn test_background_highlight_row_shows_default_for_an_invalid_value() {
    let mut app = helpers::setup_settings_app();
    app.app_config.background_highlight = Some("blue".to_string());
    let buffer = render_settings(&app);
    let y = find_row_containing(&buffer, "Background Highlight: default");
    assert!(row_text(&buffer, y).contains("Background Highlight: default"));
}

#[test]
fn test_background_highlight_swatch_draws_sample_text_in_white_on_the_configured_colour() {
    let mut app = helpers::setup_settings_app();
    app.app_config.background_highlight = Some("#2E3440".to_string());
    let configured = Color::Rgb(0x2E, 0x34, 0x40);
    let buffer = render_settings(&app);
    let y = find_row_containing(&buffer, "Background Highlight: #2E3440");
    let run = bg_run(&buffer, y, configured);
    let symbols: String = run.iter().map(|(s, _)| s.as_str()).collect();
    assert_eq!(symbols, " Sample ", "expected the swatch text on row {y}");
    for (symbol, fg) in run.iter().filter(|(s, _)| s != " ") {
        assert_eq!(
            *fg,
            Color::White,
            "expected white text for {symbol:?} in the swatch"
        );
    }
}

#[test]
fn test_background_highlight_swatch_uses_terminal_blue_by_default() {
    let app = helpers::setup_settings_app();
    let buffer = render_settings(&app);
    let y = find_row_containing(&buffer, "Background Highlight: default");
    let run = bg_run(&buffer, y, Color::Blue);
    let symbols: String = run.iter().map(|(s, _)| s.as_str()).collect();
    assert_eq!(symbols, " Sample ", "expected the swatch text on row {y}");
    for (symbol, fg) in run.iter().filter(|(s, _)| s != " ") {
        assert_eq!(
            *fg,
            Color::White,
            "expected white text for {symbol:?} in the swatch"
        );
    }
}

#[test]
fn test_background_highlight_row_is_index_5_and_selectable() {
    let mut app = helpers::setup_settings_app();
    app.focus.settings_focus = SettingsFocus::Configuration;
    app.selection.settings_config.set(Some(5));
    let buffer = render_settings(&app);

    let highlight_y = find_row_containing(&buffer, "Background Highlight: ");
    let highlight_x = find_col_on_row(&buffer, highlight_y, "Background Highlight: ");
    let highlight_cell = buffer.cell((highlight_x, highlight_y)).unwrap();
    assert_eq!(highlight_cell.bg, app.highlight_bg());

    let editing_y = find_row_containing(&buffer, "Editing Format: ");
    let editing_x = find_col_on_row(&buffer, editing_y, "Editing Format: ");
    let editing_cell = buffer.cell((editing_x, editing_y)).unwrap();
    assert_ne!(editing_cell.bg, app.highlight_bg());
}

#[test]
fn test_other_settings_rows_keep_their_styles() {
    let app = helpers::setup_settings_app();
    let buffer = render_settings(&app);
    let y = find_row_containing(&buffer, "Configuration Format: ");
    let x = find_col_on_row(&buffer, y, "Configuration Format: ");
    let cell = buffer.cell((x, y)).unwrap();
    assert_eq!(cell.bg, Color::Blue);
    assert_eq!(cell.fg, Color::DarkGray);
}

#[test]
fn test_settings_item_count_includes_the_background_highlight_row() {
    let mut app = helpers::setup_settings_app();

    app.has_data_file = false;
    assert_eq!(app.settings_item_count(SettingsFocus::Configuration), 6);

    app.has_data_file = true;
    app.cli_file_override = false;
    assert_eq!(app.settings_item_count(SettingsFocus::Configuration), 8);

    app.cli_file_override = true;
    assert_eq!(app.settings_item_count(SettingsFocus::Configuration), 10);
}

#[test]
fn test_settings_j_from_editing_format_lands_on_background_highlight() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    app.focus.settings_focus = SettingsFocus::Configuration;
    app.selection.settings_config.set(Some(4));
    app.handle_settings_key_nav(KeyCode::Char('j'));
    assert_eq!(app.selection.settings_config.get(), Some(5));

    let mut override_app = helpers::setup_settings_app();
    override_app.has_data_file = true;
    override_app.cli_file_override = true;
    override_app.focus.settings_focus = SettingsFocus::Configuration;
    override_app.selection.settings_config.set(Some(4));
    override_app.handle_settings_key_nav(KeyCode::Char('j'));
    assert_eq!(override_app.selection.settings_config.get(), Some(5));
}

#[test]
fn test_settings_j_from_background_highlight_lands_on_storage_backend() {
    use crossterm::event::KeyCode;

    let mut app = helpers::setup_settings_app();
    app.has_data_file = true;
    app.cli_file_override = false;
    app.focus.settings_focus = SettingsFocus::Configuration;
    app.selection.settings_config.set(Some(5));

    app.handle_settings_key_nav(KeyCode::Char('j'));

    assert_eq!(app.selection.settings_config.get(), Some(6));
    let buffer = render_settings(&app);
    let y = find_row_containing(&buffer, "Storage Backend: ");
    let x = find_col_on_row(&buffer, y, "Storage Backend: ");
    assert_eq!(buffer.cell((x, y)).unwrap().bg, app.highlight_bg());
}

#[test]
fn test_background_highlight_swatch_fits_at_80_columns() {
    let mut app = helpers::setup_settings_app();
    let buffer = render_full(&mut app);
    let y = find_row_containing(&buffer, "Background Highlight: default");
    let row = row_text(&buffer, y);
    assert!(
        row.contains("Background Highlight: default") && row.contains(" Sample "),
        "expected the full label and swatch to fit on row {y}: {row:?}"
    );
}
