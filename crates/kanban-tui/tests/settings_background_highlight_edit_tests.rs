use crossterm::event::{KeyCode, KeyEvent};
use kanban_tui::app::focus::Focus;
use kanban_tui::app::{AppMode, DialogMode};
use kanban_tui::events::EventHandler;
use kanban_tui::keybindings::KeybindingRegistry;
use kanban_tui::App;
use ratatui::backend::{CrosstermBackend, TestBackend};
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::{Terminal, TerminalOptions, Viewport};
use std::io;

fn headless_terminal() -> Terminal<CrosstermBackend<io::Stdout>> {
    Terminal::with_options(
        CrosstermBackend::new(io::stdout()),
        TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 80, 40)),
        },
    )
    .unwrap()
}

fn press(
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    events: &EventHandler,
    code: KeyCode,
) {
    app.handle_key_event(KeyEvent::from(code), terminal, events);
}

fn type_str(
    app: &mut App,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    events: &EventHandler,
    s: &str,
) {
    for c in s.chars() {
        press(app, terminal, events, KeyCode::Char(c));
    }
}

fn settings_app_with_highlight_row_selected() -> App {
    let mut app = App::test_default();
    app.focus.active = Focus::Boards;
    app.handle_open_settings();
    app.selection.settings_config.set(Some(5));
    app
}

fn render_buffer(app: &mut App) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render(app, frame);
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

// Configuration pane starts at (0, 0) at 120x30; its inner area starts at
// (1, 1). The Background Highlight row is index 5, and its value field
// starts after the 22-column "Background Highlight: " label.
const VALUE_X: u16 = 1 + 22;
const ROW_Y: u16 = 1 + 5;
const SAMPLE_FIRST_LETTER_X: u16 = VALUE_X + 7 + 1 + 1;
const SAMPLE_LAST_LETTER_X: u16 = SAMPLE_FIRST_LETTER_X + 5;

#[tokio::test]
async fn test_enter_on_the_background_highlight_row_opens_the_inline_hex_input() {
    let mut app = settings_app_with_highlight_row_selected();
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    press(&mut app, &mut terminal, &events, KeyCode::Enter);

    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::EditBackgroundHighlight)
    );
    assert_eq!(app.input.as_str(), "");
}

#[tokio::test]
async fn test_enter_prefills_the_input_with_the_current_hex() {
    let mut app = settings_app_with_highlight_row_selected();
    app.app_config.background_highlight = Some("#2e3440".to_string());
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    press(&mut app, &mut terminal, &events, KeyCode::Enter);

    assert_eq!(app.input.as_str(), "#2E3440");
}

#[tokio::test]
async fn test_typing_a_valid_hex_previews_it_on_the_row_without_applying_it_elsewhere() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = settings_app_with_highlight_row_selected();
    app.app_config.configuration_location =
        Some(dir.path().join("config.toml").display().to_string());
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    press(&mut app, &mut terminal, &events, KeyCode::Enter);
    type_str(&mut app, &mut terminal, &events, "#2E3440");

    let buffer = render_buffer(&mut app);
    for x in SAMPLE_FIRST_LETTER_X..=SAMPLE_LAST_LETTER_X {
        let cell = buffer.cell((x, ROW_Y)).unwrap();
        assert_eq!(
            cell.bg,
            Color::Rgb(0x2E, 0x34, 0x40),
            "sample cell at x={x} should preview the typed colour"
        );
        assert_eq!(cell.fg, Color::White);
    }

    assert_eq!(app.app_config.background_highlight, None);
    assert_eq!(app.highlight_bg(), Color::Blue);
    assert!(!dir.path().join("config.toml").exists());
}

#[tokio::test]
async fn test_typing_a_partial_hex_keeps_the_committed_swatch_and_marks_the_value_invalid() {
    let mut app = settings_app_with_highlight_row_selected();
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    press(&mut app, &mut terminal, &events, KeyCode::Enter);
    type_str(&mut app, &mut terminal, &events, "#2E3");

    let buffer = render_buffer(&mut app);
    let swatch = buffer.cell((SAMPLE_FIRST_LETTER_X, ROW_Y)).unwrap();
    assert_eq!(swatch.bg, Color::Blue);
    let first_value_cell = buffer.cell((VALUE_X, ROW_Y)).unwrap();
    assert_eq!(first_value_cell.fg, Color::Red);
}

#[tokio::test]
async fn test_enter_with_a_valid_hex_applies_and_persists_it() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let mut app = settings_app_with_highlight_row_selected();
    app.app_config.configuration_location = Some(config_path.display().to_string());
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    press(&mut app, &mut terminal, &events, KeyCode::Enter);
    type_str(&mut app, &mut terminal, &events, "#2E3440");
    press(&mut app, &mut terminal, &events, KeyCode::Enter);

    assert_eq!(app.mode, AppMode::Settings);
    assert_eq!(
        app.app_config.background_highlight,
        Some("#2E3440".to_string())
    );
    assert_eq!(app.highlight_bg(), Color::Rgb(0x2E, 0x34, 0x40));

    let loaded = kanban_service::config::load_from(&config_path);
    assert_eq!(loaded.background_highlight, Some("#2E3440".to_string()));

    let output = render_to_string(&mut app, 120, 30);
    assert!(output.contains("Background Highlight: #2E3440"));
}

#[tokio::test]
async fn test_enter_with_lowercase_hex_persists_the_uppercase_form() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let mut app = settings_app_with_highlight_row_selected();
    app.app_config.configuration_location = Some(config_path.display().to_string());
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    press(&mut app, &mut terminal, &events, KeyCode::Enter);
    type_str(&mut app, &mut terminal, &events, "#2e3440");
    press(&mut app, &mut terminal, &events, KeyCode::Enter);

    assert_eq!(
        app.app_config.background_highlight,
        Some("#2E3440".to_string())
    );
    let loaded = kanban_service::config::load_from(&config_path);
    assert_eq!(loaded.background_highlight, Some("#2E3440".to_string()));

    let output = render_to_string(&mut app, 120, 30);
    assert!(output.contains("Background Highlight: #2E3440"));
}

#[tokio::test]
async fn test_enter_with_an_invalid_hex_is_refused_and_nothing_is_persisted() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let mut app = settings_app_with_highlight_row_selected();
    app.app_config.configuration_location = Some(config_path.display().to_string());
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    press(&mut app, &mut terminal, &events, KeyCode::Enter);
    type_str(&mut app, &mut terminal, &events, "#2E3");
    press(&mut app, &mut terminal, &events, KeyCode::Enter);

    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::EditBackgroundHighlight)
    );
    assert_eq!(app.app_config.background_highlight, None);
    assert!(!config_path.exists());
    assert!(app.ui_state.banner.is_none());
}

#[tokio::test]
async fn test_enter_on_an_empty_input_resets_to_the_default_and_removes_the_key() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let mut app = settings_app_with_highlight_row_selected();
    app.app_config.configuration_location = Some(config_path.display().to_string());
    app.app_config.background_highlight = Some("#2E3440".to_string());
    kanban_service::config::save_background_highlight(
        &app.app_config,
        Some("#2E3440".parse().unwrap()),
    )
    .unwrap();
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    press(&mut app, &mut terminal, &events, KeyCode::Enter);
    for _ in 0..7 {
        press(&mut app, &mut terminal, &events, KeyCode::Backspace);
    }
    press(&mut app, &mut terminal, &events, KeyCode::Enter);

    assert_eq!(app.app_config.background_highlight, None);
    let loaded = kanban_service::config::load_from(&config_path);
    assert_eq!(loaded.background_highlight, None);
    assert_eq!(app.highlight_bg(), Color::Blue);
}

#[tokio::test]
async fn test_empty_input_previews_the_default_highlight() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let mut app = settings_app_with_highlight_row_selected();
    app.app_config.configuration_location = Some(config_path.display().to_string());
    app.app_config.background_highlight = Some("#2E3440".to_string());
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    press(&mut app, &mut terminal, &events, KeyCode::Enter);
    for _ in 0..7 {
        press(&mut app, &mut terminal, &events, KeyCode::Backspace);
    }

    let buffer = render_buffer(&mut app);
    let swatch = buffer.cell((SAMPLE_FIRST_LETTER_X, ROW_Y)).unwrap();
    assert_eq!(swatch.bg, Color::Blue);
}

#[tokio::test]
async fn test_esc_cancels_without_applying_or_persisting() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let mut app = settings_app_with_highlight_row_selected();
    app.app_config.configuration_location = Some(config_path.display().to_string());
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    press(&mut app, &mut terminal, &events, KeyCode::Enter);
    type_str(&mut app, &mut terminal, &events, "#2E3440");
    press(&mut app, &mut terminal, &events, KeyCode::Esc);

    assert_eq!(app.mode, AppMode::Settings);
    assert_eq!(app.app_config.background_highlight, None);
    assert!(!config_path.exists());
    assert!(app.input.is_empty());
}

#[tokio::test]
async fn test_non_hex_keys_are_ignored_and_q_does_not_quit() {
    let mut app = settings_app_with_highlight_row_selected();
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    press(&mut app, &mut terminal, &events, KeyCode::Enter);
    for c in ['q', 'z', '?', ' '] {
        press(&mut app, &mut terminal, &events, KeyCode::Char(c));
    }

    assert_eq!(app.input.as_str(), "");
    assert!(!app.should_quit);
    assert_eq!(
        app.mode,
        AppMode::Dialog(DialogMode::EditBackgroundHighlight)
    );
}

#[tokio::test]
async fn test_input_is_capped_at_seven_characters() {
    let mut app = settings_app_with_highlight_row_selected();
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    press(&mut app, &mut terminal, &events, KeyCode::Enter);
    type_str(&mut app, &mut terminal, &events, "#2E3440FF");

    assert_eq!(app.input.as_str(), "#2E3440");
}

#[tokio::test]
async fn test_cursor_sits_after_the_typed_text() {
    let mut app = settings_app_with_highlight_row_selected();
    let mut terminal_headless = headless_terminal();
    let events = EventHandler::new();

    press(&mut app, &mut terminal_headless, &events, KeyCode::Enter);
    type_str(&mut app, &mut terminal_headless, &events, "#2E");

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render(&mut app, frame);
        })
        .unwrap();
    terminal.backend_mut().assert_cursor_position((26, 6));
}

#[tokio::test]
async fn test_save_failure_keeps_the_colour_for_the_session_and_shows_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"").unwrap();
    let config_path = blocker.join("config.toml");
    let mut app = settings_app_with_highlight_row_selected();
    app.app_config.configuration_location = Some(config_path.display().to_string());
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    press(&mut app, &mut terminal, &events, KeyCode::Enter);
    type_str(&mut app, &mut terminal, &events, "#2E3440");
    press(&mut app, &mut terminal, &events, KeyCode::Enter);

    assert_eq!(app.highlight_bg(), Color::Rgb(0x2E, 0x34, 0x40));
    let banner = app
        .ui_state
        .banner
        .as_ref()
        .expect("a failed save must surface an error banner");
    assert!(banner
        .message
        .to_lowercase()
        .contains("background highlight"));
}

#[tokio::test]
async fn test_footer_shows_input_dialog_bindings_while_editing() {
    let mut app = settings_app_with_highlight_row_selected();
    let mut terminal = headless_terminal();
    let events = EventHandler::new();

    press(&mut app, &mut terminal, &events, KeyCode::Enter);

    let provider = KeybindingRegistry::get_provider(&app);
    assert!(provider.get_context().name.contains("Background Highlight"));
}

fn render_to_string(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            kanban_tui::ui::render(app, frame);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let mut result = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            result.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        result.push('\n');
    }
    result
}
